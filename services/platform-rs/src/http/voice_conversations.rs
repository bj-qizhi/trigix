use super::*;
use crate::voice_conversation::{
    FinalVoiceTranscriptRequest, VoiceConversationError, VoicePrivacyPolicy,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VoiceConversationQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VoicePolicyRequest {
    tenant_id: String,
    policy: VoicePrivacyPolicy,
}

async fn accept_final_transcript(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(request): Json<FinalVoiceTranscriptRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let record = state
        .voice_conversation_store
        .accept_final_transcript(&tenant_id, request, now_unix_ms())
        .await
        .map_err(map_voice_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::to_value(record).unwrap_or_default()),
    ))
}

async fn get_conversation(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(conversation_id): Path<String>,
    Query(query): Query<VoiceConversationQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let record = state
        .voice_conversation_store
        .get(&tenant_id, &conversation_id, now_unix_ms())
        .await
        .map_err(map_voice_error)?;
    Ok(Json(serde_json::to_value(record).unwrap_or_default()))
}

async fn delete_conversation(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(conversation_id): Path<String>,
    Query(query): Query<VoiceConversationQuery>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .voice_conversation_store
        .delete(&tenant_id, &conversation_id)
        .await
        .map_err(map_voice_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_voice_policy(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(request): Json<VoicePolicyRequest>,
) -> Result<Json<VoicePrivacyPolicy>, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    state
        .voice_conversation_store
        .set_policy(&tenant_id, request.policy)
        .await
        .map(Json)
        .map_err(map_voice_error)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn map_voice_error(error: VoiceConversationError) -> ApiError {
    match error {
        VoiceConversationError::InvalidPolicy | VoiceConversationError::InvalidRequest => {
            ApiError::bad_request("invalid voice conversation request")
        }
        VoiceConversationError::Duplicate => ApiError {
            status: StatusCode::CONFLICT,
            message: "voice transcript already accepted".to_owned(),
        },
        VoiceConversationError::NotFound => ApiError::not_found("voice conversation"),
        VoiceConversationError::StoreUnavailable => ApiError::internal("voice_conversation_store"),
    }
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/voice/conversations/final-transcripts",
            post(accept_final_transcript),
        )
        .route(
            "/v1/voice/conversations/:conversation_id",
            get(get_conversation).delete(delete_conversation),
        )
        .route("/v1/voice/privacy-policy", put(set_voice_policy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    fn token(tenant_id: &str, role: crate::auth::Role) -> String {
        sign_token(&Claims {
            sub: "voice-user".to_owned(),
            tenant_id: tenant_id.to_owned(),
            workspace_id: "workspace-1".to_owned(),
            project_id: "project-1".to_owned(),
            exp: u64::MAX,
            role,
            ..Claims::default()
        })
        .unwrap()
    }

    #[tokio::test]
    async fn jwt_tenant_wins_and_default_response_contains_no_transcript() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/voice/conversations/final-transcripts")
                    .header("content-type", "application/json")
                    .header(
                        "authorization",
                        format!("Bearer {}", token("tenant-a", crate::auth::Role::Editor)),
                    )
                    .body(Body::from(
                        serde_json::json!({
                            "tenant_id": "tenant-b",
                            "session_id": "voice-session-1",
                            "sequence": 1,
                            "occurred_at_unix_ms": 1_000,
                            "transcript": "private final transcript"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(record["tenant_id"], "tenant-a");
        assert_eq!(record["transcript_retained"], false);
        assert!(record.get("redacted_transcript").is_none());
        assert!(!String::from_utf8_lossy(&bytes).contains("private final transcript"));
    }

    #[tokio::test]
    async fn authority_fields_are_rejected_by_the_http_boundary() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/voice/conversations/final-transcripts")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "tenant_id": "tenant-a",
                            "session_id": "voice-session-1",
                            "sequence": 1,
                            "occurred_at_unix_ms": 1_000,
                            "transcript": "open settings",
                            "desktop_action": {"type": "launch_application"}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
