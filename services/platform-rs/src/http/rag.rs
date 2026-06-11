// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn rag_list_kbs_handler(
    State(_state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant = effective_tenant_id(&claims, "tenant-1");
    let base = ai_runtime_base()?;
    let url = format!("{}/v1/rag/kbs", base);
    rag_forward_json(
        reqwest::Client::new()
            .get(&url)
            .query(&[("tenant_id", tenant.as_str())]),
    )
    .await
}

async fn rag_list_documents_handler(
    State(_state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(q): Query<RagDocsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant = effective_tenant_id(&claims, "tenant-1");
    let base = ai_runtime_base()?;
    let url = format!("{}/v1/rag/documents", base);
    rag_forward_json(
        reqwest::Client::new()
            .get(&url)
            .query(&[("tenant_id", tenant.as_str()), ("kb", q.kb.as_str())]),
    )
    .await
}

async fn rag_ingest_handler(
    State(_state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<RagIngestBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_write(&claims)?;
    let tenant = effective_tenant_id(&claims, "tenant-1");
    let base = ai_runtime_base()?;
    let url = format!("{}/v1/rag/ingest", base);
    let mut payload = serde_json::json!({
        "tenant_id": tenant,
        "kb": body.kb,
        "doc_id": body.doc_id,
        "text": body.text,
    });
    if let Some(cs) = body.chunk_size {
        payload["chunk_size"] = cs.into();
    }
    if let Some(ov) = body.overlap {
        payload["overlap"] = ov.into();
    }
    rag_forward_json(reqwest::Client::new().post(&url).json(&payload)).await
}

async fn rag_delete_document_handler(
    State(_state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((kb, doc_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_write(&claims)?;
    let tenant = effective_tenant_id(&claims, "tenant-1");
    let base = ai_runtime_base()?;
    let url = format!(
        "{}/v1/rag/documents/{}/{}/{}",
        base,
        urlencode(&tenant),
        urlencode(&kb),
        urlencode(&doc_id)
    );
    rag_forward_json(reqwest::Client::new().delete(&url)).await
}

// ── Custom node registry (node SDK) ─────────────────────────────────────────

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/rag/kbs", get(rag_list_kbs_handler))
        .route("/v1/rag/documents", get(rag_list_documents_handler))
        .route("/v1/rag/ingest", post(rag_ingest_handler))
        .route(
            "/v1/rag/documents/:kb/:doc_id",
            delete(rag_delete_document_handler),
        )
}
