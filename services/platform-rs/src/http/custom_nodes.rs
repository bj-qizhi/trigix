// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn custom_nodes_list_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<crate::custom_nodes::CustomNodeDef>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "tenant-1");
    Ok(Json(
        state.custom_node_store.list_by_tenant(&tenant_id).await,
    ))
}

async fn custom_nodes_upsert_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CustomNodeBody>,
) -> Result<(StatusCode, Json<crate::custom_nodes::CustomNodeDef>), ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, "tenant-1");
    let slug = body.slug.trim().to_lowercase();
    if slug.is_empty()
        || !slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "slug must be non-empty (alphanumeric, dash, underscore)".to_string(),
        });
    }
    if body.endpoint.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "endpoint is required".to_string(),
        });
    }
    let def = crate::custom_nodes::CustomNodeDef {
        id: uuid::Uuid::new_v4().to_string(),
        tenant_id,
        slug,
        label: body.label,
        description: body.description,
        endpoint: body.endpoint.trim().to_string(),
        config_schema: if body.config_schema.is_null() {
            serde_json::json!({})
        } else {
            body.config_schema
        },
        created_at: crate::custom_nodes::unix_now(),
    };
    let saved = state.custom_node_store.upsert(def).await;
    Ok((StatusCode::CREATED, Json(saved)))
}

async fn custom_nodes_delete_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, "tenant-1");
    if state.custom_node_store.delete(&tenant_id, &id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "custom node not found".to_string(),
        })
    }
}

/// `POST /v1/custom-nodes/import` — fetch a node service's `/manifest` and
/// register every node it advertises in one step (admin only).
async fn custom_nodes_import_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<ImportManifestBody>,
) -> Result<Json<Vec<crate::custom_nodes::CustomNodeDef>>, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, "tenant-1");
    let base = body.base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "base_url is required".to_string(),
        });
    }
    let manifest_url = format!("{base}/manifest");
    let manifest: ManifestResponse = reqwest::Client::new()
        .get(&manifest_url)
        .send()
        .await
        .map_err(|e| ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: format!("could not reach {manifest_url}: {e}"),
        })?
        .json()
        .await
        .map_err(|e| ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: format!("invalid manifest at {manifest_url}: {e}"),
        })?;

    let mut imported = Vec::new();
    for n in manifest.nodes {
        // The manifest endpoint may be absolute or relative to base_url.
        let endpoint = if n.endpoint.starts_with("http://") || n.endpoint.starts_with("https://") {
            n.endpoint.clone()
        } else {
            format!("{base}{}", n.endpoint)
        };
        let def = crate::custom_nodes::CustomNodeDef {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.clone(),
            slug: n.slug.trim().to_lowercase(),
            label: n.label.unwrap_or_else(|| n.slug.clone()),
            description: n.description.unwrap_or_default(),
            endpoint,
            config_schema: if n.config_schema.is_null() {
                serde_json::json!({})
            } else {
                n.config_schema
            },
            created_at: crate::custom_nodes::unix_now(),
        };
        imported.push(state.custom_node_store.upsert(def).await);
    }
    Ok(Json(imported))
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/custom-nodes",
            get(custom_nodes_list_handler).post(custom_nodes_upsert_handler),
        )
        .route("/v1/custom-nodes/import", post(custom_nodes_import_handler))
        .route("/v1/custom-nodes/:id", delete(custom_nodes_delete_handler))
}
