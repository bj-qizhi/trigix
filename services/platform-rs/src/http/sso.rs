// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

/// Public list of enabled SSO connections — used to render login buttons.
async fn sso_public_handler(
    State(state): State<AppState>,
) -> Json<Vec<crate::sso::PublicSsoConnection>> {
    Json(state.sso_store.list_enabled_public().await)
}

async fn sso_list_connections_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<crate::sso::SsoConnection>>, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, "tenant-1");
    Ok(Json(state.sso_store.list_by_tenant(&tenant_id).await))
}

async fn sso_create_connection_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CreateSsoBody>,
) -> Result<(StatusCode, Json<crate::sso::SsoConnection>), ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, "tenant-1");
    let slug = body.slug.trim().to_lowercase();
    if slug.is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "slug must be non-empty and alphanumeric/dash only".to_string(),
        });
    }
    if state.sso_store.get_by_slug(&slug).await.is_some() {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "an SSO connection with this slug already exists".to_string(),
        });
    }
    let kind = body.kind.unwrap_or_else(crate::sso::default_kind);
    let is_oauth = crate::sso_oauth::is_oauth_kind(&kind);
    if kind != "oidc" && !is_oauth {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "kind must be one of: oidc, feishu, dingtalk, wechat_work".to_string(),
        });
    }
    let issuer = body
        .issuer
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();
    if kind == "oidc" && issuer.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "issuer is required for OIDC connections".to_string(),
        });
    }
    let conn = crate::sso::SsoConnection {
        id: uuid::Uuid::new_v4().to_string(),
        tenant_id,
        slug,
        provider: body.provider,
        kind,
        issuer,
        client_id: body.client_id,
        client_secret: body.client_secret,
        agent_id: body.agent_id.filter(|s| !s.trim().is_empty()),
        scopes: body
            .scopes
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "openid email profile".to_string()),
        enabled: true,
        created_at: crate::sso::unix_now(),
    };
    let created = state.sso_store.create(conn).await;
    Ok((StatusCode::CREATED, Json(created)))
}

async fn sso_delete_connection_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, "tenant-1");
    if state.sso_store.delete(&tenant_id, &id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "SSO connection not found".to_string(),
        })
    }
}

/// `PATCH /v1/sso-connections/:id` — enable or disable a connection (admin).
/// A disabled connection rejects login and is hidden from the login buttons.
async fn sso_update_connection_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateSsoBody>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, "tenant-1");
    if state
        .sso_store
        .set_enabled(&tenant_id, &id, body.enabled)
        .await
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "SSO connection not found".to_string(),
        })
    }
}

// ── RAG knowledge-base management (proxies the AI runtime with tenant scoping) ──

async fn sso_login_handler(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let conn = match state.sso_store.get_by_slug(&slug).await {
        Some(c) if c.enabled => c,
        _ => return sso_error_redirect("unknown or disabled SSO connection"),
    };
    let (state_jwt, nonce) = match crate::sso::sign_state(&slug) {
        Ok(v) => v,
        Err(e) => return sso_error_redirect(&e),
    };
    let redirect_uri = sso_callback_uri(&slug);

    // Custom-OAuth2 providers (Feishu / DingTalk / WeChat Work).
    if crate::sso_oauth::is_oauth_kind(&conn.kind) {
        return match crate::sso_oauth::authorize_url(
            &conn.kind,
            &conn.client_id,
            conn.agent_id.as_deref(),
            &redirect_uri,
            &state_jwt,
        ) {
            Some(url) => sso_redirect(&url),
            None => sso_error_redirect("unsupported provider kind"),
        };
    }

    // Standard OIDC.
    let md = match crate::sso::discover(&conn.issuer).await {
        Ok(m) => m,
        Err(e) => return sso_error_redirect(&format!("OIDC discovery failed: {e}")),
    };
    let authorize = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}",
        md.authorization_endpoint,
        urlencode(&conn.client_id),
        urlencode(&redirect_uri),
        urlencode(&conn.scopes),
        urlencode(&state_jwt),
        urlencode(&nonce),
    );
    sso_redirect(&authorize)
}

/// `GET /v1/sso/:slug/callback` — handle the IdP redirect: exchange the code,
/// verify the ID token, provision the user, and hand a Trigix JWT to the SPA.
async fn sso_callback_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    axum::extract::Query(q): axum::extract::Query<SsoCallbackQuery>,
) -> Response {
    if let Some(err) = q.error {
        return sso_error_redirect(&format!("IdP returned error: {err}"));
    }
    let (code, state_jwt) = match (q.code, q.state) {
        (Some(c), Some(s)) => (c, s),
        _ => return sso_error_redirect("missing code or state"),
    };

    let st = match crate::sso::verify_state(&state_jwt) {
        Ok(s) if s.slug == slug => s,
        Ok(_) => return sso_error_redirect("state/slug mismatch"),
        Err(e) => return sso_error_redirect(&e),
    };

    let conn = match state.sso_store.get_by_slug(&slug).await {
        Some(c) if c.enabled => c,
        _ => return sso_error_redirect("unknown or disabled SSO connection"),
    };

    let redirect_uri = sso_callback_uri(&slug);

    // Custom-OAuth2 providers (Feishu / DingTalk / WeChat Work). The signed
    // state above already provided CSRF protection.
    if crate::sso_oauth::is_oauth_kind(&conn.kind) {
        let info = match crate::sso_oauth::fetch_user(
            &conn.kind,
            &conn.client_id,
            &conn.client_secret,
            conn.agent_id.as_deref(),
            &code,
            &redirect_uri,
        )
        .await
        {
            Ok(i) => i,
            Err(e) => return sso_error_redirect(&e),
        };
        // Some Chinese providers don't expose an email; synthesize a stable one
        // from the provider subject so the user can still be provisioned.
        let email = info
            .email
            .unwrap_or_else(|| format!("sso-{}@{}.local", info.subject, slug));
        return sso_finish_login(&state, &conn.tenant_id, &email, info.name).await;
    }

    // Standard OIDC.
    let md = match crate::sso::discover(&conn.issuer).await {
        Ok(m) => m,
        Err(e) => return sso_error_redirect(&format!("OIDC discovery failed: {e}")),
    };
    let tokens = match crate::sso::exchange_code(
        &md.token_endpoint,
        &code,
        &conn.client_id,
        &conn.client_secret,
        &redirect_uri,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => return sso_error_redirect(&e),
    };
    let claims = match crate::sso::verify_id_token(
        &tokens.id_token,
        &md.jwks_uri,
        &conn.issuer,
        &conn.client_id,
        &st.nonce,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => return sso_error_redirect(&e),
    };
    let email = match claims.email {
        Some(e) if !e.is_empty() => e,
        _ => return sso_error_redirect("IdP did not return an email claim"),
    };
    sso_finish_login(&state, &conn.tenant_id, &email, claims.name).await
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/sso/public", get(sso_public_handler))
        .route("/v1/sso/:slug/login", get(sso_login_handler))
        .route("/v1/sso/:slug/callback", get(sso_callback_handler))
        .route(
            "/v1/sso-connections",
            get(sso_list_connections_handler).post(sso_create_connection_handler),
        )
        .route(
            "/v1/sso-connections/:id",
            delete(sso_delete_connection_handler).patch(sso_update_connection_handler),
        )
}

#[cfg(test)]
mod tests {
    use crate::http::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::http::StatusCode;
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn sso_connection_crud_and_public_list() {
        let app = router();

        // Create a connection (dev mode: require_admin passes without a token).
        let body = json!({
            "slug": "Acme-Okta",
            "provider": "Okta",
            "issuer": "https://acme.okta.com/",
            "client_id": "client-123",
            "client_secret": "super-secret-value",
            "scopes": "openid email profile"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sso-connections")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        // slug is normalized to lowercase; issuer trailing slash trimmed.
        assert_eq!(created["slug"], "acme-okta");
        assert_eq!(created["issuer"], "https://acme.okta.com");
        // The client secret must never be serialized back.
        assert!(!String::from_utf8_lossy(&bytes).contains("super-secret-value"));

        // Admin list shows it, still without the secret.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/sso-connections")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("super-secret-value"));

        // Public list exposes only slug + provider.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/sso/public")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let pubs: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(pubs.len(), 1);
        assert_eq!(pubs[0]["slug"], "acme-okta");
        assert_eq!(pubs[0]["provider"], "Okta");
        assert!(pubs[0].get("client_id").is_none());
    }

    #[tokio::test]
    async fn sso_create_rejects_bad_slug() {
        let app = router();
        let body = json!({
            "slug": "bad slug!",
            "provider": "Okta",
            "issuer": "https://x",
            "client_id": "c",
            "client_secret": "s"
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sso-connections")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn sso_disable_hides_from_public_and_rejects_login() {
        let app = router();
        let body = json!({
            "slug": "togg-okta", "provider": "Okta", "kind": "oidc",
            "issuer": "https://x.okta.com", "client_id": "c", "client_secret": "s"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sso-connections")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let id = created["id"].as_str().unwrap();

        // Disable it.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/sso-connections/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"enabled": false}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Public list no longer includes it.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/sso/public")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let pubs: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert!(!pubs.iter().any(|p| p["slug"] == "togg-okta"));

        // Login is rejected (redirects to the SPA with an error).
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/sso/togg-okta/login")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FOUND);
        let loc = resp.headers().get("location").unwrap().to_str().unwrap();
        assert!(loc.contains("sso_error"));
    }
}
