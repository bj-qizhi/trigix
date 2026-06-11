// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn list_credentials(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<CredentialQuery>,
) -> Result<Json<Vec<crate::credentials::CredentialSummary>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let list = state.credential_store.list(&tenant_id).await?;
    Ok(Json(list))
}

async fn create_credential(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CreateCredentialBody>,
) -> Result<(StatusCode, Json<crate::credentials::CredentialSummary>), ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let summary = state
        .credential_store
        .create(&body.tenant_id, &body.name, &body.value)
        .await?;
    state.audit_store.record(
        &body.tenant_id,
        audit_action::CREDENTIAL_CREATED,
        "credential",
        &summary.id,
        None,
    );
    Ok((StatusCode::CREATED, Json(summary)))
}

async fn delete_credential(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(credential_id): Path<String>,
    Query(query): Query<CredentialQuery>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .credential_store
        .delete(&tenant_id, &credential_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::CREDENTIAL_DELETED,
        "credential",
        &credential_id,
        None,
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn update_credential(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(credential_id): Path<String>,
    Json(body): Json<UpdateCredentialBody>,
) -> Result<Json<crate::credentials::CredentialSummary>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let description: Option<Option<&str>> = match &body.description {
        Some(serde_json::Value::Null) => Some(None),
        Some(serde_json::Value::String(s)) => Some(Some(s.as_str())),
        None => None,
        _ => None,
    };
    let expires_at: Option<Option<u64>> = match &body.expires_at {
        Some(serde_json::Value::Null) => Some(None),
        Some(serde_json::Value::Number(n)) => Some(n.as_u64()),
        None => None,
        _ => None,
    };
    let summary = state
        .credential_store
        .update(
            &tenant_id,
            &credential_id,
            body.value.as_deref(),
            description,
            expires_at,
        )
        .await?;
    state.audit_store.record(
        &tenant_id,
        "credential.updated",
        "credential",
        &credential_id,
        None,
    );
    Ok(Json(summary))
}

async fn list_expiring_credentials(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<ExpiringQuery>,
) -> Result<Json<Vec<crate::credentials::CredentialSummary>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let before_unix = now + query.within_days * 86400;
    let list = state
        .credential_store
        .list_expiring(&tenant_id, before_unix)
        .await?;
    Ok(Json(list))
}

// ── Credential usage audit ────────────────────────────────────────────────

async fn credential_usage_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(q): Query<TenantQuery>,
) -> Result<Json<CredentialUsageResponse>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &q.tenant_id);
    // list all workflows, then for each check published versions for credential refs
    let workflows = state
        .workflow_service
        .list_workflows(&tenant_id, None, None, None)
        .await
        .map_err(|_| ApiError::internal("Failed to list workflows"))?;

    let mut usages: std::collections::HashMap<String, Vec<CredentialUsageEntry>> =
        std::collections::HashMap::new();

    for wf in &workflows {
        // only check the latest version
        if let Some(vid) = &wf.latest_version_id {
            if let Ok(versions) = state
                .workflow_service
                .list_versions(&tenant_id, &wf.id, None, None)
                .await
            {
                if let Some(ver) = versions.iter().find(|v| &v.id == vid) {
                    let graph_str = serde_json::to_string(&ver.graph).unwrap_or_default();
                    // find all {{credential.NAME}} patterns
                    let mut start = 0;
                    while let Some(pos) = graph_str[start..].find("{{credential.") {
                        let abs = start + pos + 13; // skip "{{credential."
                        if let Some(end) = graph_str[abs..].find("}}") {
                            let cred_name = graph_str[abs..abs + end].trim().to_string();
                            if !cred_name.is_empty() {
                                usages
                                    .entry(cred_name)
                                    .or_default()
                                    .push(CredentialUsageEntry {
                                        workflow_id: wf.id.clone(),
                                        workflow_name: wf.name.clone(),
                                        version_id: ver.id.clone(),
                                        version: ver.version as u32,
                                    });
                            }
                            start = abs + end + 2;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }

    // deduplicate entries per credential (same workflow can reference same cred multiple times)
    for entries in usages.values_mut() {
        entries.dedup_by_key(|e| e.workflow_id.clone());
    }

    Ok(Json(CredentialUsageResponse { usages }))
}

// ── Env vars ──────────────────────────────────────────────────────────────

async fn list_env_vars(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<EnvVarQuery>,
) -> Result<Json<Vec<EnvVarRecord>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let set = query.set.as_deref().unwrap_or(DEFAULT_SET);
    let vars = state.env_store.list_in(&tenant_id, set).await?;
    Ok(Json(vars))
}

async fn upsert_env_var(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(key): Path<String>,
    Query(query): Query<EnvVarQuery>,
    Json(body): Json<UpsertEnvVarRequest>,
) -> Result<Json<EnvVarRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let set = query.set.as_deref().unwrap_or(DEFAULT_SET);
    let record = state
        .env_store
        .set_in(&tenant_id, set, &key, &body.value)
        .await?;
    Ok(Json(record))
}

async fn delete_env_var(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(key): Path<String>,
    Query(query): Query<EnvVarQuery>,
) -> Result<StatusCode, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let set = query.set.as_deref().unwrap_or(DEFAULT_SET);
    state.env_store.delete_in(&tenant_id, set, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_env_sets(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<CredentialQuery>,
) -> Result<Json<Vec<EnvSetSummary>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let sets = state.env_store.list_sets(&tenant_id).await?;
    Ok(Json(sets))
}

async fn delete_env_set(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<EnvSetQuery>,
) -> Result<StatusCode, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state.env_store.delete_set(&tenant_id, &query.name).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/credentials",
            get(list_credentials).post(create_credential),
        )
        .route("/v1/credentials/expiring", get(list_expiring_credentials))
        .route("/v1/credentials/usage", get(credential_usage_handler))
        .route(
            "/v1/credentials/:credential_id",
            get(method_not_allowed)
                .delete(delete_credential)
                .patch(update_credential),
        )
        .route("/v1/env-vars", get(list_env_vars))
        .route(
            "/v1/env-vars/:key",
            get(method_not_allowed)
                .put(upsert_env_var)
                .delete(delete_env_var),
        )
        .route("/v1/env-sets", get(list_env_sets).delete(delete_env_set))
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
    async fn credential_update_endpoint_returns_200() {
        let app = build_router(default_app_state());
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/credentials")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tenant_id": "tenant-1",
                            "name": "update-test-key",
                            "value": "initial"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let cred_id = body["id"].as_str().unwrap().to_string();

        let resp2 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/credentials/{cred_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tenant_id": "tenant-1",
                            "description": "Updated key",
                            "expires_at": 1999999999u64
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
        let bytes2 = to_bytes(resp2.into_body(), usize::MAX).await.unwrap();
        let body2: serde_json::Value = serde_json::from_slice(&bytes2).unwrap();
        assert_eq!(body2["description"].as_str().unwrap(), "Updated key");
        assert_eq!(body2["expires_at"].as_u64().unwrap(), 1999999999u64);
    }

    #[tokio::test]
    async fn list_expiring_credentials_endpoint() {
        use crate::credentials::CredentialStore;
        let state = default_app_state();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let c1 = state
            .credential_store
            .create("tenant-1", "soon-key-exp", "v")
            .await
            .unwrap();
        state
            .credential_store
            .update("tenant-1", &c1.id, None, None, Some(Some(now + 5 * 86400)))
            .await
            .unwrap();
        let c2 = state
            .credential_store
            .create("tenant-1", "far-key-exp", "v")
            .await
            .unwrap();
        state
            .credential_store
            .update(
                "tenant-1",
                &c2.id,
                None,
                None,
                Some(Some(now + 365 * 86400)),
            )
            .await
            .unwrap();

        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/credentials/expiring?tenant_id=tenant-1&within_days=30")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["name"].as_str().unwrap(), "soon-key-exp");
    }
}
