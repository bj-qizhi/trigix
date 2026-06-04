// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Set to true when a shutdown signal is received; causes new execution requests to return 503.
pub static DRAINING: AtomicBool = AtomicBool::new(false);

pub static METRIC_REQUESTS: AtomicU64 = AtomicU64::new(0);
pub static METRIC_EXEC_STARTED: AtomicU64 = AtomicU64::new(0);
pub static METRIC_EXEC_SUCCEEDED: AtomicU64 = AtomicU64::new(0);
pub static METRIC_EXEC_FAILED: AtomicU64 = AtomicU64::new(0);
pub static METRIC_EXEC_CANCELLED: AtomicU64 = AtomicU64::new(0);
/// Tracks number of currently-running inline executions.
pub static METRIC_EXEC_RUNNING: AtomicU64 = AtomicU64::new(0);
/// Jobs routed to the dead-letter stream (deserialize failure or execution error).
pub static METRIC_DLQ_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Maximum concurrent inline executions. Reads `MAX_CONCURRENT_EXECUTIONS` env var (default 50).
fn max_concurrent_executions() -> u64 {
    std::env::var("MAX_CONCURRENT_EXECUTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
}

/// Per-tenant max concurrent running executions. Reads `MAX_EXECUTIONS_PER_TENANT` env var (default 10).
fn max_executions_per_tenant() -> u64 {
    std::env::var("MAX_EXECUTIONS_PER_TENANT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

use axum::body::Bytes;
use axum::extract::{Extension, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::ReceiverStream;

use execution_core::ExecutionStatus;
use tracing::info;
use trigix_executor::approval::{ApprovalError as GateError, ApprovalGate};

use crate::api_keys::PlatformApiKeyStore;
use crate::audit::{action as audit_action, PlatformAuditStore};
use crate::auth::{sign_token, verify_token, Claims};
use crate::billing::{
    secs_until_quota_reset, BillingStatus, BillingStore, PlatformBillingStore, TenantQuota,
};
use crate::cache::CacheClient;
use crate::comments::{CreateCommentRequest, EditCommentRequest, PlatformCommentStore};
use crate::credentials::{
    resolve_credentials_in_json, CredentialError, CredentialStore, PlatformCredentialStore,
};
use crate::email_verification::EmailVerificationStore;
use crate::env_vars::{
    resolve_env_in_json, EnvSetSummary, EnvVarError, EnvVarRecord, EnvVarStore,
    PlatformEnvVarStore, DEFAULT_SET,
};
use crate::event_subscriptions::{
    fire_event, CreateSubscriptionRequest, PlatformSubscriptionStore, SubscriptionError,
    EVENT_EXECUTION_CANCELLED, EVENT_EXECUTION_COMPLETED, EVENT_EXECUTION_FAILED,
    EVENT_EXECUTION_STARTED,
};
use crate::execution::{
    ExecutionError, ExecutionRecord, ExecutionService, ExecutionSummary, PlatformExecutionStore,
    PlatformExecutorClient, StartExecutionRequest,
};
use crate::form::{FormError, PlatformFormStore, PublishFormRequest};
use crate::invitations::{InviteStore, PlatformInviteStore};
use crate::notification_prefs::NotificationPrefsStore;
use crate::notifications::{NotificationStore, PlatformNotificationStore};
use crate::openapi as oa;
use crate::orgs::OrgStore;
use crate::password_reset::PasswordResetStore;
use crate::scheduler::{PlatformScheduleStore, ScheduleEntry};
use crate::stripe_billing::{price_id_to_tier, tier_to_price_id, StripeClient};
use crate::test_cases::{
    CreateTestCaseRequest, PlatformTestCaseStore, TestCaseError, UpdateTestCaseRequest,
};
use crate::token_usage::{PlatformTokenUsageStore, TokenUsageStore};
use crate::users::UserStore;
use crate::variables::PlatformVariableStore;
use crate::webhook::{PlatformWebhookStore, WebhookError, WebhookRecord, WebhookStore};
use crate::workflow::{
    ArchiveWorkflowRequest, CreateWorkflowRequest, CreateWorkflowVersionRequest,
    PlatformWorkflowVersionStore, PublishWorkflowVersionRequest, RestoreWorkflowRequest,
    UpdateWorkflowRequest, WorkflowError, WorkflowRecord, WorkflowService, WorkflowVersionRecord,
};
use crate::workspace::PlatformWorkspaceStore;

type PlatformService = ExecutionService<PlatformExecutionStore, PlatformExecutorClient>;
type PlatformWorkflowService = WorkflowService<PlatformWorkflowVersionStore>;

/// Simple sliding-window rate limiter: N requests per 60-second window per tenant.
#[derive(Clone, Default)]
struct RateLimiter {
    windows: Arc<Mutex<HashMap<String, (u64, Instant)>>>,
}

impl RateLimiter {
    /// Returns `true` if the request is allowed, `false` if the limit is exceeded.
    fn check(&self, key: &str) -> bool {
        // Configurable via RATE_LIMIT_PER_MINUTE env var (default 3000 for dev)
        let limit = std::env::var("RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(3000);
        self.check_with_limit(key, limit)
    }

    /// Like `check` but with a caller-specified per-minute limit.
    fn check_with_limit(&self, key: &str, limit: u32) -> bool {
        const WINDOW: Duration = Duration::from_secs(60);
        let now = Instant::now();
        let mut map = self.windows.lock().unwrap();
        let entry = map.entry(key.to_string()).or_insert((0, now));
        if now.duration_since(entry.1) >= WINDOW {
            *entry = (1, now);
            true
        } else if entry.0 < limit as u64 {
            entry.0 += 1;
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    execution_service: Arc<PlatformService>,
    workflow_service: Arc<PlatformWorkflowService>,
    webhook_store: Arc<PlatformWebhookStore>,
    approval_gate: Arc<ApprovalGate>,
    credential_store: Arc<PlatformCredentialStore>,
    env_store: Arc<PlatformEnvVarStore>,
    schedule_store: Arc<PlatformScheduleStore>,
    audit_store: Arc<PlatformAuditStore>,
    workspace_store: Arc<PlatformWorkspaceStore>,
    variable_store: Arc<PlatformVariableStore>,
    api_key_store: Arc<PlatformApiKeyStore>,
    token_usage_store: Arc<PlatformTokenUsageStore>,
    form_store: Arc<PlatformFormStore>,
    test_case_store: Arc<PlatformTestCaseStore>,
    comment_store: Arc<PlatformCommentStore>,
    subscription_store: Arc<PlatformSubscriptionStore>,
    cache: Arc<CacheClient>,
    user_store: Arc<crate::users::PlatformUserStore>,
    org_store: Arc<crate::orgs::PlatformOrgStore>,
    invite_store: Arc<crate::invitations::PlatformInviteStore>,
    reset_store: Arc<crate::password_reset::PlatformPasswordResetStore>,
    verification_store: Arc<crate::email_verification::PlatformEmailVerificationStore>,
    notification_prefs_store: Arc<crate::notification_prefs::PlatformNotificationPrefsStore>,
    email_client: Arc<crate::email::EmailClient>,
    billing_store: Arc<PlatformBillingStore>,
    stripe_client: Option<Arc<StripeClient>>,
    rate_limiter: RateLimiter,
    notification_store: Arc<PlatformNotificationStore>,
    sso_store: Arc<crate::sso::PlatformSsoStore>,
}

pub fn router() -> Router {
    let state = default_app_state();
    spawn_schedule_runner(state.clone());
    spawn_execution_timeout_guard(state.clone());
    spawn_credential_expiry_checker(state.clone());
    build_router(state)
}

pub(crate) fn default_app_state() -> AppState {
    let store = PlatformExecutionStore::memory();
    let workflow_store = PlatformWorkflowVersionStore::memory_with_dev_seed();
    let gate = Arc::new(ApprovalGate::default());
    let usage_store = Arc::new(PlatformTokenUsageStore::default());
    let service = ExecutionService::new(
        store.clone(),
        PlatformExecutorClient::inline_with_gate_and_usage(
            store,
            Arc::clone(&gate),
            Arc::clone(&usage_store),
        ),
    );
    AppState {
        execution_service: Arc::new(service),
        workflow_service: Arc::new(WorkflowService::new(workflow_store)),
        webhook_store: Arc::new(PlatformWebhookStore::default()),
        approval_gate: gate,
        credential_store: Arc::new(PlatformCredentialStore::default()),
        env_store: Arc::new(PlatformEnvVarStore::default()),
        schedule_store: Arc::new(PlatformScheduleStore::default()),
        audit_store: Arc::new(PlatformAuditStore::default()),
        workspace_store: Arc::new(PlatformWorkspaceStore::default()),
        variable_store: Arc::new(PlatformVariableStore::default()),
        api_key_store: Arc::new(PlatformApiKeyStore::default()),
        token_usage_store: usage_store,
        form_store: Arc::new(PlatformFormStore::memory()),
        test_case_store: Arc::new(PlatformTestCaseStore::memory()),
        comment_store: Arc::new(PlatformCommentStore::default()),
        subscription_store: Arc::new(PlatformSubscriptionStore::default()),
        cache: Arc::new(CacheClient::default()),
        user_store: Arc::new(crate::users::PlatformUserStore::default()),
        org_store: Arc::new(crate::orgs::PlatformOrgStore::default()),
        invite_store: Arc::new(crate::invitations::PlatformInviteStore::default()),
        reset_store: Arc::new(crate::password_reset::PlatformPasswordResetStore::memory()),
        verification_store: Arc::new(
            crate::email_verification::PlatformEmailVerificationStore::memory(),
        ),
        notification_prefs_store: Arc::new(
            crate::notification_prefs::PlatformNotificationPrefsStore::memory(),
        ),
        email_client: Arc::new(crate::email::EmailClient::default()),
        billing_store: Arc::new(PlatformBillingStore::memory()),
        stripe_client: StripeClient::from_env().map(Arc::new),
        rate_limiter: RateLimiter::default(),
        notification_store: Arc::new(PlatformNotificationStore::default()),
        sso_store: Arc::new(crate::sso::PlatformSsoStore::default()),
    }
}

fn spawn_schedule_runner(state: AppState) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            for entry in state.schedule_store.take_due().await {
                let Ok(version) = state
                    .workflow_service
                    .get_version(&entry.tenant_id, &entry.workflow_version_id)
                    .await
                else {
                    continue;
                };
                let graph = resolve_graph_credentials(
                    version.graph,
                    &state.credential_store,
                    &state.env_store,
                    &entry.tenant_id,
                    DEFAULT_SET,
                )
                .await;
                let _ = state
                    .execution_service
                    .start(StartExecutionRequest {
                        tenant_id: entry.tenant_id,
                        workflow_id: entry.workflow_id,
                        workflow_version_id: entry.workflow_version_id,
                        graph,
                        input_json: "{}".to_string(),
                        label: None,
                        callback_url: None,
                        trigger_type: Some("schedule".to_string()),
                        dry_run: false,
                        retried_from: None,
                    })
                    .await;
            }
        }
    });
}

/// Auto-cancels executions that have been running longer than EXECUTION_TIMEOUT_SECS (default 3600).
fn spawn_execution_timeout_guard(state: AppState) {
    tokio::spawn(async move {
        let timeout_secs: u64 = std::env::var("EXECUTION_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // Gather all tenants with running executions by trying known tenants.
            // Since this is in-memory we can inspect all summaries across the store.
            // For the inline executor, running executions are actually completed synchronously,
            // so this guard primarily catches stuck/orphaned entries.
            let _ = state
                .execution_service
                .cancel_stale_running(timeout_secs, now)
                .await;
        }
    });
}

/// Background task: checks for credentials expiring within 7 days and pushes notifications.
/// Runs every 6 hours. Only notifies once per credential per check window.
fn spawn_credential_expiry_checker(state: AppState) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let warn_horizon = now + 7 * 86400;
            // We don't have a "list all tenants" API, so we check the ones we know about
            // via the audit store or through the credential store's internal scan.
            // For memory store this iterates all records; for Postgres it queries directly.
            // Use an empty tenant_id scan for memory (returns all) — Postgres needs explicit tenant.
            // Since we can't enumerate tenants without a tenant registry, we rely on
            // the notification store's per-tenant model: use internal state for memory,
            // and skip for Postgres (notifications fired per-request instead).
            if let Ok(expiring) = state.credential_store.list_expiring("", warn_horizon).await {
                for cred in expiring {
                    let days_left = cred
                        .expires_at
                        .map(|e| (e.saturating_sub(now)) / 86400)
                        .unwrap_or(0);
                    let level = if days_left == 0 { "error" } else { "warning" };
                    let tenant_id = "tenant-1"; // TODO: resolve from credential store metadata
                    state.notification_store.create(
                        tenant_id,
                        None,
                        &format!("Credential expiring: {}", cred.name),
                        &format!(
                            "Credential '{}' expires in {} day(s). Rotate it before it expires.",
                            cred.name, days_left
                        ),
                        level,
                    );
                }
            }
        }
    });
}

/// Spawns the Redis Streams execution queue worker.
/// Reads from `af:exec:queue` stream using consumer group `af:exec:workers`.
/// Each message contains a serialized `ExecutionRecord`; the worker runs it inline
/// and ACKs the message on completion.
fn spawn_queue_worker(state: AppState) {
    tokio::spawn(async move {
        let stream = crate::cache::keys::exec_queue_stream();
        let group = crate::cache::keys::exec_queue_group();
        let worker_id = format!("worker-{}", uuid::Uuid::new_v4());

        // Create consumer group (idempotent — BUSYGROUP error is swallowed).
        state.cache.xgroup_create_mkstream(stream, group).await;
        tracing::info!(worker_id = %worker_id, "Queue worker started");

        loop {
            let messages = state
                .cache
                .xreadgroup(stream, group, &worker_id, 10, 5000)
                .await;

            for (msg_id, fields) in messages {
                let job_json = fields
                    .iter()
                    .find(|(k, _)| k == "job")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();

                // On any terminal failure, route the job to the dead-letter stream
                // instead of dropping it, so it can be inspected and re-driven.
                let dead_reason: Option<String> = match serde_json::from_str::<
                    crate::execution::ExecutionRecord,
                >(&job_json)
                {
                    Ok(record) => {
                        let inline = crate::execution::InlineExecutorClient::new(
                            state.execution_service.store().clone(),
                            Arc::clone(&state.approval_gate),
                        )
                        .with_token_usage(Arc::clone(&state.token_usage_store));
                        use crate::execution::ExecutorClient;
                        match inline.start(&record).await {
                            Ok(_) => None,
                            Err(e) => {
                                tracing::error!(execution_id = %record.id, error = ?e, "Queue worker: execution failed");
                                Some(format!("execution failed: {e:?}"))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(msg_id = %msg_id, error = %e, "Queue worker: failed to deserialize job");
                        Some(format!("deserialize failed: {e}"))
                    }
                };

                if let Some(reason) = dead_reason {
                    let dead_stream = crate::cache::keys::exec_queue_dead_stream();
                    let failed_at = crate::execution::unix_now().to_string();
                    state
                        .cache
                        .xadd(
                            dead_stream,
                            &[
                                ("job", &job_json),
                                ("error", &reason),
                                ("failed_at", &failed_at),
                                ("original_msg_id", &msg_id),
                                ("worker_id", &worker_id),
                            ],
                        )
                        .await;
                    METRIC_DLQ_TOTAL.fetch_add(1, Ordering::Relaxed);
                }

                // ACK the original so it leaves the pending list; failures are now
                // preserved in the dead-letter stream rather than silently lost.
                state.cache.xack(stream, group, &msg_id).await;
            }
        }
    });
}

/// After an execution is started, spawn a watcher task that fires user notification emails
/// when the execution reaches a terminal state (failed/succeeded) — based on the workflow
/// creator's notification preferences.
fn spawn_execution_notification(
    state: &AppState,
    record: &crate::execution::ExecutionRecord,
    workflow: &crate::workflow::WorkflowRecord,
) {
    let creator_id = match &workflow.created_by {
        Some(id) if !id.is_empty() => id.clone(),
        _ => return,
    };
    let exec_id = record.id.clone();
    let tenant = record.tenant_id.clone();
    let wf_name = workflow.name.clone();
    let sla_seconds = workflow.sla_seconds;
    let budget_usd = workflow.budget_usd;
    let exec_started_at = record.started_at;
    let exec_service = Arc::clone(&state.execution_service);
    let prefs_store = Arc::clone(&state.notification_prefs_store);
    let user_store = Arc::clone(&state.user_store);
    let email_client = Arc::clone(&state.email_client);
    let notif_store = Arc::clone(&state.notification_store);

    tokio::spawn(async move {
        use crate::notification_prefs::NotificationPrefsStore;
        use crate::notifications::NotificationStore;
        use crate::users::UserStore;

        // Poll until terminal (max ~60s for inline executor, longer for queue)
        let mut terminal_status: Option<ExecutionStatus> = None;
        let mut error_msg = String::new();
        for _ in 0..240 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let exec = match exec_service.get(&tenant, &exec_id).await {
                Ok(e) => e,
                Err(_) => break,
            };
            match exec.status {
                ExecutionStatus::Failed => {
                    error_msg = exec
                        .node_results
                        .iter()
                        .filter_map(|r| r.error.as_deref())
                        .next()
                        .unwrap_or("Execution failed")
                        .to_string();
                    terminal_status = Some(ExecutionStatus::Failed);
                    break;
                }
                ExecutionStatus::Succeeded => {
                    terminal_status = Some(ExecutionStatus::Succeeded);
                    break;
                }
                ExecutionStatus::Cancelled => break,
                _ => {}
            }
        }

        let status = match terminal_status {
            Some(s) => s,
            None => return,
        };

        // SLA breach: fire a warning notification when execution exceeded the configured threshold
        if let Some(sla) = sla_seconds {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let elapsed = now_secs.saturating_sub(exec_started_at);
            if elapsed > sla {
                notif_store.create(
                    &tenant,
                    Some(&creator_id),
                    &format!("SLA breach: {}", wf_name),
                    &format!(
                        "Execution {} took {}s, exceeding the SLA of {}s.",
                        &exec_id[..exec_id.len().min(16)],
                        elapsed,
                        sla,
                    ),
                    "warning",
                );
            }
        }

        // Budget check: estimate AI token cost and fire notification if over budget
        if let Some(budget) = budget_usd {
            if status == ExecutionStatus::Succeeded {
                if let Ok(exec) = exec_service.get(&tenant, &exec_id).await {
                    let usage_records = crate::token_usage::extract_token_usage(
                        &tenant,
                        &exec_id,
                        &exec.node_results,
                        crate::execution::unix_now(),
                    );
                    let cost: f64 = usage_records
                        .iter()
                        .map(|r| {
                            let price_per_m = match r.model.as_str() {
                                m if m.contains("gpt-4o") => 5.0,
                                m if m.contains("gpt-4") => 30.0,
                                m if m.contains("gpt-3.5") => 0.5,
                                m if m.contains("gemini-2.0") => 0.1,
                                m if m.contains("gemini-1.5-pro") => 3.5,
                                m if m.contains("gemini") => 0.075,
                                m if m.contains("claude-opus") => 15.0,
                                m if m.contains("claude-sonnet") => 3.0,
                                m if m.contains("claude-haiku") => 0.25,
                                _ => 2.0,
                            };
                            (r.total_tokens as f64 / 1_000_000.0) * price_per_m
                        })
                        .sum();
                    if cost > budget {
                        notif_store.create(
                            &tenant,
                            Some(&creator_id),
                            &format!("Budget exceeded: {}", wf_name),
                            &format!(
                                "Execution {} estimated AI cost ${:.4} exceeded budget of ${:.2}.",
                                &exec_id[..exec_id.len().min(16)],
                                cost,
                                budget,
                            ),
                            "warning",
                        );
                    }
                }
            }
        }

        // Always push in-app notification regardless of email prefs
        let (title, body, level) = match status {
            ExecutionStatus::Failed => (
                format!("Execution failed: {}", wf_name),
                format!(
                    "Execution {} failed — {}",
                    &exec_id[..exec_id.len().min(16)],
                    error_msg
                ),
                "error",
            ),
            ExecutionStatus::Succeeded => (
                format!("Execution succeeded: {}", wf_name),
                format!(
                    "Execution {} completed successfully.",
                    &exec_id[..exec_id.len().min(16)]
                ),
                "info",
            ),
            _ => return,
        };
        notif_store.create(&tenant, Some(&creator_id), &title, &body, level);

        let prefs = tokio::task::spawn_blocking({
            let store = Arc::clone(&prefs_store);
            let uid = creator_id.clone();
            move || store.get(&uid)
        })
        .await
        .unwrap_or_else(|_| crate::notification_prefs::NotificationPrefs::default_for(&creator_id));

        let should_notify = match status {
            ExecutionStatus::Failed => prefs.email_on_failure,
            ExecutionStatus::Succeeded => prefs.email_on_success,
            _ => false,
        };
        if !should_notify {
            return;
        }

        let user = tokio::task::spawn_blocking({
            let store = Arc::clone(&user_store);
            let uid = creator_id.clone();
            move || store.find_by_id(&uid)
        })
        .await
        .ok()
        .flatten();

        if let Some(user) = user {
            match status {
                ExecutionStatus::Failed => {
                    email_client
                        .send_execution_failure(&user.email, &wf_name, &exec_id, &error_msg)
                        .await
                }
                ExecutionStatus::Succeeded => {
                    email_client
                        .send_execution_success(&user.email, &wf_name, &exec_id)
                        .await
                }
                _ => {}
            }
        }
    });
}

/// Attaches a UUID request ID to every request and response via `X-Request-Id` header.
async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let id = uuid::Uuid::new_v4().to_string();
    req.extensions_mut().insert(id.clone());
    let mut resp = next.run(req).await;
    if let Ok(v) = id.parse() {
        resp.headers_mut().insert("x-request-id", v);
    }
    resp
}

async fn security_headers_middleware(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(
        "x-frame-options",
        "DENY".parse().expect("static header value"),
    );
    h.insert(
        "x-content-type-options",
        "nosniff".parse().expect("static header value"),
    );
    h.insert(
        "referrer-policy",
        "strict-origin-when-cross-origin"
            .parse()
            .expect("static header value"),
    );
    h.insert(
        "strict-transport-security",
        "max-age=31536000; includeSubDomains"
            .parse()
            .expect("static header value"),
    );
    resp
}

/// Validates the Bearer JWT on all routes except the token endpoint and webhook triggers.
/// Also accepts a `?token=<jwt>` query param for SSE endpoints (EventSource can't set headers).
/// Enforces auth only when `AUTH_REQUIRED=true` is set in the environment.
/// Applies per-tenant rate limiting (300 req/60s sliding window).
async fn auth_middleware(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let public = path == "/v1/auth/token"
        || path == "/v1/auth/register"
        || path == "/v1/auth/login"
        || path == "/v1/auth/accept-invite"
        || path == "/v1/auth/forgot-password"
        || path == "/v1/auth/reset-password"
        || path == "/v1/auth/verify-email"
        || path == "/v1/auth/resend-verification"
        || path.starts_with("/v1/invitations/")
        || path.starts_with("/v1/webhooks/")
        || path.starts_with("/v1/sso/")
        || path == "/healthz"
        || path == "/healthz/detail"
        || path == "/v1/system/info"
        || path == "/metrics"
        || path == "/openapi.json"
        || path == "/docs"
        || path == "/v1/stripe/webhook";

    // Extract claims for auth check and rate-limit key
    let claims: Option<Claims> = extract_claims(req.headers()).or_else(|| {
        req.uri()
            .query()
            .and_then(|q| {
                q.split('&')
                    .find(|p| p.starts_with("token="))
                    .map(|p| p.trim_start_matches("token=").to_owned())
            })
            .and_then(|t| verify_token(&t))
    });

    if !public {
        let auth_required = std::env::var("AUTH_REQUIRED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        if auth_required && claims.is_none() {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Unauthorized", "message": "Valid Bearer token required" })),
            )
                .into_response();
        }
    }

    // Rate-limit by tenant_id (or fallback key for unauthenticated public routes)
    let rate_key = claims
        .as_ref()
        .map(|c| c.tenant_id.clone())
        .unwrap_or_else(|| "anonymous".to_string());
    if !state.rate_limiter.check(&rate_key) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({ "error": "Too Many Requests", "message": "Rate limit exceeded. Max 300 requests per 60 seconds." })),
        )
            .into_response();
    }

    // Inject validated claims into request extensions so handlers can enforce tenant isolation.
    req.extensions_mut().insert(claims);

    METRIC_REQUESTS.fetch_add(1, Ordering::Relaxed);
    next.run(req).await
}

pub(crate) fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/healthz/detail", get(healthz_detail))
        .route("/openapi.json", get(openapi_json))
        .route("/docs", get(openapi_docs))
        .route("/v1/system/info", get(system_info))
        .route("/v1/system/queue-depth", get(queue_depth_handler))
        .route("/v1/admin/dlq", get(dlq_list_handler))
        .route("/v1/admin/dlq/requeue", post(dlq_requeue_handler))
        .route("/v1/search", get(search_handler))
        .route("/metrics", get(metrics_handler))
        .route("/v1/executions", get(list_executions).post(start_execution))
        .route("/v1/executions/batch", post(start_execution_batch))
        .route("/v1/executions/stats", get(execution_stats_handler))
        .route("/v1/executions/stream", get(list_executions_stream))
        .route(
            "/v1/executions/cancel-running",
            post(cancel_all_running_executions),
        )
        .route(
            "/v1/executions/:execution_id",
            get(get_execution)
                .delete(delete_execution)
                .patch(patch_execution_label),
        )
        .route(
            "/v1/executions/:execution_id/note",
            post(set_execution_note_handler),
        )
        .route(
            "/v1/executions/:execution_id/star",
            post(star_execution_handler),
        )
        .route(
            "/v1/executions/:execution_id/unstar",
            post(unstar_execution_handler),
        )
        .route("/v1/executions/:execution_id/events", get(execution_events))
        .route(
            "/v1/executions/:execution_id/approve",
            get(method_not_allowed).post(approve_execution),
        )
        .route(
            "/v1/executions/:execution_id/reject",
            get(method_not_allowed).post(reject_execution),
        )
        .route(
            "/v1/executions/:execution_id/cancel",
            get(method_not_allowed).post(cancel_execution),
        )
        .route(
            "/v1/executions/:execution_id/retry",
            get(method_not_allowed).post(retry_execution),
        )
        .route("/v1/workflows", get(list_workflows).post(create_workflow))
        .route(
            "/v1/workflows/:workflow_id",
            get(get_workflow).patch(update_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/versions",
            get(list_workflow_versions).post(create_workflow_version),
        )
        .route(
            "/v1/workflows/:workflow_id/executions",
            get(method_not_allowed).post(start_execution_from_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/archive",
            get(method_not_allowed).post(archive_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/restore",
            get(method_not_allowed).post(restore_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/pin",
            get(method_not_allowed).post(pin_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/unpin",
            get(method_not_allowed).post(unpin_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/lock",
            get(method_not_allowed).post(lock_workflow_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/unlock",
            get(method_not_allowed).post(unlock_workflow_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/visibility",
            get(method_not_allowed).patch(set_workflow_visibility_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/move",
            get(method_not_allowed).post(move_workflow_folder),
        )
        .route(
            "/v1/workflow-versions/:workflow_version_id",
            get(get_workflow_version),
        )
        .route(
            "/v1/workflow-versions/:workflow_version_id/executions",
            get(method_not_allowed).post(start_execution_from_workflow_version),
        )
        .route(
            "/v1/workflow-versions/:workflow_version_id/publish",
            get(method_not_allowed).post(publish_workflow_version),
        )
        .route(
            "/v1/workflow-versions/:workflow_version_id/webhook",
            get(method_not_allowed).post(create_webhook),
        )
        .route("/v1/webhooks", get(list_webhooks_handler))
        .route(
            "/v1/webhooks/:token",
            get(method_not_allowed)
                .post(trigger_webhook)
                .delete(delete_webhook_handler),
        )
        .route(
            "/v1/webhooks/:token/deliveries",
            get(list_webhook_deliveries_handler),
        )
        .route(
            "/v1/webhooks/:token/condition",
            patch(update_webhook_condition_handler),
        )
        .route(
            "/v1/webhooks/:token/rate-limit",
            patch(update_webhook_rate_limit_handler),
        )
        .route("/v1/webhooks/:token/pause", post(pause_webhook_handler))
        .route("/v1/webhooks/:token/resume", post(resume_webhook_handler))
        .route(
            "/v1/webhooks/:token/rotate-secret",
            post(rotate_webhook_secret_handler),
        )
        .route(
            "/v1/webhooks/:token/payload-transform",
            post(set_payload_transform_handler),
        )
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
        .route("/v1/schedules", get(list_schedules))
        .route(
            "/v1/schedules/:version_id/pause",
            post(pause_schedule_handler),
        )
        .route(
            "/v1/schedules/:version_id/resume",
            post(resume_schedule_handler),
        )
        .route("/v1/audit-log", get(list_audit_log))
        .route("/v1/token-usage", get(get_token_usage_handler))
        .route("/v1/analytics/node-types", get(node_type_analytics_handler))
        .route("/v1/analytics/workflow-deps", get(workflow_deps_handler))
        .route(
            "/v1/analytics/workflow-stats",
            get(workflow_stats_analytics_handler),
        )
        .route("/v1/analytics/sla-breaches", get(sla_breaches_handler))
        .route("/v1/analytics/errors", get(error_analysis_handler))
        .route(
            "/v1/workflows/:workflow_id/latest-execution",
            get(get_latest_execution_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/variables",
            get(list_variables_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/variables/:key",
            get(get_variable_handler)
                .put(set_variable_handler)
                .delete(delete_variable_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/variables/:key/increment",
            post(increment_variable_handler),
        )
        .route(
            "/v1/workspaces",
            get(list_workspaces_handler).post(create_workspace_handler),
        )
        .route(
            "/v1/workspaces/:workspace_id",
            get(method_not_allowed).delete(delete_workspace_handler),
        )
        .route(
            "/v1/workspaces/:workspace_id/projects",
            get(list_projects_handler).post(create_project_handler),
        )
        .route(
            "/v1/projects/:project_id",
            get(method_not_allowed).delete(delete_project_handler),
        )
        .route("/v1/workflows/:workflow_id/export", get(export_workflow))
        .route(
            "/v1/workflows/:workflow_id/stats",
            get(workflow_stats_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/estimate",
            get(workflow_estimate_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/node-stats",
            get(workflow_node_stats_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/health",
            get(workflow_health_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/json-schema",
            get(workflow_json_schema_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/duplicate",
            get(method_not_allowed).post(duplicate_workflow),
        )
        .route(
            "/v1/workflows/import",
            get(method_not_allowed).post(import_workflow),
        )
        .route(
            "/v1/workflows/generate",
            get(method_not_allowed).post(generate_workflow),
        )
        .route("/v1/copilot", get(method_not_allowed).post(copilot_handler))
        .route(
            "/v1/api-keys",
            get(list_api_keys_handler).post(create_api_key_handler),
        )
        .route(
            "/v1/api-keys/:key_id",
            get(method_not_allowed).delete(delete_api_key_handler),
        )
        .route("/v1/auth/token", get(method_not_allowed).post(create_token))
        .route(
            "/v1/auth/register",
            get(method_not_allowed).post(register_user),
        )
        .route("/v1/auth/login", get(method_not_allowed).post(login_user))
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
        .route("/v1/rag/kbs", get(rag_list_kbs_handler))
        .route("/v1/rag/documents", get(rag_list_documents_handler))
        .route("/v1/rag/ingest", post(rag_ingest_handler))
        .route(
            "/v1/rag/documents/:kb/:doc_id",
            delete(rag_delete_document_handler),
        )
        .route("/v1/auth/me", get(me_handler).patch(update_me_handler))
        .route(
            "/v1/auth/me/notifications",
            get(get_notifications_handler).put(put_notifications_handler),
        )
        .route("/v1/admin/users", get(admin_list_users_handler))
        .route(
            "/v1/admin/users/:user_id",
            delete(admin_delete_user_handler),
        )
        .route(
            "/v1/admin/invitations",
            get(admin_list_invitations_handler).post(admin_create_invitation_handler),
        )
        .route(
            "/v1/admin/invitations/:invite_id",
            delete(admin_delete_invitation_handler),
        )
        .route("/v1/invitations/:token", get(get_invitation_handler))
        .route("/v1/auth/accept-invite", post(accept_invite_handler))
        .route("/v1/auth/forgot-password", post(forgot_password_handler))
        .route("/v1/auth/reset-password", post(reset_password_handler))
        .route("/v1/auth/verify-email", post(verify_email_handler))
        .route(
            "/v1/auth/resend-verification",
            post(resend_verification_handler),
        )
        .route("/v1/cron/preview", post(cron_preview_handler))
        .route(
            "/v1/workflows/:workflow_id/test-cases",
            get(list_test_cases_handler).post(create_test_case_handler),
        )
        .route(
            "/v1/test-cases/:test_case_id",
            get(get_test_case_handler)
                .patch(update_test_case_handler)
                .delete(delete_test_case_handler),
        )
        .route(
            "/v1/test-cases/:test_case_id/run",
            get(method_not_allowed).post(run_test_case_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/publish-form",
            get(method_not_allowed).post(publish_form_handler),
        )
        .route("/v1/workflows/:workflow_id/forms", get(list_forms_handler))
        .route(
            "/v1/forms/:token",
            get(get_form_handler).delete(delete_form_handler),
        )
        .route("/v1/forms/:token/submit", post(submit_form_handler))
        .route(
            "/v1/workflows/:workflow_id/comments",
            get(list_workflow_comments_handler).post(create_workflow_comment_handler),
        )
        .route(
            "/v1/comments/:comment_id",
            patch(edit_workflow_comment_handler).delete(delete_workflow_comment_handler),
        )
        .route(
            "/v1/event-subscriptions",
            get(list_event_subscriptions_handler).post(create_event_subscription_handler),
        )
        .route(
            "/v1/event-subscriptions/:sub_id",
            delete(delete_event_subscription_handler),
        )
        .route("/v1/orgs", get(list_orgs_handler).post(create_org_handler))
        .route(
            "/v1/orgs/:org_id",
            get(get_org_handler).delete(delete_org_handler),
        )
        .route(
            "/v1/orgs/:org_id/members",
            get(list_org_members_handler).post(add_org_member_handler),
        )
        .route(
            "/v1/orgs/:org_id/members/:user_id",
            delete(remove_org_member_handler),
        )
        .route("/v1/orgs/:org_id/switch", post(switch_org_handler))
        .route(
            "/v1/workflows/:workflow_id/rollback/:version_id",
            post(rollback_workflow_version),
        )
        .route("/.well-known/mcp.json", get(mcp_manifest))
        .route("/v1/mcp/tools", post(mcp_execute_tool))
        .route("/v1/billing/status", get(billing_status_handler))
        .route("/v1/billing/history", get(billing_history_handler))
        .route("/v1/billing/checkout", post(billing_checkout_handler))
        .route("/v1/billing/portal", post(billing_portal_handler))
        .route("/v1/stripe/webhook", post(stripe_webhook_handler))
        .route(
            "/v1/admin/billing/:tenant_id/quota",
            put(admin_set_quota_handler),
        )
        .route("/v1/notifications", get(list_notifications_handler))
        .route(
            "/v1/notifications/read-all",
            post(mark_all_notifications_read_handler),
        )
        .route(
            "/v1/notifications/:notif_id",
            delete(delete_notification_handler),
        )
        .route(
            "/v1/notifications/:notif_id/read",
            post(mark_notification_read_handler),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(middleware::from_fn(security_headers_middleware))
        .with_state(state)
}

pub fn router_with_store(store: PlatformExecutionStore) -> Router {
    let workflow_store = PlatformWorkflowVersionStore::memory_with_dev_seed();
    let gate = Arc::new(ApprovalGate::default());
    let service = ExecutionService::new(
        store.clone(),
        PlatformExecutorClient::inline_with_gate(store, Arc::clone(&gate)),
    );
    router_with_services(
        service,
        WorkflowService::new(workflow_store),
        PlatformWebhookStore::default(),
        gate,
        PlatformCredentialStore::default(),
    )
}

pub fn router_with_store_and_executor(
    store: PlatformExecutionStore,
    workflow_store: PlatformWorkflowVersionStore,
    executor: PlatformExecutorClient,
) -> Router {
    let service = ExecutionService::new(store, executor);
    router_with_services(
        service,
        WorkflowService::new(workflow_store),
        PlatformWebhookStore::default(),
        Arc::new(ApprovalGate::default()),
        PlatformCredentialStore::default(),
    )
}

pub fn router_with_services(
    execution_service: PlatformService,
    workflow_service: PlatformWorkflowService,
    webhook_store: PlatformWebhookStore,
    approval_gate: Arc<ApprovalGate>,
    credential_store: PlatformCredentialStore,
) -> Router {
    let state = AppState {
        execution_service: Arc::new(execution_service),
        workflow_service: Arc::new(workflow_service),
        webhook_store: Arc::new(webhook_store),
        approval_gate,
        credential_store: Arc::new(credential_store),
        env_store: Arc::new(PlatformEnvVarStore::default()),
        schedule_store: Arc::new(PlatformScheduleStore::default()),
        audit_store: Arc::new(PlatformAuditStore::default()),
        workspace_store: Arc::new(PlatformWorkspaceStore::default()),
        variable_store: Arc::new(PlatformVariableStore::default()),
        api_key_store: Arc::new(PlatformApiKeyStore::default()),
        token_usage_store: Arc::new(PlatformTokenUsageStore::default()),
        form_store: Arc::new(PlatformFormStore::memory()),
        test_case_store: Arc::new(PlatformTestCaseStore::memory()),
        comment_store: Arc::new(PlatformCommentStore::default()),
        subscription_store: Arc::new(PlatformSubscriptionStore::default()),
        cache: Arc::new(CacheClient::default()),
        user_store: Arc::new(crate::users::PlatformUserStore::default()),
        org_store: Arc::new(crate::orgs::PlatformOrgStore::default()),
        invite_store: Arc::new(crate::invitations::PlatformInviteStore::default()),
        reset_store: Arc::new(crate::password_reset::PlatformPasswordResetStore::memory()),
        verification_store: Arc::new(
            crate::email_verification::PlatformEmailVerificationStore::memory(),
        ),
        notification_prefs_store: Arc::new(
            crate::notification_prefs::PlatformNotificationPrefsStore::memory(),
        ),
        email_client: Arc::new(crate::email::EmailClient::default()),
        billing_store: Arc::new(PlatformBillingStore::memory()),
        stripe_client: StripeClient::from_env().map(Arc::new),
        rate_limiter: RateLimiter::default(),
        notification_store: Arc::new(PlatformNotificationStore::default()),
        sso_store: Arc::new(crate::sso::PlatformSsoStore::default()),
    };

    build_router(state)
}

/// Full-store constructor used by main when DATABASE_URL is set.
pub fn router_with_all_stores(
    execution_service: PlatformService,
    workflow_service: PlatformWorkflowService,
    approval_gate: Arc<ApprovalGate>,
    credential_store: crate::credentials::PlatformCredentialStore,
    env_store: crate::env_vars::PlatformEnvVarStore,
    audit_store: crate::audit::PlatformAuditStore,
    webhook_store: PlatformWebhookStore,
    schedule_store: PlatformScheduleStore,
    workspace_store: PlatformWorkspaceStore,
    variable_store: PlatformVariableStore,
    api_key_store: PlatformApiKeyStore,
    token_usage_store: PlatformTokenUsageStore,
    form_store: PlatformFormStore,
    test_case_store: PlatformTestCaseStore,
    comment_store: PlatformCommentStore,
    subscription_store: PlatformSubscriptionStore,
    cache: CacheClient,
    user_store: crate::users::PlatformUserStore,
    org_store: crate::orgs::PlatformOrgStore,
    invite_store: crate::invitations::PlatformInviteStore,
    reset_store: crate::password_reset::PlatformPasswordResetStore,
    verification_store: crate::email_verification::PlatformEmailVerificationStore,
    notification_prefs_store: crate::notification_prefs::PlatformNotificationPrefsStore,
    email_client: crate::email::EmailClient,
    billing_store: PlatformBillingStore,
    sso_store: crate::sso::PlatformSsoStore,
) -> Router {
    let state = AppState {
        execution_service: Arc::new(execution_service),
        workflow_service: Arc::new(workflow_service),
        webhook_store: Arc::new(webhook_store),
        approval_gate,
        credential_store: Arc::new(credential_store),
        env_store: Arc::new(env_store),
        schedule_store: Arc::new(schedule_store),
        audit_store: Arc::new(audit_store),
        workspace_store: Arc::new(workspace_store),
        variable_store: Arc::new(variable_store),
        api_key_store: Arc::new(api_key_store),
        token_usage_store: Arc::new(token_usage_store),
        form_store: Arc::new(form_store),
        test_case_store: Arc::new(test_case_store),
        comment_store: Arc::new(comment_store),
        subscription_store: Arc::new(subscription_store),
        cache: Arc::new(cache),
        user_store: Arc::new(user_store),
        org_store: Arc::new(org_store),
        invite_store: Arc::new(invite_store),
        reset_store: Arc::new(reset_store),
        verification_store: Arc::new(verification_store),
        notification_prefs_store: Arc::new(notification_prefs_store),
        email_client: Arc::new(email_client),
        billing_store: Arc::new(billing_store),
        stripe_client: StripeClient::from_env().map(Arc::new),
        rate_limiter: RateLimiter::default(),
        notification_store: Arc::new(PlatformNotificationStore::default()),
        sso_store: Arc::new(sso_store),
    };
    spawn_schedule_runner(state.clone());
    spawn_execution_timeout_guard(state.clone());
    if state.cache.is_available() {
        spawn_queue_worker(state.clone());
    }
    build_router(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn openapi_json() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&oa::spec()).unwrap_or_default(),
    )
}

async fn openapi_docs() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        oa::swagger_ui_html(),
    )
}

#[derive(serde::Serialize)]
struct HealthDetail {
    status: &'static str,
    version: &'static str,
    database: bool,
    cache: bool,
}

async fn healthz_detail(State(state): State<AppState>) -> Json<HealthDetail> {
    let database = state
        .workflow_service
        .list_workflows("__health__", None, None, None)
        .await
        .is_ok();
    let cache = state.cache.is_available();
    Json(HealthDetail {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        database,
        cache,
    })
}

#[derive(serde::Serialize)]
struct SystemInfo {
    version: &'static str,
    node_types: usize,
    auth_required: bool,
    rust_edition: &'static str,
    features: &'static [&'static str],
    max_concurrent_executions: u64,
    max_executions_per_tenant: u64,
    running_executions: u64,
}

async fn system_info() -> Json<SystemInfo> {
    Json(SystemInfo {
        version: env!("CARGO_PKG_VERSION"),
        node_types: 136,
        auth_required: std::env::var("AUTH_REQUIRED").as_deref() == Ok("true"),
        max_concurrent_executions: max_concurrent_executions(),
        max_executions_per_tenant: max_executions_per_tenant(),
        running_executions: METRIC_EXEC_RUNNING.load(Ordering::Relaxed),
        rust_edition: "2021",
        features: &[
            "jwt-auth",
            "webhook-signature",
            "sse-streaming",
            "parallel-fanout",
            "postgres-persistence",
            "cron-scheduling",
            "input-schema-validation",
            "token-usage-tracking",
            "named-env-sets",
            "per-tenant-quota",
            "distributed-scheduler-lock",
            "webhook-retry",
            "workflow-locking",
            "event-subscriptions",
            "rbac",
            "openapi-docs",
            "redis-streams-queue",
            "user-management",
            "stripe-billing",
        ],
    })
}

#[derive(serde::Serialize)]
struct QueueDepthResponse {
    queue_depth: Option<u64>,
    stream: &'static str,
    dead_letter_depth: Option<u64>,
    dead_letter_stream: &'static str,
}

async fn queue_depth_handler(State(state): State<AppState>) -> Json<QueueDepthResponse> {
    let stream = crate::cache::keys::exec_queue_stream();
    let dead_stream = crate::cache::keys::exec_queue_dead_stream();
    let queue_depth = state.cache.xlen(stream).await;
    let dead_letter_depth = state.cache.xlen(dead_stream).await;
    Json(QueueDepthResponse {
        queue_depth,
        stream,
        dead_letter_depth,
        dead_letter_stream: dead_stream,
    })
}

#[derive(serde::Serialize)]
struct DlqEntry {
    id: String,
    error: Option<String>,
    failed_at: Option<String>,
    original_msg_id: Option<String>,
    worker_id: Option<String>,
    job: Option<String>,
}

#[derive(serde::Serialize)]
struct DlqListResponse {
    depth: Option<u64>,
    entries: Vec<DlqEntry>,
}

/// GET /v1/admin/dlq — list recent dead-letter entries (admin only).
async fn dlq_list_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<DlqListResponse>, ApiError> {
    require_admin(&claims)?;
    let dead_stream = crate::cache::keys::exec_queue_dead_stream();
    let depth = state.cache.xlen(dead_stream).await;
    let raw = state.cache.xrange_last(dead_stream, 100).await;
    let entries = raw
        .into_iter()
        .map(|(id, fields)| {
            let get = |k: &str| {
                fields
                    .iter()
                    .find(|(fk, _)| fk == k)
                    .map(|(_, v)| v.clone())
            };
            DlqEntry {
                id,
                error: get("error"),
                failed_at: get("failed_at"),
                original_msg_id: get("original_msg_id"),
                worker_id: get("worker_id"),
                job: get("job"),
            }
        })
        .collect();
    Ok(Json(DlqListResponse { depth, entries }))
}

#[derive(serde::Serialize)]
struct DlqRequeueResponse {
    requeued: usize,
}

/// POST /v1/admin/dlq/requeue — re-drive all dead-letter jobs back onto the main
/// execution queue, then remove them from the dead-letter stream (admin only).
/// Note: re-running a job re-executes the whole workflow (at-least-once); only
/// re-drive when side effects are safe to repeat.
async fn dlq_requeue_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<DlqRequeueResponse>, ApiError> {
    require_admin(&claims)?;
    let dead_stream = crate::cache::keys::exec_queue_dead_stream();
    let main_stream = crate::cache::keys::exec_queue_stream();
    let entries = state.cache.xrange_last(dead_stream, 1000).await;

    let mut requeued = 0usize;
    let mut delete_ids = Vec::new();
    for (id, fields) in entries {
        if let Some((_, job)) = fields.iter().find(|(k, _)| k == "job") {
            if state
                .cache
                .xadd(main_stream, &[("job", job)])
                .await
                .is_some()
            {
                requeued += 1;
            }
        }
        delete_ids.push(id);
    }
    state.cache.xdel(dead_stream, &delete_ids).await;
    Ok(Json(DlqRequeueResponse { requeued }))
}

// ── Billing helpers ───────────────────────────────────────────────────────────

/// Fire a quota alert email when usage first crosses the 80% or 100% threshold.
/// `prev_used` is the count before the current execution was recorded.
fn spawn_quota_alert(state: &AppState, tenant_id: &str, prev_used: i64) {
    let quota = state.billing_store.get_quota(tenant_id);
    let max = quota.max_executions_per_month;
    if max == i64::MAX || max == 0 {
        return;
    }
    let t80 = (max as f64 * 0.8) as i64;
    // Only fire once: when usage transitions from below the threshold to at/above it
    let ym = {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = (now / 86400) as i64;
        let year = 1970 + days / 365;
        let month = (days % 365 / 30).clamp(0, 11) + 1;
        format!("{year:04}{month:02}")
    };
    let usage = state.billing_store.get_usage(tenant_id, &ym);
    let cur = usage.executions_used;
    let crossed_80 = prev_used < t80 && cur >= t80 && cur < max;
    let crossed_100 = prev_used < max && cur >= max;
    if !crossed_80 && !crossed_100 {
        return;
    }
    let pct = (cur as f64 / max as f64 * 100.0).min(100.0);
    let tier = quota.tier.clone();
    let tenant_id = tenant_id.to_string();
    let email_client = Arc::clone(&state.email_client);
    // Push in-app notification for quota warning
    let (notif_title, notif_body, notif_level) = if crossed_100 {
        (
            "Execution quota exceeded".to_string(),
            format!(
                "{:.0}% of monthly quota used — new executions are blocked.",
                pct
            ),
            "error",
        )
    } else {
        (
            "Execution quota warning".to_string(),
            format!("{:.0}% of {} monthly quota used.", pct, tier),
            "warning",
        )
    };
    state
        .notification_store
        .create(&tenant_id, None, &notif_title, &notif_body, notif_level);
    // Use QUOTA_ALERT_EMAIL env var as recipient; log-only if not set
    let recipient = std::env::var("QUOTA_ALERT_EMAIL").unwrap_or_default();
    tokio::spawn(async move {
        if recipient.is_empty() {
            tracing::warn!(
                tenant_id = %tenant_id,
                used = cur, max, pct,
                "Execution quota threshold crossed — set QUOTA_ALERT_EMAIL to send email"
            );
        } else {
            email_client
                .send_quota_warning(&recipient, &tenant_id, cur, max, &tier, pct)
                .await;
        }
    });
}

// ── Billing ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct BillingStatusResponse {
    #[serde(flatten)]
    status: BillingStatus,
    has_subscription: bool,
    stripe_enabled: bool,
    reset_in_secs: u64,
}

async fn billing_status_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Json<BillingStatusResponse> {
    let tenant_id = effective_tenant_id(&claims, "");
    let status = state.billing_store.billing_status(&tenant_id);
    let (_, subscription_id) = state.billing_store.get_stripe_ids(&tenant_id);
    let has_subscription = subscription_id.is_some();
    let stripe_enabled = state.stripe_client.is_some();
    let reset_in_secs = secs_until_quota_reset();
    Json(BillingStatusResponse {
        status,
        has_subscription,
        stripe_enabled,
        reset_in_secs,
    })
}

#[derive(Deserialize)]
struct HistoryQuery {
    #[serde(default = "default_history_months")]
    months: usize,
}
fn default_history_months() -> usize {
    6
}

async fn billing_history_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(q): Query<HistoryQuery>,
) -> Json<Vec<crate::billing::UsageSummary>> {
    let tenant_id = effective_tenant_id(&claims, "");
    let months = q.months.clamp(1, 24);
    Json(state.billing_store.get_usage_history(&tenant_id, months))
}

#[derive(Deserialize)]
struct CheckoutBody {
    tier: String,
}

async fn billing_checkout_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CheckoutBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let stripe = state
        .stripe_client
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("Stripe not configured"))?;
    let tenant_id = effective_tenant_id(&claims, "");
    let price_id =
        tier_to_price_id(&body.tier).ok_or_else(|| ApiError::bad_request("Unknown tier"))?;

    let (customer_id, _) = state.billing_store.get_stripe_ids(&tenant_id);
    let customer_email = if let Some(uid) = claims.as_ref().and_then(|c| c.user_id.clone()) {
        let store = Arc::clone(&state.user_store);
        tokio::task::spawn_blocking(move || store.find_by_id(&uid))
            .await
            .ok()
            .flatten()
            .map(|u| u.email)
    } else {
        None
    };

    let base =
        std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let success_url = format!("{base}/account?billing=success");
    let cancel_url = format!("{base}/account?billing=canceled");

    let url = stripe
        .create_checkout_session(
            &price_id,
            &body.tier,
            &tenant_id,
            customer_id.as_deref(),
            customer_email.as_deref(),
            &success_url,
            &cancel_url,
        )
        .await
        .map_err(|e| ApiError::internal(&e))?;

    Ok(Json(serde_json::json!({ "url": url })))
}

async fn billing_portal_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let stripe = state
        .stripe_client
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("Stripe not configured"))?;
    let tenant_id = effective_tenant_id(&claims, "");
    let (customer_id, _) = state.billing_store.get_stripe_ids(&tenant_id);
    let customer_id =
        customer_id.ok_or_else(|| ApiError::bad_request("No Stripe customer found"))?;

    let base =
        std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let return_url = format!("{base}/account");

    let url = stripe
        .create_portal_session(&customer_id, &return_url)
        .await
        .map_err(|e| ApiError::internal(&e))?;

    Ok(Json(serde_json::json!({ "url": url })))
}

async fn stripe_webhook_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let secret = std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default();
    if secret.is_empty() {
        return (StatusCode::OK, "webhook secret not configured").into_response();
    }
    if !StripeClient::verify_webhook_signature(&body, sig, &secret) {
        return (StatusCode::BAD_REQUEST, "invalid signature").into_response();
    }

    let Ok(event) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return (StatusCode::BAD_REQUEST, "invalid json").into_response();
    };

    let event_type = event["type"].as_str().unwrap_or("");
    let obj = &event["data"]["object"];

    match event_type {
        "checkout.session.completed" => {
            let tenant_id = obj["metadata"]["tenant_id"].as_str().unwrap_or("");
            let tier = obj["metadata"]["tier"].as_str().unwrap_or("pro");
            let customer = obj["customer"].as_str();
            let sub_id = obj["subscription"].as_str();
            if !tenant_id.is_empty() {
                let quota = match tier {
                    "pro" => TenantQuota::pro(tenant_id),
                    "business" => TenantQuota::business(tenant_id),
                    "enterprise" => TenantQuota::unlimited(tenant_id),
                    _ => TenantQuota::pro(tenant_id),
                };
                state.billing_store.set_quota(quota);
                state
                    .billing_store
                    .set_stripe_ids(tenant_id, customer, sub_id);
                info!(
                    tenant_id,
                    tier, "Stripe checkout.session.completed → quota upgraded"
                );
            }
        }
        "customer.subscription.updated" => {
            let customer = obj["customer"].as_str().unwrap_or("");
            let sub_id = obj["id"].as_str();
            let tier = obj["items"]["data"][0]["price"]["id"]
                .as_str()
                .and_then(|pid| price_id_to_tier(pid));
            if let Some(tenant_id) = state.billing_store.get_tenant_by_stripe_customer(customer) {
                if let Some(ref t) = tier {
                    let quota = match t.as_str() {
                        "pro" => TenantQuota::pro(&tenant_id),
                        "business" => TenantQuota::business(&tenant_id),
                        "enterprise" => TenantQuota::unlimited(&tenant_id),
                        _ => TenantQuota::pro(&tenant_id),
                    };
                    state.billing_store.set_quota(quota);
                }
                state
                    .billing_store
                    .set_stripe_ids(&tenant_id, Some(customer), sub_id);
                info!(tenant_id, "Stripe customer.subscription.updated");
            }
        }
        "customer.subscription.deleted" => {
            let customer = obj["customer"].as_str().unwrap_or("");
            if let Some(tenant_id) = state.billing_store.get_tenant_by_stripe_customer(customer) {
                state.billing_store.set_quota(TenantQuota::free(&tenant_id));
                state
                    .billing_store
                    .set_stripe_ids(&tenant_id, Some(customer), None);
                info!(
                    tenant_id,
                    "Stripe customer.subscription.deleted → downgraded to free"
                );
            }
        }
        _ => {}
    }

    StatusCode::OK.into_response()
}

#[derive(Debug, Deserialize)]
struct SetQuotaBody {
    tier: String,
}

async fn admin_set_quota_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(tenant_id): Path<String>,
    Json(body): Json<SetQuotaBody>,
) -> Result<Json<TenantQuota>, ApiError> {
    require_write(&claims)?;
    let quota = match body.tier.as_str() {
        "free" => TenantQuota::free(&tenant_id),
        "pro" => TenantQuota::pro(&tenant_id),
        "business" => TenantQuota::business(&tenant_id),
        "enterprise" => TenantQuota::unlimited(&tenant_id),
        other => {
            return Err(ApiError::bad_request(&format!(
                "Unknown tier: {other}. Valid: free, pro, business, enterprise"
            )))
        }
    };
    state.billing_store.set_quota(quota.clone());
    state.audit_store.record(
        &tenant_id,
        "billing.quota.updated",
        "tenant",
        &tenant_id,
        None,
    );
    Ok(Json(quota))
}

// ── MCP (Model Context Protocol) ──────────────────────────────────────────────

async fn mcp_manifest() -> impl IntoResponse {
    let manifest = serde_json::json!({
        "schema_version": "v1",
        "name": "trigix",
        "description": "AI Agent Workflow Platform — run and manage workflows via MCP",
        "tools": [
            {
                "name": "list_workflows",
                "description": "List all published workflows available to the caller",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "tenant_id": { "type": "string", "description": "Tenant scope (optional)" }
                    }
                }
            },
            {
                "name": "execute_workflow",
                "description": "Execute a workflow by ID or name and return the execution record",
                "input_schema": {
                    "type": "object",
                    "required": ["workflow_id"],
                    "properties": {
                        "workflow_id": { "type": "string", "description": "Workflow ID or exact name" },
                        "input":       { "type": "object", "description": "Input data for the workflow" },
                        "tenant_id":   { "type": "string" }
                    }
                }
            }
        ]
    });
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&manifest).unwrap_or_default(),
    )
}

#[derive(Debug, Deserialize)]
struct McpToolRequest {
    tool: String,
    input: Option<serde_json::Value>,
    tenant_id: Option<String>,
}

async fn mcp_execute_tool(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(req): Json<McpToolRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, req.tenant_id.as_deref().unwrap_or(""));
    match req.tool.as_str() {
        "list_workflows" => {
            let workflows = state
                .workflow_service
                .list_workflows(&tenant_id, None, Some("published"), Some(100))
                .await?;
            let items: Vec<serde_json::Value> = workflows
                .iter()
                .map(|w| {
                    serde_json::json!({
                        "id": w.id,
                        "name": w.name,
                        "description": w.description,
                        "tags": w.tags,
                    })
                })
                .collect();
            Ok(Json(serde_json::json!({ "workflows": items })))
        }
        "execute_workflow" => {
            let input = req.input.as_ref();
            let workflow_id = input
                .and_then(|v| v.get("workflow_id"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::bad_request("input.workflow_id is required"))?;
            let input_json = input
                .and_then(|v| v.get("input"))
                .map(|v| v.to_string())
                .unwrap_or_else(|| "{}".to_string());
            state
                .billing_store
                .check_execution_quota(&tenant_id)
                .map_err(|e| ApiError {
                    status: StatusCode::PAYMENT_REQUIRED,
                    message: e,
                })?;
            let workflow = state
                .workflow_service
                .get_workflow(&tenant_id, workflow_id)
                .await
                .or_else(|_| {
                    // Fallback: search by name is not supported in one step; propagate original error
                    Err(ApiError::not_found(&format!(
                        "workflow not found: {workflow_id}"
                    )))
                })?;
            let version_id = workflow
                .latest_version_id
                .ok_or(WorkflowError::NoPublishedVersion)?;
            let version = state
                .workflow_service
                .get_version(&tenant_id, &version_id)
                .await?;
            let graph = resolve_graph_credentials(
                version.graph,
                &state.credential_store,
                &state.env_store,
                &tenant_id,
                DEFAULT_SET,
            )
            .await;
            let record = state
                .execution_service
                .start(StartExecutionRequest {
                    tenant_id: tenant_id.clone(),
                    workflow_id: workflow.id,
                    workflow_version_id: version_id,
                    graph,
                    input_json,
                    label: Some("mcp".to_string()),
                    callback_url: None,
                    trigger_type: Some("mcp".to_string()),
                    dry_run: false,
                    retried_from: None,
                })
                .await?;
            let prev_used = state
                .billing_store
                .billing_status(&tenant_id)
                .usage
                .executions_used;
            state.billing_store.increment_execution(&tenant_id);
            spawn_quota_alert(&state, &tenant_id, prev_used);
            state.audit_store.record(
                &tenant_id,
                "execution.started.mcp",
                "execution",
                &record.id,
                None,
            );
            Ok(Json(serde_json::json!({
                "execution_id": record.id,
                "status": format!("{:?}", record.status).to_lowercase(),
                "workflow_id": record.workflow_id,
            })))
        }
        other => Err(ApiError::bad_request(&format!(
            "Unknown tool: {other}. Available: list_workflows, execute_workflow"
        ))),
    }
}

// ── Global search ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    tenant_id: Option<String>,
}

#[derive(Serialize)]
struct SearchResult {
    workflows: Vec<WorkflowSearchHit>,
    executions: Vec<ExecutionSearchHit>,
}

#[derive(Serialize)]
struct WorkflowSearchHit {
    id: String,
    name: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Serialize)]
struct ExecutionSearchHit {
    id: String,
    workflow_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<SearchResult>, ApiError> {
    let supplied = params.tenant_id.unwrap_or_default();
    let tenant_id = effective_tenant_id(&claims, &supplied);
    let q = params.q.unwrap_or_default().to_lowercase();
    if q.is_empty() {
        return Ok(Json(SearchResult {
            workflows: vec![],
            executions: vec![],
        }));
    }

    let workflows = state
        .workflow_service
        .list_workflows(&tenant_id, None, None, None)
        .await
        .map_err(|_| ApiError::bad_request("workflow list failed"))?;
    let wf_hits: Vec<WorkflowSearchHit> = workflows
        .into_iter()
        .filter(|w| {
            w.name.to_lowercase().contains(&q)
                || w.description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&q)
                || w.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .take(10)
        .map(|w| WorkflowSearchHit {
            id: w.id,
            name: w.name,
            status: w.status,
            description: w.description,
        })
        .collect();

    let executions = state
        .execution_service
        .list(&tenant_id)
        .await
        .map_err(|_| ApiError::bad_request("execution list failed"))?;
    let exec_hits: Vec<ExecutionSearchHit> = executions
        .into_iter()
        .filter(|e| {
            e.id.starts_with(&q) || e.label.as_deref().unwrap_or("").to_lowercase().contains(&q)
        })
        .take(10)
        .map(|e| ExecutionSearchHit {
            id: e.id,
            workflow_id: e.workflow_id,
            status: format!("{:?}", e.status).to_lowercase(),
            label: e.label,
        })
        .collect();

    Ok(Json(SearchResult {
        workflows: wf_hits,
        executions: exec_hits,
    }))
}

async fn metrics_handler() -> axum::response::Response {
    let mut out = String::with_capacity(512);

    let started = METRIC_EXEC_STARTED.load(Ordering::Relaxed);
    let succeeded = METRIC_EXEC_SUCCEEDED.load(Ordering::Relaxed);
    let failed = METRIC_EXEC_FAILED.load(Ordering::Relaxed);
    let cancelled = METRIC_EXEC_CANCELLED.load(Ordering::Relaxed);
    let running = METRIC_EXEC_RUNNING.load(Ordering::Relaxed);
    let requests = METRIC_REQUESTS.load(Ordering::Relaxed);
    let max_concurrent = max_concurrent_executions();

    out.push_str("# HELP af_executions_started_total Executions started since server start\n");
    out.push_str("# TYPE af_executions_started_total counter\n");
    out.push_str(&format!("af_executions_started_total {started}\n"));
    out.push_str("# HELP af_executions_completed_total Executions completed by outcome\n");
    out.push_str("# TYPE af_executions_completed_total counter\n");
    out.push_str(&format!(
        "af_executions_completed_total{{outcome=\"succeeded\"}} {succeeded}\n"
    ));
    out.push_str(&format!(
        "af_executions_completed_total{{outcome=\"failed\"}} {failed}\n"
    ));
    out.push_str(&format!(
        "af_executions_completed_total{{outcome=\"cancelled\"}} {cancelled}\n"
    ));
    out.push_str("# HELP af_executions_running Current number of in-flight executions\n");
    out.push_str("# TYPE af_executions_running gauge\n");
    out.push_str(&format!("af_executions_running {running}\n"));
    out.push_str(
        "# HELP af_executions_max_concurrent Configured max concurrent executions limit\n",
    );
    out.push_str("# TYPE af_executions_max_concurrent gauge\n");
    out.push_str(&format!("af_executions_max_concurrent {max_concurrent}\n"));
    let per_tenant_max = max_executions_per_tenant();
    out.push_str("# HELP af_executions_max_per_tenant Per-tenant concurrent execution limit\n");
    out.push_str("# TYPE af_executions_max_per_tenant gauge\n");
    out.push_str(&format!("af_executions_max_per_tenant {per_tenant_max}\n"));
    out.push_str("# HELP af_http_requests_total HTTP requests handled since server start\n");
    out.push_str("# TYPE af_http_requests_total counter\n");
    out.push_str(&format!("af_http_requests_total {requests}\n"));
    let dlq = METRIC_DLQ_TOTAL.load(Ordering::Relaxed);
    out.push_str(
        "# HELP af_dlq_messages_total Jobs routed to the dead-letter stream since server start\n",
    );
    out.push_str("# TYPE af_dlq_messages_total counter\n");
    out.push_str(&format!("af_dlq_messages_total {dlq}\n"));

    axum::response::Response::builder()
        .status(200)
        .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
        .body(axum::body::Body::from(out))
        .unwrap()
}

async fn start_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut request): Json<StartExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    if DRAINING.load(Ordering::Relaxed) {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Server is shutting down; new executions are not accepted.".to_string(),
        });
    }
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    state
        .billing_store
        .check_execution_quota(&request.tenant_id)
        .map_err(|e| ApiError {
            status: StatusCode::PAYMENT_REQUIRED,
            message: e,
        })?;
    request.graph = resolve_graph_credentials(
        request.graph,
        &state.credential_store,
        &state.env_store,
        &request.tenant_id,
        DEFAULT_SET,
    )
    .await;
    request.graph = inject_sub_workflow_graphs(
        request.graph,
        &state.workflow_service,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let running = state
        .execution_service
        .count_all_running()
        .await
        .unwrap_or(0);
    if running >= max_concurrent_executions() {
        return Err(ApiError::bad_request(&format!(
            "Too many concurrent executions ({running}/{max}). Try again shortly.",
            max = max_concurrent_executions()
        )));
    }
    let tenant_running = state
        .execution_service
        .count_running_by_tenant(&request.tenant_id)
        .await
        .unwrap_or(0);
    let per_tenant_max = max_executions_per_tenant();
    if tenant_running >= per_tenant_max {
        return Err(ApiError::bad_request(&format!(
            "Tenant execution limit reached ({tenant_running}/{per_tenant_max} active). Try again when a run completes."
        )));
    }
    let prev_used = state
        .billing_store
        .billing_status(&request.tenant_id)
        .usage
        .executions_used;
    let record = state.execution_service.start(request).await?;
    info!(execution_id = %record.id, tenant_id = %record.tenant_id, "execution started");
    METRIC_EXEC_STARTED.fetch_add(1, Ordering::Relaxed);
    METRIC_EXEC_RUNNING.fetch_add(1, Ordering::Relaxed);
    state.billing_store.increment_execution(&record.tenant_id);
    spawn_quota_alert(&state, &record.tenant_id, prev_used);
    state.audit_store.record(
        &record.tenant_id,
        audit_action::EXECUTION_STARTED,
        "execution",
        &record.id,
        None,
    );
    fire_event(
        Arc::clone(&state.subscription_store),
        record.tenant_id.clone(),
        EVENT_EXECUTION_STARTED,
        serde_json::json!({"execution_id": &record.id, "workflow_id": &record.workflow_id, "status": "running"}),
    );
    match record.status {
        ExecutionStatus::Succeeded => fire_event(
            Arc::clone(&state.subscription_store),
            record.tenant_id.clone(),
            EVENT_EXECUTION_COMPLETED,
            serde_json::json!({"execution_id": &record.id, "workflow_id": &record.workflow_id, "status": "succeeded"}),
        ),
        ExecutionStatus::Failed => fire_event(
            Arc::clone(&state.subscription_store),
            record.tenant_id.clone(),
            EVENT_EXECUTION_FAILED,
            serde_json::json!({"execution_id": &record.id, "workflow_id": &record.workflow_id, "status": "failed"}),
        ),
        ExecutionStatus::Cancelled => fire_event(
            Arc::clone(&state.subscription_store),
            record.tenant_id.clone(),
            EVENT_EXECUTION_CANCELLED,
            serde_json::json!({"execution_id": &record.id, "workflow_id": &record.workflow_id, "status": "cancelled"}),
        ),
        _ => {}
    }
    Ok((StatusCode::ACCEPTED, Json(record)))
}

#[derive(Debug, Deserialize)]
struct BatchStartBody {
    requests: Vec<StartExecutionRequest>,
}

async fn start_execution_batch(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<BatchStartBody>,
) -> Result<(StatusCode, Json<Vec<ExecutionRecord>>), ApiError> {
    if DRAINING.load(Ordering::Relaxed) {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Server is shutting down; new executions are not accepted.".to_string(),
        });
    }
    if body.requests.len() > 20 {
        return Err(ApiError::bad_request(
            "Batch size limited to 20 executions.",
        ));
    }
    let mut results = Vec::with_capacity(body.requests.len());
    for mut req in body.requests {
        req.tenant_id = effective_tenant_id(&claims, &req.tenant_id);
        req.graph = resolve_graph_credentials(
            req.graph,
            &state.credential_store,
            &state.env_store,
            &req.tenant_id,
            DEFAULT_SET,
        )
        .await;
        req.graph = inject_sub_workflow_graphs(
            req.graph,
            &state.workflow_service,
            &state.credential_store,
            &req.tenant_id,
        )
        .await;
        let record = state.execution_service.start(req).await?;
        METRIC_EXEC_STARTED.fetch_add(1, Ordering::Relaxed);
        state.audit_store.record(
            &record.tenant_id,
            audit_action::EXECUTION_STARTED,
            "execution",
            &record.id,
            None,
        );
        results.push(record);
    }
    Ok((StatusCode::ACCEPTED, Json(results)))
}

async fn start_execution_from_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_version_id): Path<String>,
    Json(mut request): Json<StartWorkflowVersionExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    if DRAINING.load(Ordering::Relaxed) {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Server is shutting down; new executions are not accepted.".to_string(),
        });
    }
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    state
        .billing_store
        .check_execution_quota(&request.tenant_id)
        .map_err(|e| ApiError {
            status: StatusCode::PAYMENT_REQUIRED,
            message: e,
        })?;
    let workflow_version = state
        .workflow_service
        .get_version(&request.tenant_id, &workflow_version_id)
        .await?;
    let workflow = state
        .workflow_service
        .get_workflow(&request.tenant_id, &workflow_version.workflow_id)
        .await?;
    if workflow.status == "archived" {
        return Err(WorkflowError::ArchivedWorkflow.into());
    }
    if workflow_version.status == "draft" {
        return Err(WorkflowError::DraftVersion.into());
    }
    let env_set = request.env_set.as_deref().unwrap_or(DEFAULT_SET);
    let graph = resolve_graph_credentials(
        workflow_version.graph,
        &state.credential_store,
        &state.env_store,
        &request.tenant_id,
        env_set,
    )
    .await;
    let graph = inject_sub_workflow_graphs(
        graph,
        &state.workflow_service,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: request.tenant_id,
            workflow_id: workflow_version.workflow_id,
            workflow_version_id: workflow_version.id,
            graph,
            input_json: request.input_json,
            label: request.label,
            callback_url: request.callback_url,
            trigger_type: Some("manual".to_string()),
            dry_run: request.dry_run.unwrap_or(false),
            retried_from: None,
        })
        .await?;
    let prev_used = state
        .billing_store
        .billing_status(&record.tenant_id)
        .usage
        .executions_used;
    state.billing_store.increment_execution(&record.tenant_id);
    spawn_quota_alert(&state, &record.tenant_id, prev_used);
    state.audit_store.record(
        &record.tenant_id,
        audit_action::EXECUTION_STARTED,
        "execution",
        &record.id,
        None,
    );
    // Fire user notification on failure/success based on workflow creator's prefs
    spawn_execution_notification(&state, &record, &workflow);
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn start_execution_from_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<StartWorkflowExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    if DRAINING.load(Ordering::Relaxed) {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Server is shutting down; new executions are not accepted.".to_string(),
        });
    }
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    state
        .billing_store
        .check_execution_quota(&request.tenant_id)
        .map_err(|e| ApiError {
            status: StatusCode::PAYMENT_REQUIRED,
            message: e,
        })?;
    let workflow = state
        .workflow_service
        .get_workflow(&request.tenant_id, &workflow_id)
        .await?;
    if workflow.status == "archived" {
        return Err(WorkflowError::ArchivedWorkflow.into());
    }
    let workflow_for_notif = workflow.clone();
    let workflow_version_id = workflow
        .latest_version_id
        .ok_or(WorkflowError::NoPublishedVersion)?;
    let workflow_version = state
        .workflow_service
        .get_version(&request.tenant_id, &workflow_version_id)
        .await?;
    let env_set = request.env_set.as_deref().unwrap_or(DEFAULT_SET);
    let graph = resolve_graph_credentials(
        workflow_version.graph,
        &state.credential_store,
        &state.env_store,
        &request.tenant_id,
        env_set,
    )
    .await;
    let graph = inject_sub_workflow_graphs(
        graph,
        &state.workflow_service,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let tenant_id = request.tenant_id.clone();
    let tenant_running = state
        .execution_service
        .count_running_by_tenant(&tenant_id)
        .await
        .unwrap_or(0);
    let per_tenant_max = max_executions_per_tenant();
    if tenant_running >= per_tenant_max {
        return Err(ApiError::bad_request(&format!(
            "Tenant execution limit reached ({tenant_running}/{per_tenant_max} active). Try again when a run completes."
        )));
    }
    // Per-workflow hourly rate limit
    if let Some(max_per_hour) = workflow.max_runs_per_hour {
        let hour_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(3600);
        let recent = state
            .execution_service
            .list(&tenant_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.workflow_id == workflow_id && e.started_at >= hour_ago)
            .count() as u32;
        if recent >= max_per_hour {
            return Err(ApiError {
                status: StatusCode::TOO_MANY_REQUESTS,
                message: format!(
                    "Workflow rate limit reached ({recent}/{max_per_hour} runs in the last hour)"
                ),
            });
        }
    }
    // Per-workflow concurrent execution limit
    if let Some(max_concurrent) = workflow.max_concurrent_runs {
        let wf_running = state
            .execution_service
            .count_running_by_workflow(&tenant_id, &workflow_id)
            .await
            .unwrap_or(0);
        if wf_running >= max_concurrent as u64 {
            return Err(ApiError { status: StatusCode::TOO_MANY_REQUESTS, message: format!("Workflow concurrent execution limit reached ({wf_running}/{max_concurrent} active runs)") });
        }
    }
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: request.tenant_id,
            workflow_id: workflow.id,
            workflow_version_id: workflow_version.id,
            graph,
            input_json: request.input_json,
            label: request.label,
            callback_url: request.callback_url,
            trigger_type: Some("manual".to_string()),
            dry_run: request.dry_run.unwrap_or(false),
            retried_from: None,
        })
        .await?;
    let prev_used = state
        .billing_store
        .billing_status(&record.tenant_id)
        .usage
        .executions_used;
    state.billing_store.increment_execution(&record.tenant_id);
    spawn_quota_alert(&state, &record.tenant_id, prev_used);
    state.audit_store.record(
        &record.tenant_id,
        audit_action::EXECUTION_STARTED,
        "execution",
        &record.id,
        None,
    );
    spawn_execution_notification(&state, &record, &workflow_for_notif);
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn resolve_graph_credentials(
    mut graph: workflow_core::WorkflowGraph,
    cred_store: &PlatformCredentialStore,
    env_store: &PlatformEnvVarStore,
    tenant_id: &str,
    env_set: &str,
) -> workflow_core::WorkflowGraph {
    for node in &mut graph.nodes {
        if let Some(config) = node.config.take() {
            let (resolved, _) = resolve_credentials_in_json(&config, cred_store, tenant_id).await;
            let resolved = resolve_env_in_json(&resolved, env_store, tenant_id, env_set).await;
            node.config = Some(resolved);
        }
    }
    graph
}

/// For each SubWorkflow node in the graph, resolve the target workflow's published version graph
/// and inject it as `_graph` in the node config so the executor can run it inline.
async fn inject_sub_workflow_graphs(
    mut graph: workflow_core::WorkflowGraph,
    workflow_service: &PlatformWorkflowService,
    credential_store: &PlatformCredentialStore,
    tenant_id: &str,
) -> workflow_core::WorkflowGraph {
    let env_store = PlatformEnvVarStore::default();
    for node in &mut graph.nodes {
        if node.node_type != workflow_core::NodeType::SubWorkflow
            && node.node_type != workflow_core::NodeType::ForEach
        {
            continue;
        }
        let config = match node.config.as_mut() {
            Some(c) => c,
            None => continue,
        };
        let workflow_id = match config
            .get("workflow_id")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
        {
            Some(id) => id,
            None => continue,
        };

        let Ok(workflow) = workflow_service.get_workflow(tenant_id, &workflow_id).await else {
            continue;
        };
        let Some(version_id) = workflow.latest_version_id else {
            continue;
        };
        let Ok(version) = workflow_service.get_version(tenant_id, &version_id).await else {
            continue;
        };

        let sub_graph = resolve_graph_credentials(
            version.graph,
            credential_store,
            &env_store,
            tenant_id,
            DEFAULT_SET,
        )
        .await;
        let sub_graph_json = match serde_json::to_value(&sub_graph) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(obj) = config.as_object_mut() {
            obj.insert("_graph".to_string(), sub_graph_json);
        }
    }
    graph
}

async fn list_executions(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<ListExecutionsQuery>,
) -> Result<(axum::http::HeaderMap, Json<Vec<ExecutionSummary>>), ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let records = state.execution_service.list(&tenant_id).await?;
    let filtered: Vec<_> = records
        .into_iter()
        .filter(|r| {
            query
                .workflow_id
                .as_ref()
                .map_or(true, |id| &r.workflow_id == id)
        })
        .filter(|r| {
            query
                .label
                .as_ref()
                .map_or(true, |l| r.label.as_deref() == Some(l.as_str()))
        })
        .filter(|r| {
            query.status.as_ref().map_or(true, |s| {
                format!("{:?}", r.status).to_lowercase() == s.to_lowercase() ||
            // match the canonical string forms too
            matches!((&r.status, s.as_str()),
                (execution_core::ExecutionStatus::Running, "running") |
                (execution_core::ExecutionStatus::WaitingApproval, "waiting_approval") |
                (execution_core::ExecutionStatus::Succeeded, "succeeded") |
                (execution_core::ExecutionStatus::Failed, "failed") |
                (execution_core::ExecutionStatus::Cancelled, "cancelled")
            )
            })
        })
        .filter(|r| {
            query.search.as_ref().map_or(true, |s| {
                let s = s.to_lowercase();
                r.id.to_lowercase().starts_with(&s)
                    || r.label
                        .as_deref()
                        .map_or(false, |l| l.to_lowercase().contains(&s))
            })
        })
        .collect();
    let total = filtered.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100).min(500);
    let page = filtered.into_iter().skip(offset).take(limit).collect();
    let mut headers = axum::http::HeaderMap::new();
    if let Ok(v) = total.to_string().parse() {
        headers.insert("X-Total-Count", v);
    }
    Ok((headers, Json(page)))
}

/// SSE stream that pushes the full execution list every 2 seconds.
/// Clients connect once and receive `update` events with the serialised list.
/// The stream runs until the client disconnects; fixed-segment `/stream` takes
/// priority over the `/:execution_id` param route in Axum.
async fn list_executions_stream(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<ListExecutionsQuery>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);

    tokio::spawn(async move {
        loop {
            let records = match state.execution_service.list(&tenant_id).await {
                Ok(r) => r,
                Err(_) => break,
            };
            let filtered: Vec<_> = records
                .into_iter()
                .filter(|r| {
                    query
                        .workflow_id
                        .as_ref()
                        .map_or(true, |id| &r.workflow_id == id)
                })
                .filter(|r| {
                    query
                        .label
                        .as_ref()
                        .map_or(true, |l| r.label.as_deref() == Some(l.as_str()))
                })
                .collect();

            if let Ok(data) = serde_json::to_string(&filtered) {
                if tx
                    .send(Ok(Event::default().event("update").data(data)))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

async fn get_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Result<Json<ExecutionRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let mut record = state
        .execution_service
        .get(&tenant_id, &execution_id)
        .await?;
    if record.status == ExecutionStatus::Running
        && state.approval_gate.is_waiting(&execution_id).await
    {
        record.status = ExecutionStatus::WaitingApproval;
    }
    Ok(Json(record))
}

async fn delete_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Result<StatusCode, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    // Only allow deleting terminal-state executions
    let record = state
        .execution_service
        .get(&tenant_id, &execution_id)
        .await?;
    if matches!(
        record.status,
        ExecutionStatus::Running | ExecutionStatus::WaitingApproval
    ) {
        return Err(ApiError::bad_request(
            "Cannot delete a running execution. Cancel it first.",
        ));
    }
    state
        .execution_service
        .delete(&tenant_id, &execution_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        "EXECUTION_DELETED",
        "execution",
        &execution_id,
        None,
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct PatchExecutionBody {
    #[serde(default)]
    tenant_id: String,
    label: Option<String>,
}

async fn patch_execution_label(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Json(body): Json<PatchExecutionBody>,
) -> Result<StatusCode, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    state
        .execution_service
        .set_label(&tenant_id, &execution_id, body.label)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct SetExecutionNoteBody {
    #[serde(default)]
    tenant_id: String,
    note: Option<String>,
}

async fn set_execution_note_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Json(body): Json<SetExecutionNoteBody>,
) -> Result<impl IntoResponse, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    state
        .execution_service
        .set_note(&tenant_id, &execution_id, body.note.clone())
        .await?;
    Ok(Json(serde_json::json!({ "ok": true, "note": body.note })))
}

async fn star_execution_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<TenantQuery>,
) -> Result<impl IntoResponse, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .execution_service
        .set_starred(&tenant_id, &execution_id, true)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true, "starred": true })))
}

async fn unstar_execution_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<TenantQuery>,
) -> Result<impl IntoResponse, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .execution_service
        .set_starred(&tenant_id, &execution_id, false)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true, "starred": false })))
}

async fn execution_events(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);

    tokio::spawn(async move {
        loop {
            let record = match state.execution_service.get(&tenant_id, &execution_id).await {
                Ok(mut r) => {
                    if r.status == ExecutionStatus::Running
                        && state.approval_gate.is_waiting(&execution_id).await
                    {
                        r.status = ExecutionStatus::WaitingApproval;
                    }
                    r
                }
                Err(_) => break,
            };

            let terminal = matches!(
                record.status,
                ExecutionStatus::Succeeded | ExecutionStatus::Failed | ExecutionStatus::Cancelled
            );

            if let Ok(data) = serde_json::to_string(&record) {
                if tx
                    .send(Ok(Event::default().event("update").data(data)))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            if terminal {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

async fn get_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_version_id): Path<String>,
    Query(query): Query<GetWorkflowVersionQuery>,
) -> Result<Json<WorkflowVersionRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let record = state
        .workflow_service
        .get_version(&tenant_id, &workflow_version_id)
        .await?;
    Ok(Json(record))
}

async fn rollback_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, version_id)): Path<(String, String)>,
    Query(query): Query<GetWorkflowQuery>,
) -> Result<(StatusCode, Json<WorkflowVersionRecord>), ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let source = state
        .workflow_service
        .get_version(&tenant_id, &version_id)
        .await?;
    if source.workflow_id != workflow_id {
        return Err(ApiError::not_found(
            "workflow version not found for this workflow",
        ));
    }
    let new_version = state
        .workflow_service
        .create_version(
            &workflow_id,
            crate::workflow::CreateWorkflowVersionRequest {
                tenant_id: tenant_id.clone(),
                graph: source.graph,
                status: Some("draft".to_string()),
                message: Some(format!("Rollback to v{}", source.version)),
            },
        )
        .await?;
    state.audit_store.record(
        &tenant_id,
        "workflow.version.rollback",
        "workflow_version",
        &new_version.id,
        None,
    );
    Ok((StatusCode::CREATED, Json(new_version)))
}

async fn create_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut request): Json<CreateWorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowRecord>), ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    if request.created_by.is_none() {
        request.created_by = claims.as_ref().and_then(|c| c.user_id.clone());
    }
    let record = state.workflow_service.create_workflow(request).await?;
    state.audit_store.record(
        &record.tenant_id,
        audit_action::WORKFLOW_CREATED,
        "workflow",
        &record.id,
        None,
    );
    Ok((StatusCode::CREATED, Json(record)))
}

async fn list_workflows(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<ListWorkflowsQuery>,
) -> Result<Json<Vec<WorkflowRecord>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let caller_user_id = claims.as_ref().and_then(|c| c.user_id.clone());
    let caller_role = claims.as_ref().map(|c| c.role.as_str()).unwrap_or("");
    let mut records = state
        .workflow_service
        .list_workflows(
            &tenant_id,
            query.project_id.as_deref(),
            query.status.as_deref(),
            query.limit,
        )
        .await?;
    records.retain(|r| {
        r.visibility != "private"
            || caller_role == "admin"
            || r.created_by.as_deref() == caller_user_id.as_deref()
    });
    if let Some(tag) = &query.tag {
        records.retain(|r| r.tags.iter().any(|t| t == tag));
    }
    if let Some(folder) = &query.folder {
        records.retain(|r| r.folder.as_deref() == Some(folder.as_str()));
    }
    Ok(Json(records))
}

async fn get_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<GetWorkflowQuery>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let record = state
        .workflow_service
        .get_workflow(&tenant_id, &workflow_id)
        .await?;
    if record.visibility == "private" {
        let caller_user_id = claims.as_ref().and_then(|c| c.user_id.clone());
        let caller_role = claims.as_ref().map(|c| c.role.as_str()).unwrap_or("");
        if caller_role != "admin" && record.created_by.as_deref() != caller_user_id.as_deref() {
            return Err(ApiError::not_found("workflow not found"));
        }
    }
    Ok(Json(record))
}

async fn update_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<UpdateWorkflowRequest>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let tenant_id = request.tenant_id.clone();
    let tags_changed = request.tags.is_some();
    let record = state
        .workflow_service
        .update_workflow(&workflow_id, request)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_UPDATED,
        "workflow",
        &workflow_id,
        None,
    );
    if tags_changed && !record.tags.is_empty() {
        let tag_detail = serde_json::Value::String(record.tags.join(","));
        state.audit_store.record(
            &tenant_id,
            audit_action::WORKFLOW_TAGGED,
            "workflow",
            &workflow_id,
            Some(tag_detail),
        );
    }
    Ok(Json(record))
}

async fn archive_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<ArchiveWorkflowRequest>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let tenant_id = request.tenant_id.clone();
    let record = state
        .workflow_service
        .archive_workflow(&workflow_id, request)
        .await?;
    // Remove any schedule for this workflow's latest version.
    if let Some(version_id) = &record.latest_version_id {
        if state.schedule_store.unregister(version_id) {
            state.audit_store.record(
                &tenant_id,
                audit_action::SCHEDULE_REMOVED,
                "workflow_version",
                version_id,
                None,
            );
        }
    }
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_ARCHIVED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn restore_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<RestoreWorkflowRequest>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let tenant_id = request.tenant_id.clone();
    let record = state
        .workflow_service
        .restore_workflow(&workflow_id, request)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_RESTORED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn pin_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(
        &claims,
        body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let record = state
        .workflow_service
        .pin_workflow(&tenant_id, &workflow_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_PINNED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn unpin_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(
        &claims,
        body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let record = state
        .workflow_service
        .unpin_workflow(&tenant_id, &workflow_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_UNPINNED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn lock_workflow_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(
        &claims,
        body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let record = state
        .workflow_service
        .lock_workflow(&tenant_id, &workflow_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_LOCKED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn unlock_workflow_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(
        &claims,
        body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let record = state
        .workflow_service
        .unlock_workflow(&tenant_id, &workflow_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_UNLOCKED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

#[derive(Deserialize)]
struct SetVisibilityBody {
    #[serde(default)]
    tenant_id: String,
    visibility: String,
}

async fn set_workflow_visibility_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut body): Json<SetVisibilityBody>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let existing = state
        .workflow_service
        .get_workflow(&body.tenant_id, &workflow_id)
        .await?;
    let caller_user_id = claims.as_ref().and_then(|c| c.user_id.clone());
    let caller_role = claims.as_ref().map(|c| c.role.as_str()).unwrap_or("");
    if caller_role != "admin" && existing.created_by.as_deref() != caller_user_id.as_deref() {
        return Err(ApiError::forbidden(
            "Only the creator or an admin can change workflow visibility",
        ));
    }
    let record = state
        .workflow_service
        .set_workflow_visibility(&body.tenant_id, &workflow_id, &body.visibility)
        .await?;
    Ok(Json(record))
}

#[derive(Deserialize)]
struct MoveWorkflowBody {
    tenant_id: String,
    #[serde(default)]
    folder: Option<String>,
}

async fn move_workflow_folder(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut body): Json<MoveWorkflowBody>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let current = state
        .workflow_service
        .get_workflow(&body.tenant_id, &workflow_id)
        .await?;
    let record = state
        .workflow_service
        .update_workflow(
            &workflow_id,
            UpdateWorkflowRequest {
                tenant_id: body.tenant_id.clone(),
                name: current.name,
                tags: None,
                description: None,
                readme: None,
                folder: Some(body.folder.unwrap_or_default()).filter(|s| !s.is_empty()),
                sla_seconds: None,
                max_runs_per_hour: None,
                max_concurrent_runs: None,
                budget_usd: None,
            },
        )
        .await?;
    Ok(Json(record))
}

#[derive(serde::Serialize)]
struct WorkflowStats {
    total: usize,
    succeeded: usize,
    failed: usize,
    running: usize,
    avg_duration_secs: Option<f64>,
}

async fn workflow_stats_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<WorkflowStats>, ApiError> {
    let tenant_id = effective_tenant_id(
        &claims,
        query.get("tenant_id").map(|s| s.as_str()).unwrap_or(""),
    );
    let records = state.execution_service.list(&tenant_id).await?;
    let wf_execs: Vec<_> = records
        .into_iter()
        .filter(|r| r.workflow_id == workflow_id)
        .collect();
    let total = wf_execs.len();
    let succeeded = wf_execs
        .iter()
        .filter(|r| matches!(r.status, execution_core::ExecutionStatus::Succeeded))
        .count();
    let failed = wf_execs
        .iter()
        .filter(|r| matches!(r.status, execution_core::ExecutionStatus::Failed))
        .count();
    let running = wf_execs
        .iter()
        .filter(|r| {
            matches!(
                r.status,
                execution_core::ExecutionStatus::Running
                    | execution_core::ExecutionStatus::WaitingApproval
            )
        })
        .count();
    let durations: Vec<f64> = wf_execs
        .iter()
        .filter_map(|r| r.finished_at.map(|f| (f as f64) - (r.started_at as f64)))
        .filter(|&d| d >= 0.0)
        .collect();
    let avg_duration_secs = if durations.is_empty() {
        None
    } else {
        Some(durations.iter().sum::<f64>() / durations.len() as f64)
    };
    Ok(Json(WorkflowStats {
        total,
        succeeded,
        failed,
        running,
        avg_duration_secs,
    }))
}

#[derive(serde::Serialize)]
struct WorkflowEstimate {
    sample_count: usize,
    p50_secs: Option<f64>,
    p95_secs: Option<f64>,
    min_secs: Option<f64>,
    max_secs: Option<f64>,
}

async fn workflow_estimate_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Json<WorkflowEstimate> {
    let tenant_id = effective_tenant_id(
        &claims,
        query.get("tenant_id").map(|s| s.as_str()).unwrap_or(""),
    );
    let records = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let mut durations: Vec<f64> = records
        .into_iter()
        .filter(|r| {
            r.workflow_id == workflow_id
                && matches!(r.status, execution_core::ExecutionStatus::Succeeded)
        })
        .filter_map(|r| r.finished_at.map(|f| (f as f64) - (r.started_at as f64)))
        .filter(|&d| d >= 0.0)
        .collect();
    durations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = durations.len();
    let percentile = |p: f64| -> Option<f64> {
        if n == 0 {
            return None;
        }
        let idx = ((n as f64) * p).floor() as usize;
        Some(durations[idx.min(n - 1)])
    };
    Json(WorkflowEstimate {
        sample_count: n,
        p50_secs: percentile(0.5),
        p95_secs: percentile(0.95),
        min_secs: durations.first().copied(),
        max_secs: durations.last().copied(),
    })
}

#[derive(Debug, serde::Serialize)]
struct NodeStat {
    node_id: String,
    node_type: String,
    total: usize,
    succeeded: usize,
    failed: usize,
    skipped: usize,
    avg_duration_ms: Option<f64>,
}

async fn workflow_node_stats_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Json<Vec<NodeStat>> {
    let tenant_id = effective_tenant_id(
        &claims,
        query.get("tenant_id").map(|s| s.as_str()).unwrap_or(""),
    );
    let records = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let wf_execs: Vec<_> = records
        .into_iter()
        .filter(|r| r.workflow_id == workflow_id)
        .collect();

    let mut map: HashMap<String, NodeStat> = HashMap::new();
    let mut durations: HashMap<String, Vec<f64>> = HashMap::new();

    for exec in &wf_execs {
        if let Ok(full) = state.execution_service.get(&tenant_id, &exec.id).await {
            for nr in &full.node_results {
                let st = map.entry(nr.node_id.clone()).or_insert(NodeStat {
                    node_id: nr.node_id.clone(),
                    node_type: nr.node_type.clone(),
                    total: 0,
                    succeeded: 0,
                    failed: 0,
                    skipped: 0,
                    avg_duration_ms: None,
                });
                st.total += 1;
                match nr.status {
                    execution_core::NodeStatus::Succeeded => st.succeeded += 1,
                    execution_core::NodeStatus::Failed => st.failed += 1,
                    execution_core::NodeStatus::Skipped => st.skipped += 1,
                    _ => {}
                }
                if nr.duration_ms > 0 {
                    durations
                        .entry(nr.node_id.clone())
                        .or_default()
                        .push(nr.duration_ms as f64);
                }
            }
        }
    }

    for (node_id, durs) in &durations {
        if let Some(st) = map.get_mut(node_id) {
            st.avg_duration_ms = Some(durs.iter().sum::<f64>() / durs.len() as f64);
        }
    }

    let mut stats: Vec<_> = map.into_values().collect();
    stats.sort_by(|a, b| b.total.cmp(&a.total));
    Json(stats)
}

async fn create_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<CreateWorkflowVersionRequest>,
) -> Result<(StatusCode, Json<WorkflowVersionRecord>), ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let wf = state
        .workflow_service
        .get_workflow(&request.tenant_id, &workflow_id)
        .await?;
    if wf.locked {
        return Err(WorkflowError::LockedWorkflow.into());
    }
    let record = state
        .workflow_service
        .create_version(&workflow_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn publish_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_version_id): Path<String>,
    Json(mut request): Json<PublishWorkflowVersionRequest>,
) -> Result<Json<WorkflowVersionRecord>, ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let record = state
        .workflow_service
        .publish_version(&request.tenant_id, &workflow_version_id)
        .await?;

    // If the trigger node has an interval_secs or cron_expression config, register with the scheduler.
    let _ = state.schedule_store.unregister(&workflow_version_id);
    let cron_expr = extract_trigger_cron(&record.graph);
    let interval_secs = extract_trigger_interval(&record.graph).unwrap_or(0);
    if cron_expr.is_some() || interval_secs >= 60 {
        let next_run_at = cron_expr
            .as_deref()
            .and_then(crate::scheduler::cron_next_instant)
            .unwrap_or_else(|| {
                std::time::Instant::now() + std::time::Duration::from_secs(interval_secs)
            });
        state.schedule_store.register(ScheduleEntry {
            workflow_id: record.workflow_id.clone(),
            workflow_version_id: record.id.clone(),
            tenant_id: request.tenant_id.clone(),
            interval_secs,
            cron_expression: cron_expr,
            next_run_at,
            paused: false,
        });
        state.audit_store.record(
            &request.tenant_id,
            audit_action::SCHEDULE_REGISTERED,
            "workflow_version",
            &record.id,
            None,
        );
    }
    state.audit_store.record(
        &request.tenant_id,
        audit_action::WORKFLOW_PUBLISHED,
        "workflow_version",
        &record.id,
        None,
    );

    Ok(Json(record))
}

async fn list_workflow_versions(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<ListWorkflowVersionsQuery>,
) -> Result<Json<Vec<WorkflowVersionRecord>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let records = state
        .workflow_service
        .list_versions(
            &tenant_id,
            &workflow_id,
            query.status.as_deref(),
            query.limit,
        )
        .await?;
    Ok(Json(records))
}

async fn approve_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
    Json(body): Json<ApprovalBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .approval_gate
        .resolve(&execution_id, true)
        .await
        .map_err(|e| match e {
            GateError::NotFound => ApiError {
                status: StatusCode::NOT_FOUND,
                message: "NoApprovalPending".to_string(),
            },
        })?;
    let comment_meta = body
        .comment
        .as_ref()
        .map(|c| serde_json::json!({ "comment": c }));
    state.audit_store.record(
        body.tenant_id.as_deref().unwrap_or(""),
        audit_action::EXECUTION_APPROVED,
        "execution",
        &execution_id,
        comment_meta,
    );
    Ok(Json(
        serde_json::json!({ "ok": true, "comment": body.comment }),
    ))
}

async fn reject_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
    Json(body): Json<ApprovalBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .approval_gate
        .resolve(&execution_id, false)
        .await
        .map_err(|e| match e {
            GateError::NotFound => ApiError {
                status: StatusCode::NOT_FOUND,
                message: "NoApprovalPending".to_string(),
            },
        })?;
    let comment_meta = body
        .comment
        .as_ref()
        .map(|c| serde_json::json!({ "comment": c }));
    state.audit_store.record(
        body.tenant_id.as_deref().unwrap_or(""),
        audit_action::EXECUTION_REJECTED,
        "execution",
        &execution_id,
        comment_meta,
    );
    Ok(Json(
        serde_json::json!({ "ok": true, "comment": body.comment }),
    ))
}

async fn cancel_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Json(body): Json<CancelExecutionBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    state
        .execution_service
        .cancel(&tenant_id, &execution_id)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "ExecutionNotFound".to_string(),
        })?;
    state.audit_store.record(
        &tenant_id,
        audit_action::EXECUTION_CANCELLED,
        "execution",
        &execution_id,
        None,
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn cancel_all_running_executions(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CancelExecutionBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let count = state
        .execution_service
        .cancel_all_running(&tenant_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::EXECUTION_CANCELLED,
        "execution",
        "*",
        Some(serde_json::json!({ "bulk_cancel": count })),
    );
    Ok(Json(serde_json::json!({ "cancelled": count })))
}

async fn retry_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Json(body): Json<RetryExecutionBody>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let original = state
        .execution_service
        .get(&tenant_id, &execution_id)
        .await?;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: original.tenant_id.clone(),
            workflow_id: original.workflow_id.clone(),
            workflow_version_id: original.workflow_version_id.clone(),
            graph: original.graph.clone(),
            input_json: body
                .input_json
                .clone()
                .unwrap_or_else(|| original.input_json.clone()),
            label: body.label.clone().or_else(|| original.label.clone()),
            callback_url: None,
            trigger_type: Some("retry".to_string()),
            dry_run: false,
            retried_from: Some(execution_id.clone()),
        })
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::EXECUTION_RETRIED,
        "execution",
        &record.id,
        Some(serde_json::json!({ "retried_from": execution_id })),
    );
    Ok((StatusCode::CREATED, Json(record)))
}

async fn create_webhook(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_version_id): Path<String>,
    Json(mut body): Json<CreateWebhookBody>,
) -> Result<Json<WebhookResponse>, ApiError> {
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    match state
        .webhook_store
        .get_by_version(&workflow_version_id)
        .await
    {
        Ok(Some(existing)) => {
            return Ok(Json(WebhookResponse {
                url: format!("/v1/webhooks/{}", existing.token),
                token: existing.token,
                secret: existing.secret,
            }));
        }
        Ok(None) => {}
        Err(e) => return Err(e.into()),
    }

    let version = state
        .workflow_service
        .get_version(&body.tenant_id, &workflow_version_id)
        .await?;

    let secret = extract_trigger_webhook_secret(&version.graph);
    let token = uuid::Uuid::new_v4().to_string().replace('-', "");
    let record = state
        .webhook_store
        .upsert(WebhookRecord {
            token,
            tenant_id: body.tenant_id,
            workflow_id: version.workflow_id,
            workflow_version_id,
            secret: secret.clone(),
            condition_expr: None,
            max_calls_per_minute: None,
            paused: false,
            payload_transform_script: None,
        })
        .await
        .map_err(ApiError::from)?;

    Ok(Json(WebhookResponse {
        url: format!("/v1/webhooks/{}", record.token),
        token: record.token,
        secret,
    }))
}

async fn trigger_webhook(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    if DRAINING.load(Ordering::Relaxed) {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Server is shutting down; new executions are not accepted.".to_string(),
        });
    }
    let webhook = state
        .webhook_store
        .get_by_token(&token)
        .await
        .map_err(ApiError::from)?;

    // Capture delivery metadata before consuming webhook fields
    let delivery_token = webhook.token.clone();
    let delivery_tenant = webhook.tenant_id.clone();
    let delivery_store = state.webhook_store.clone();

    let inner_result: Result<ExecutionRecord, ApiError> = async {
        state
            .billing_store
            .check_execution_quota(&webhook.tenant_id)
            .map_err(|e| ApiError {
                status: StatusCode::PAYMENT_REQUIRED,
                message: e,
            })?;

        // Replay-attack protection: reject requests where the timestamp header is
        // absent (when a secret is set) or outside a ±5-minute window.
        if webhook.secret.is_some() {
            const WINDOW_SECS: u64 = 300;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let ts: u64 = headers
                .get("x-trigix-timestamp")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if ts == 0 || now.abs_diff(ts) > WINDOW_SECS {
                return Err(ApiError {
                    status: StatusCode::BAD_REQUEST,
                    message: format!(
                        "Missing or stale X-Trigix-Timestamp (window: ±{WINDOW_SECS}s). \
                         Send the current Unix timestamp in the header."
                    ),
                });
            }
        }

        // If the webhook has a secret, validate the HMAC-SHA256 signature.
        if let Some(secret) = &webhook.secret {
            let sig = headers
                .get("x-webhook-signature")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !crate::webhook::verify_signature(secret, &body, sig) {
                return Err(ApiError {
                    status: StatusCode::UNAUTHORIZED,
                    message: "Invalid webhook signature".to_string(),
                });
            }
        }

        let mut input_json = if body.is_empty() {
            "{}".to_string()
        } else {
            String::from_utf8(body.to_vec()).unwrap_or_else(|_| "{}".to_string())
        };

        // Reject paused webhooks
        if webhook.paused {
            return Err(ApiError {
                status: StatusCode::SERVICE_UNAVAILABLE,
                message: "Webhook is paused".to_string(),
            });
        }

        // Per-webhook rate limit (in-memory sliding window)
        if let Some(max_per_min) = webhook.max_calls_per_minute {
            if !state
                .rate_limiter
                .check_with_limit(&format!("wh:{}", &webhook.token), max_per_min)
            {
                return Err(ApiError {
                    status: StatusCode::TOO_MANY_REQUESTS,
                    message: format!("Webhook rate limit exceeded ({max_per_min}/min)"),
                });
            }
        }

        // Evaluate optional condition expression against payload
        if let Some(cond) = &webhook.condition_expr {
            if !cond.is_empty() {
                let payload: serde_json::Value =
                    serde_json::from_str(&input_json).unwrap_or(serde_json::Value::Null);
                if !crate::webhook::eval_condition(cond, &payload) {
                    // Condition not met — accepted but no execution started (202 Accepted)
                    return Err(ApiError {
                        status: StatusCode::ACCEPTED,
                        message: format!("filtered: condition not met ({cond})"),
                    });
                }
            }
        }

        // Apply optional payload transform script
        if let Some(script) = &webhook.payload_transform_script {
            if !script.is_empty() {
                input_json = crate::webhook::apply_payload_transform(script, &input_json);
            }
        }

        let version = state
            .workflow_service
            .get_version(&webhook.tenant_id, &webhook.workflow_version_id)
            .await?;

        let graph = resolve_graph_credentials(
            version.graph,
            &state.credential_store,
            &state.env_store,
            &webhook.tenant_id,
            DEFAULT_SET,
        )
        .await;

        let record = state
            .execution_service
            .start(StartExecutionRequest {
                tenant_id: webhook.tenant_id,
                workflow_id: webhook.workflow_id,
                workflow_version_id: webhook.workflow_version_id,
                graph,
                input_json,
                label: None,
                callback_url: None,
                trigger_type: Some("webhook".to_string()),
                dry_run: false,
                retried_from: None,
            })
            .await?;
        let prev_used = state
            .billing_store
            .billing_status(&record.tenant_id)
            .usage
            .executions_used;
        state.billing_store.increment_execution(&record.tenant_id);
        spawn_quota_alert(&state, &record.tenant_id, prev_used);
        state.audit_store.record(
            &record.tenant_id,
            audit_action::EXECUTION_STARTED,
            "execution",
            &record.id,
            None,
        );
        Ok(record)
    }
    .await;

    // Record delivery outcome regardless of success or failure
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let delivery = crate::webhook::WebhookDelivery {
        id: uuid::Uuid::new_v4().to_string(),
        webhook_token: delivery_token,
        tenant_id: delivery_tenant,
        delivered_at: now,
        status_code: match &inner_result {
            Ok(_) => Some(202),
            Err(e) => Some(e.status.as_u16() as i32),
        },
        success: inner_result.is_ok(),
        error_message: match &inner_result {
            Err(e) => Some(e.message.clone()),
            Ok(_) => None,
        },
        execution_id: match &inner_result {
            Ok(r) => Some(r.id.clone()),
            Err(_) => None,
        },
    };
    delivery_store.record_delivery(delivery).await;
    inner_result.map(|r| (StatusCode::ACCEPTED, Json(r)))
}

async fn list_schedules(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<CredentialQuery>,
) -> Json<Vec<crate::scheduler::ScheduleSummary>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(state.schedule_store.list(&tenant_id))
}

async fn pause_schedule_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(version_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_write(&claims)?;
    let found = state.schedule_store.set_paused(&version_id, true);
    if found {
        Ok(Json(
            serde_json::json!({"ok": true, "paused": true, "workflow_version_id": version_id}),
        ))
    } else {
        Err(ApiError::not_found("schedule"))
    }
}

async fn resume_schedule_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(version_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_write(&claims)?;
    let found = state.schedule_store.set_paused(&version_id, false);
    if found {
        Ok(Json(
            serde_json::json!({"ok": true, "paused": false, "workflow_version_id": version_id}),
        ))
    } else {
        Err(ApiError::not_found("schedule"))
    }
}

fn extract_trigger_interval(graph: &workflow_core::WorkflowGraph) -> Option<u64> {
    graph
        .nodes
        .iter()
        .find(|n| n.node_type == workflow_core::NodeType::Trigger)
        .and_then(|n| n.config.as_ref())
        .and_then(|c| c.get("interval_secs"))
        .and_then(|v| v.as_u64())
        .filter(|&secs| secs >= 60)
}

fn extract_trigger_cron(graph: &workflow_core::WorkflowGraph) -> Option<String> {
    graph
        .nodes
        .iter()
        .find(|n| n.node_type == workflow_core::NodeType::Trigger)
        .and_then(|n| n.config.as_ref())
        .and_then(|c| c.get("cron_expression"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .and_then(|s| crate::scheduler::cron_next_instant(s).map(|_| s.to_string()))
}

fn extract_trigger_webhook_secret(graph: &workflow_core::WorkflowGraph) -> Option<String> {
    graph
        .nodes
        .iter()
        .find(|n| n.node_type == workflow_core::NodeType::Trigger)
        .and_then(|n| n.config.as_ref())
        .and_then(|c| c.get("webhook_secret"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

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

#[derive(Debug, Deserialize)]
struct UpdateCredentialBody {
    #[serde(default)]
    tenant_id: String,
    value: Option<String>,
    description: Option<serde_json::Value>,
    expires_at: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ExpiringQuery {
    #[serde(default)]
    tenant_id: String,
    #[serde(default = "default_expiry_days")]
    within_days: u64,
}
fn default_expiry_days() -> u64 {
    30
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

#[derive(serde::Serialize)]
struct CredentialUsageEntry {
    workflow_id: String,
    workflow_name: String,
    version_id: String,
    version: u32,
}

#[derive(serde::Serialize)]
struct CredentialUsageResponse {
    usages: std::collections::HashMap<String, Vec<CredentialUsageEntry>>,
}

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

#[derive(Debug, Deserialize)]
struct EnvVarQuery {
    tenant_id: String,
    #[serde(default)]
    set: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EnvSetQuery {
    tenant_id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpsertEnvVarRequest {
    value: String,
}

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

async fn export_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<ExportWorkflowQuery>,
) -> Result<Json<WorkflowExport>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let workflow = state
        .workflow_service
        .get_workflow(&tenant_id, &workflow_id)
        .await?;
    let version_id = workflow
        .latest_version_id
        .ok_or(WorkflowError::NoPublishedVersion)?;
    let version = state
        .workflow_service
        .get_version(&tenant_id, &version_id)
        .await?;
    let exported_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(Json(WorkflowExport {
        name: workflow.name.clone(),
        description: workflow.description.clone(),
        readme: workflow.readme.clone(),
        tags: workflow.tags.clone(),
        graph: version.graph,
        exported_at,
    }))
}

async fn import_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<ImportWorkflowBody>,
) -> Result<(StatusCode, Json<WorkflowRecord>), ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let name = body
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Imported Workflow".to_string());
    let mut workflow = state
        .workflow_service
        .create_workflow(CreateWorkflowRequest {
            tenant_id: body.tenant_id.clone(),
            workspace_id: body.workspace_id,
            project_id: body.project_id,
            name,
            description: body.description.clone(),
            folder: None,
            created_by: claims.as_ref().and_then(|c| c.user_id.clone()),
        })
        .await?;
    // Restore readme and tags from export if provided
    if body.readme.is_some() || !body.tags.is_empty() {
        let update = UpdateWorkflowRequest {
            tenant_id: body.tenant_id.clone(),
            name: workflow.name.clone(),
            description: body.description.clone(),
            tags: if body.tags.is_empty() {
                None
            } else {
                Some(body.tags.clone())
            },
            readme: body.readme.clone(),
            folder: None,
            sla_seconds: None,
            max_runs_per_hour: None,
            max_concurrent_runs: None,
            budget_usd: None,
        };
        if let Ok(updated) = state
            .workflow_service
            .update_workflow(&workflow.id, update)
            .await
        {
            workflow = updated;
        }
    }
    let version = state
        .workflow_service
        .create_version(
            &workflow.id,
            CreateWorkflowVersionRequest {
                tenant_id: body.tenant_id.clone(),
                graph: body.graph,
                status: None,
                message: None,
            },
        )
        .await?;
    state.audit_store.record(
        &body.tenant_id,
        audit_action::WORKFLOW_CREATED,
        "workflow",
        &workflow.id,
        None,
    );
    workflow.latest_version_id = Some(version.id);
    Ok((StatusCode::CREATED, Json(workflow)))
}

// ── AI Workflow Generation ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GenerateWorkflowRequest {
    prompt: String,
    #[serde(default)]
    tenant_id: String,
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    model: Option<String>,
    /// If true, auto-creates and returns a WorkflowRecord; otherwise returns just the graph JSON.
    #[serde(default)]
    create: bool,
}

#[derive(Debug, Serialize)]
struct GenerateWorkflowResponse {
    graph: serde_json::Value,
    name: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow: Option<crate::workflow::WorkflowRecord>,
}

async fn generate_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<GenerateWorkflowRequest>,
) -> Result<(StatusCode, Json<GenerateWorkflowResponse>), ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);

    let api_key = body
        .api_key
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .ok_or_else(|| {
            ApiError::bad_request(
                "No Claude API key: provide api_key in request or set ANTHROPIC_API_KEY env var",
            )
        })?;
    let model = body
        .model
        .as_deref()
        .unwrap_or("claude-sonnet-4-6")
        .to_string();

    let system_prompt = r#"You are an expert workflow designer for Trigix, an AI-powered automation platform.

Generate a workflow graph JSON based on the user's description. Respond with ONLY valid JSON in this exact structure:
{
  "name": "Workflow name (concise)",
  "description": "One sentence description",
  "graph": {
    "workflow_version_id": "draft",
    "nodes": [
      { "id": "node_1", "type": "trigger", "config": {} },
      { "id": "node_2", "type": "...", "config": { ... } }
    ],
    "edges": [
      { "source": "node_1", "target": "node_2" }
    ]
  }
}

Available node types and their required config fields:
- trigger: {} (always start here)
- http: { url, method (GET/POST), headers?, body? }
- claude: { api_key, model (claude-sonnet-4-6), prompt_template, system_prompt?, max_tokens? }
- openai: { api_key, model (gpt-4o-mini), prompt_template, system_prompt?, max_tokens? }
- condition: { field (dot-path), operator (equals/not_equals/contains/gt/lt/exists/not_exists), value? }
- transform: { template (JSON with {{node_id.field}} placeholders) }
- filter: { items (expr), field, operator, value? }
- aggregate: { items (expr), operation (count/sum/avg/min/max/join/first/last), field? }
- delay: { delay_secs }
- slack: { webhook_url, message_template }
- github: { token, endpoint, method }
- jira: { base_url, email, token, endpoint, method, body? }
- notion: { token, endpoint, method, body? }
- database: { url, query }
- code: { code (Rhai script) }
- sub_workflow: { workflow_id }
- fan_out: {} (parallel split)
- fan_in: {} (wait for all parallel branches)
- assert: { condition, message? }
- validate: { source, schema }
- loop: { items, template? }
- extract: { source, path }
- merge: { fields: [{source, key?}] }
- catch: {} (error handler — connect with error edge from failing node)
- note: { text } (documentation only)

Template variables: {{input.field}}, {{node_id.field}}, {{credential.name}}, {{env.KEY}}
Edges: source → target. For condition nodes add condition_label: "true" or "false" on edges.

Rules:
- Always start with a trigger node as node_1
- Use descriptive node IDs like "fetch_data", "parse_response", "send_slack"
- Keep graphs focused — 3-8 nodes is ideal
- Use {{credential.name}} for sensitive values (API keys, tokens)
- Return ONLY the JSON, no explanation"#;

    let payload = serde_json::json!({
        "model": model,
        "max_tokens": 2048,
        "system": system_prompt,
        "messages": [{ "role": "user", "content": body.prompt }],
    });

    let http_client = reqwest::Client::new();
    let resp = http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude request failed: {e}")))?;

    if !resp.status().is_success() {
        let code = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(&format!("Claude API {code}: {text}")));
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude response parse error: {e}")))?;

    let raw_content = resp_json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    // Extract JSON from the response (may be wrapped in markdown code blocks)
    let json_str = if raw_content.contains("```") {
        raw_content
            .split("```")
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, s)| s.trim_start_matches("json").trim())
            .next()
            .unwrap_or(&raw_content)
            .to_string()
    } else {
        raw_content.clone()
    };

    let generated: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|_| ApiError::bad_request(&format!("Claude returned invalid JSON: {json_str}")))?;

    let name = generated["name"]
        .as_str()
        .unwrap_or("Generated Workflow")
        .to_string();
    let description = generated["description"].as_str().unwrap_or("").to_string();
    let graph = generated["graph"].clone();

    if graph.is_null() {
        return Err(ApiError::bad_request(
            "Claude response missing 'graph' field",
        ));
    }

    let mut workflow_record: Option<crate::workflow::WorkflowRecord> = None;

    if body.create {
        let wf = state
            .workflow_service
            .create_workflow(crate::workflow::CreateWorkflowRequest {
                tenant_id: body.tenant_id.clone(),
                workspace_id: body.workspace_id.unwrap_or_default(),
                project_id: body.project_id.unwrap_or_default(),
                name: name.clone(),
                description: Some(description.clone()),
                folder: None,
                created_by: claims.as_ref().and_then(|c| c.user_id.clone()),
            })
            .await?;

        // Deserialize the graph JSON into WorkflowGraph so create_version can store it
        let mut graph_val = graph.clone();
        if let Some(obj) = graph_val.as_object_mut() {
            obj.insert(
                "workflow_version_id".to_string(),
                serde_json::Value::String("draft".to_string()),
            );
        }
        let workflow_graph: workflow_core::WorkflowGraph = serde_json::from_value(graph_val)
            .map_err(|e| ApiError::bad_request(&format!("Invalid graph structure: {e}")))?;

        state
            .workflow_service
            .create_version(
                &wf.id,
                crate::workflow::CreateWorkflowVersionRequest {
                    tenant_id: body.tenant_id.clone(),
                    graph: workflow_graph,
                    status: None,
                    message: Some("Generated by AI".to_string()),
                },
            )
            .await?;

        state.audit_store.record(
            &body.tenant_id,
            audit_action::WORKFLOW_CREATED,
            "workflow",
            &wf.id,
            Some(serde_json::Value::String("ai_generated".to_string())),
        );

        workflow_record = Some(wf);
    }

    Ok((
        StatusCode::CREATED,
        Json(GenerateWorkflowResponse {
            graph,
            name,
            description,
            workflow: workflow_record,
        }),
    ))
}

fn default_model() -> String {
    "claude-sonnet-4-6".to_string()
}

#[derive(serde::Deserialize)]
struct CopilotRequest {
    message: String,
    #[serde(default)]
    graph_json: Option<String>,
    api_key: Option<String>,
    #[serde(default = "default_model")]
    model: String,
    tenant_id: String,
}

#[derive(serde::Serialize)]
struct CopilotResponse {
    reply: String,
}

async fn copilot_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CopilotRequest>,
) -> Result<Json<CopilotResponse>, ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);

    let api_key = body
        .api_key
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .ok_or_else(|| {
            ApiError::bad_request("No Claude API key: provide api_key or set ANTHROPIC_API_KEY")
        })?;

    let graph_context = if let Some(g) = &body.graph_json {
        format!("\n\nCurrent workflow graph (JSON):\n```json\n{}\n```", g)
    } else {
        String::new()
    };

    let system = format!(
        "You are an expert assistant for Trigix, an AI-powered workflow automation platform.\
\n\nYou help users understand, debug, and improve their workflows. You have deep knowledge of:\
\n- All 136 node types (trigger, http, claude, openai, gemini, slack, github, database, code, condition, loop, etc.)\
\n- Template variables: {{{{input.field}}}}, {{{{node_id.field}}}}, {{{{credential.name}}}}, {{{{env.KEY}}}}\
\n- Best practices for workflow design (error handling with catch nodes, validation, retry logic)\
\n- Integration patterns (webhooks, scheduled triggers, fan-out/fan-in parallelism)\
\n\nWhen asked to suggest changes, provide concrete, actionable advice with example node configs in JSON.\
\nKeep replies concise and practical — 2-5 sentences for simple questions, structured lists for complex ones.{}",
        graph_context
    );

    let payload = serde_json::json!({
        "model": body.model,
        "max_tokens": 1024,
        "system": system,
        "messages": [{ "role": "user", "content": body.message }],
    });

    let http_client = reqwest::Client::new();
    let resp = http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude request failed: {e}")))?;

    if !resp.status().is_success() {
        let code = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(&format!("Claude API {code}: {text}")));
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude response parse: {e}")))?;

    let reply = resp_json["content"][0]["text"]
        .as_str()
        .unwrap_or("(no response)")
        .trim()
        .to_string();

    let _ = state.audit_store.record(
        &body.tenant_id,
        "copilot.query",
        "copilot",
        &body.tenant_id,
        None,
    );

    Ok(Json(CopilotResponse { reply }))
}

async fn duplicate_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut body): Json<DuplicateWorkflowBody>,
) -> Result<(StatusCode, Json<WorkflowRecord>), ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let original = state
        .workflow_service
        .get_workflow(&body.tenant_id, &workflow_id)
        .await?;
    let new_name = format!("{} (copy)", original.name);
    let mut new_workflow = state
        .workflow_service
        .create_workflow(CreateWorkflowRequest {
            tenant_id: body.tenant_id.clone(),
            workspace_id: original.workspace_id,
            project_id: original.project_id,
            name: new_name,
            description: original.description,
            folder: original.folder,
            created_by: claims.as_ref().and_then(|c| c.user_id.clone()),
        })
        .await?;
    if let Some(version_id) = &original.latest_version_id {
        let version = state
            .workflow_service
            .get_version(&body.tenant_id, version_id)
            .await?;
        let dup_version = state
            .workflow_service
            .create_version(
                &new_workflow.id,
                CreateWorkflowVersionRequest {
                    tenant_id: body.tenant_id.clone(),
                    graph: version.graph,
                    status: None,
                    message: None,
                },
            )
            .await?;
        new_workflow.latest_version_id = Some(dup_version.id);
    }
    state.audit_store.record(
        &body.tenant_id,
        audit_action::WORKFLOW_DUPLICATED,
        "workflow",
        &new_workflow.id,
        Some(serde_json::json!({ "duplicated_from": workflow_id })),
    );
    Ok((StatusCode::CREATED, Json(new_workflow)))
}

async fn list_audit_log(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<AuditLogQuery>,
) -> Json<Vec<crate::audit::AuditEvent>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let limit = query.limit.unwrap_or(200).min(1000);
    let events = state.audit_store.list(&tenant_id, limit).await;
    let filtered = events
        .into_iter()
        .filter(|e| query.action.as_ref().map_or(true, |a| &e.action == a))
        .filter(|e| {
            query
                .resource_id
                .as_ref()
                .map_or(true, |r| &e.resource_id == r)
        })
        .collect();
    Json(filtered)
}

// ── Token Usage ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TokenUsageQuery {
    tenant_id: String,
    #[serde(default)]
    days: Option<u64>,
}

async fn get_token_usage_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<TokenUsageQuery>,
) -> Json<crate::token_usage::TokenUsageSummary> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let days = query.days.unwrap_or(30).min(365);
    let since = crate::execution::unix_now().saturating_sub(days * 86400);
    let summary = state.token_usage_store.summarize(&tenant_id, since).await;
    Json(summary)
}

// ── Execution stats ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ExecutionStats {
    total: u64,
    running: u64,
    waiting_approval: u64,
    succeeded: u64,
    failed: u64,
    cancelled: u64,
    by_trigger: std::collections::HashMap<String, u64>,
    avg_duration_secs: Option<f64>,
}

async fn execution_stats_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<TenantQuery>,
) -> Result<Json<ExecutionStats>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let all = state.execution_service.list(&tenant_id).await?;
    let mut stats = ExecutionStats {
        total: all.len() as u64,
        running: 0,
        waiting_approval: 0,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        by_trigger: std::collections::HashMap::new(),
        avg_duration_secs: None,
    };
    let mut duration_sum = 0u64;
    let mut duration_count = 0u64;
    for e in &all {
        match e.status {
            execution_core::ExecutionStatus::Running => stats.running += 1,
            execution_core::ExecutionStatus::WaitingApproval => stats.waiting_approval += 1,
            execution_core::ExecutionStatus::Succeeded => stats.succeeded += 1,
            execution_core::ExecutionStatus::Failed => stats.failed += 1,
            execution_core::ExecutionStatus::Cancelled => stats.cancelled += 1,
        }
        let trigger = e.trigger_type.as_deref().unwrap_or("manual").to_string();
        *stats.by_trigger.entry(trigger).or_insert(0) += 1;
        if let Some(fin) = e.finished_at {
            duration_sum += fin.saturating_sub(e.started_at);
            duration_count += 1;
        }
    }
    if duration_count > 0 {
        stats.avg_duration_secs = Some(duration_sum as f64 / duration_count as f64);
    }
    Ok(Json(stats))
}

// ── Node-type analytics ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TenantQuery {
    #[serde(default)]
    tenant_id: String,
}

#[derive(Debug, Serialize)]
struct NodeTypeStat {
    node_type: String,
    total: u64,
    succeeded: u64,
    failed: u64,
    skipped: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    avg_duration_ms: Option<u64>,
}

// ── Workflow health check ──────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct WorkflowHealthReport {
    workflow_id: String,
    status: String, // "healthy" | "warning" | "error"
    issues: Vec<WorkflowHealthIssue>,
    published_version_id: Option<String>,
    last_run_status: Option<String>,
    last_run_at: Option<u64>,
}

#[derive(Debug, Serialize)]
struct WorkflowHealthIssue {
    severity: String, // "error" | "warning"
    message: String,
}

async fn workflow_json_schema_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(q): Query<TenantQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &q.tenant_id);
    let workflow = state
        .workflow_service
        .get_workflow(&tenant_id, &workflow_id)
        .await?;
    let schema = if let Some(version_id) = &workflow.latest_version_id {
        state
            .workflow_service
            .get_version(&tenant_id, version_id)
            .await
            .ok()
            .map(|v| v.graph.input_schema)
            .unwrap_or_default()
    } else {
        vec![]
    };

    let mut properties = serde_json::Map::new();
    let mut required_fields: Vec<serde_json::Value> = vec![];

    for field in &schema {
        let json_type = match field.field_type.as_str() {
            "number" => "number",
            "boolean" => "boolean",
            "json" => "object",
            _ => "string",
        };
        let mut prop = serde_json::json!({ "type": json_type });
        if !field.description.is_empty() {
            prop["description"] = serde_json::Value::String(field.description.clone());
        }
        if let Some(ref default) = field.default_value {
            prop["default"] = serde_json::Value::String(default.clone());
        }
        properties.insert(field.key.clone(), prop);
        if field.required {
            required_fields.push(serde_json::Value::String(field.key.clone()));
        }
    }

    let mut result = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": workflow.name,
        "type": "object",
        "properties": properties,
    });
    if !required_fields.is_empty() {
        result["required"] = serde_json::Value::Array(required_fields);
    }
    Ok(Json(result))
}

async fn workflow_health_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<TenantQuery>,
) -> Result<Json<WorkflowHealthReport>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let mut issues: Vec<WorkflowHealthIssue> = Vec::new();

    // Get the workflow
    let wf = state
        .workflow_service
        .get_workflow(&tenant_id, &workflow_id)
        .await
        .map_err(|_| ApiError::not_found("Workflow not found"))?;

    // Check published version exists
    let published_version_id: Option<String> = {
        let versions = state
            .workflow_service
            .list_versions(&tenant_id, &workflow_id, None, None)
            .await
            .unwrap_or_default();
        versions
            .into_iter()
            .find(|v| v.status == "published")
            .map(|v| v.id)
    };

    if published_version_id.is_none() {
        issues.push(WorkflowHealthIssue {
            severity: "warning".into(),
            message: "No published version. Workflow cannot be triggered.".into(),
        });
    }

    // Check lock status
    if wf.locked {
        issues.push(WorkflowHealthIssue {
            severity: "warning".into(),
            message: "Workflow is locked. Edits are prevented.".into(),
        });
    }

    // Check credentials referenced in the published version's graph
    if let Some(ref vid) = published_version_id {
        if let Ok(ver) = state.workflow_service.get_version(&tenant_id, vid).await {
            let cred_names: Vec<String> = {
                let graph_str = serde_json::to_string(&ver.graph).unwrap_or_default();
                let mut names = Vec::new();
                let mut remaining = graph_str.as_str();
                while let Some(start) = remaining.find("{{credential.") {
                    remaining = &remaining[start + 13..];
                    if let Some(end) = remaining.find("}}") {
                        names.push(remaining[..end].to_string());
                        remaining = &remaining[end + 2..];
                    }
                }
                names.sort();
                names.dedup();
                names
            };
            for name in &cred_names {
                match state.credential_store.get_by_name(&tenant_id, name).await {
                    Ok(None) => issues.push(WorkflowHealthIssue {
                        severity: "error".into(),
                        message: format!(
                            "Credential '{}' referenced in graph but not found in store.",
                            name
                        ),
                    }),
                    Err(_) => {}
                    Ok(Some(_)) => {
                        // Credential found — check if expiring
                        let creds = state
                            .credential_store
                            .list(&tenant_id)
                            .await
                            .unwrap_or_default();
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        for cred in &creds {
                            if cred.name == *name {
                                if let Some(exp) = cred.expires_at {
                                    let days = (exp.saturating_sub(now)) / 86400;
                                    if exp <= now {
                                        issues.push(WorkflowHealthIssue {
                                            severity: "error".into(),
                                            message: format!("Credential '{}' has EXPIRED. Rotate it immediately.", name),
                                        });
                                    } else if days <= 7 {
                                        issues.push(WorkflowHealthIssue {
                                            severity: "warning".into(),
                                            message: format!(
                                                "Credential '{}' expires in {} day(s).",
                                                name, days
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Check last run status
    let all_execs = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let last_exec = all_execs
        .iter()
        .filter(|e| e.workflow_id == workflow_id)
        .max_by_key(|e| e.started_at);
    let last_run_status = last_exec.map(|e| format!("{:?}", e.status).to_lowercase());
    let last_run_at = last_exec.map(|e| e.started_at);

    if let Some(ref s) = last_run_status {
        if s == "failed" {
            issues.push(WorkflowHealthIssue {
                severity: "warning".into(),
                message: "Most recent execution failed.".into(),
            });
        }
    }

    let status = if issues.iter().any(|i| i.severity == "error") {
        "error"
    } else if !issues.is_empty() {
        "warning"
    } else {
        "healthy"
    };

    Ok(Json(WorkflowHealthReport {
        workflow_id,
        status: status.into(),
        issues,
        published_version_id,
        last_run_status,
        last_run_at,
    }))
}

async fn get_latest_execution_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<TenantQuery>,
) -> Result<Json<Option<ExecutionSummary>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let all = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let latest = all
        .into_iter()
        .filter(|e| e.workflow_id == workflow_id)
        .max_by_key(|e| e.started_at);
    Ok(Json(latest))
}

async fn node_type_analytics_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<TenantQuery>,
) -> Json<Vec<NodeTypeStat>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let execs = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let ids: Vec<String> = execs.iter().map(|e| e.id.clone()).collect();

    // Track sum+count for avg_duration_ms computation
    let mut map: std::collections::HashMap<String, NodeTypeStat> = std::collections::HashMap::new();
    let mut dur_sum: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut dur_count: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for id in ids {
        if let Ok(rec) = state.execution_service.get(&tenant_id, &id).await {
            for nr in &rec.node_results {
                let st = map.entry(nr.node_type.clone()).or_insert(NodeTypeStat {
                    node_type: nr.node_type.clone(),
                    total: 0,
                    succeeded: 0,
                    failed: 0,
                    skipped: 0,
                    avg_duration_ms: None,
                });
                st.total += 1;
                match nr.status {
                    execution_core::NodeStatus::Succeeded => st.succeeded += 1,
                    execution_core::NodeStatus::Failed => st.failed += 1,
                    execution_core::NodeStatus::Skipped => st.skipped += 1,
                    _ => {}
                }
                if nr.duration_ms > 0 {
                    *dur_sum.entry(nr.node_type.clone()).or_insert(0) += nr.duration_ms;
                    *dur_count.entry(nr.node_type.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    for (node_type, st) in map.iter_mut() {
        if let (Some(&sum), Some(&cnt)) = (dur_sum.get(node_type), dur_count.get(node_type)) {
            if cnt > 0 {
                st.avg_duration_ms = Some(sum / cnt);
            }
        }
    }
    let mut stats: Vec<_> = map.into_values().collect();
    stats.sort_by(|a, b| b.total.cmp(&a.total));
    Json(stats)
}

// ── Workflow dependency graph ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct WorkflowDepEdge {
    from_workflow_id: String,
    to_workflow_id: String,
    node_type: String,
}

#[derive(Debug, Serialize)]
struct WorkflowDepsResponse {
    edges: Vec<WorkflowDepEdge>,
}

async fn workflow_deps_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<TenantQuery>,
) -> Json<WorkflowDepsResponse> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let workflows = state
        .workflow_service
        .list_workflows(&tenant_id, None, None, None)
        .await
        .unwrap_or_default();
    let mut edges = Vec::new();

    for wf in &workflows {
        let Some(ref vid) = wf.latest_version_id else {
            continue;
        };
        let Ok(version) = state.workflow_service.get_version(&tenant_id, vid).await else {
            continue;
        };
        for node in &version.graph.nodes {
            let nt = format!("{:?}", node.node_type).to_lowercase();
            if nt != "subworkflow" && nt != "foreach" {
                continue;
            }
            let config = node.config.as_ref().unwrap_or(&serde_json::Value::Null);
            let Some(target_id) = config.get("workflow_id").and_then(|v| v.as_str()) else {
                continue;
            };
            if target_id != wf.id {
                edges.push(WorkflowDepEdge {
                    from_workflow_id: wf.id.clone(),
                    to_workflow_id: target_id.to_string(),
                    node_type: nt,
                });
            }
        }
    }

    Json(WorkflowDepsResponse { edges })
}

// ── Workflow Stats Analytics ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct WorkflowStatRow {
    workflow_id: String,
    total: u64,
    succeeded: u64,
    failed: u64,
    cancelled: u64,
    running: u64,
    avg_duration_secs: Option<f64>,
    last_run_at: Option<u64>,
}

#[derive(Debug, Serialize)]
struct WorkflowStatsAnalyticsResponse {
    rows: Vec<WorkflowStatRow>,
    since: u64,
}

#[derive(Debug, Deserialize)]
struct WorkflowStatsQuery {
    tenant_id: Option<String>,
    #[serde(default = "default_days")]
    days: u64,
}

fn default_days() -> u64 {
    30
}

async fn workflow_stats_analytics_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<WorkflowStatsQuery>,
) -> Json<WorkflowStatsAnalyticsResponse> {
    let tenant_id = effective_tenant_id(&claims, query.tenant_id.as_deref().unwrap_or(""));
    let executions = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let since = now_secs.saturating_sub(query.days * 86400);

    // Aggregate per workflow_id
    let mut map: std::collections::HashMap<String, WorkflowStatRow> =
        std::collections::HashMap::new();
    let mut duration_sum: std::collections::HashMap<String, (u64, f64)> =
        std::collections::HashMap::new();

    for ex in executions.iter().filter(|e| e.started_at >= since) {
        let entry = map
            .entry(ex.workflow_id.clone())
            .or_insert_with(|| WorkflowStatRow {
                workflow_id: ex.workflow_id.clone(),
                total: 0,
                succeeded: 0,
                failed: 0,
                cancelled: 0,
                running: 0,
                avg_duration_secs: None,
                last_run_at: None,
            });
        entry.total += 1;
        match ex.status {
            ExecutionStatus::Succeeded => entry.succeeded += 1,
            ExecutionStatus::Failed => entry.failed += 1,
            ExecutionStatus::Cancelled => entry.cancelled += 1,
            ExecutionStatus::Running | ExecutionStatus::WaitingApproval => entry.running += 1,
        }
        if let Some(fin) = ex.finished_at {
            if fin > ex.started_at {
                let dur = (fin - ex.started_at) as f64;
                let d = duration_sum
                    .entry(ex.workflow_id.clone())
                    .or_insert((0, 0.0));
                d.0 += 1;
                d.1 += dur;
            }
        }
        if entry.last_run_at.map_or(true, |t| ex.started_at > t) {
            entry.last_run_at = Some(ex.started_at);
        }
    }

    for (wf_id, (count, sum)) in &duration_sum {
        if let Some(row) = map.get_mut(wf_id) {
            row.avg_duration_secs = Some(sum / *count as f64);
        }
    }

    let mut rows: Vec<WorkflowStatRow> = map.into_values().collect();
    rows.sort_by(|a, b| b.total.cmp(&a.total));

    Json(WorkflowStatsAnalyticsResponse { rows, since })
}

// ── SLA breach analytics ──────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct SlaBreachEntry {
    execution_id: String,
    workflow_id: String,
    workflow_name: String,
    sla_seconds: u64,
    elapsed_seconds: u64,
    overage_seconds: u64,
    started_at: u64,
    finished_at: u64,
}

#[derive(serde::Serialize)]
struct SlaBreachesResponse {
    breaches: Vec<SlaBreachEntry>,
    total_workflows_with_sla: usize,
    compliance_rate: f64,
    total_completed: usize,
}

async fn sla_breaches_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<WorkflowStatsQuery>,
) -> Json<SlaBreachesResponse> {
    let tenant_id = effective_tenant_id(&claims, query.tenant_id.as_deref().unwrap_or(""));

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let since = now_secs.saturating_sub(query.days * 86400);

    // fetch workflows with SLA set
    let workflows = state
        .workflow_service
        .list_workflows(&tenant_id, None, None, None)
        .await
        .unwrap_or_default();
    let sla_map: std::collections::HashMap<String, (String, u64)> = workflows
        .iter()
        .filter_map(|w| w.sla_seconds.map(|s| (w.id.clone(), (w.name.clone(), s))))
        .collect();

    if sla_map.is_empty() {
        return Json(SlaBreachesResponse {
            breaches: vec![],
            total_workflows_with_sla: 0,
            compliance_rate: 100.0,
            total_completed: 0,
        });
    }

    let executions = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();

    let mut breaches = Vec::new();
    let mut total_completed = 0usize;
    let mut total_compliant = 0usize;

    for ex in executions.iter().filter(|e| e.started_at >= since) {
        if let Some((wf_name, sla)) = sla_map.get(&ex.workflow_id) {
            if ex.status == ExecutionStatus::Succeeded || ex.status == ExecutionStatus::Failed {
                if let Some(fin) = ex.finished_at {
                    total_completed += 1;
                    let elapsed = fin.saturating_sub(ex.started_at);
                    if elapsed > *sla {
                        breaches.push(SlaBreachEntry {
                            execution_id: ex.id.clone(),
                            workflow_id: ex.workflow_id.clone(),
                            workflow_name: wf_name.clone(),
                            sla_seconds: *sla,
                            elapsed_seconds: elapsed,
                            overage_seconds: elapsed - sla,
                            started_at: ex.started_at,
                            finished_at: fin,
                        });
                    } else {
                        total_compliant += 1;
                    }
                }
            }
        }
    }

    breaches.sort_by(|a, b| b.overage_seconds.cmp(&a.overage_seconds));

    let compliance_rate = if total_completed > 0 {
        (total_compliant as f64 / total_completed as f64) * 100.0
    } else {
        100.0
    };

    Json(SlaBreachesResponse {
        breaches,
        total_workflows_with_sla: sla_map.len(),
        compliance_rate,
        total_completed,
    })
}

// ── Error analysis analytics ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct TopErrorEntry {
    error_message: String,
    count: usize,
    node_type: String,
    workflow_id: String,
    workflow_name: String,
    last_seen: u64,
}

#[derive(Debug, Serialize)]
struct ErrorAnalysisResponse {
    top_errors: Vec<TopErrorEntry>,
    total_failed_nodes: usize,
    distinct_error_types: usize,
}

async fn error_analysis_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<WorkflowStatsQuery>,
) -> Json<ErrorAnalysisResponse> {
    let tenant_id = effective_tenant_id(&claims, query.tenant_id.as_deref().unwrap_or(""));
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let since = now_secs.saturating_sub(query.days * 86400);

    let workflows = state
        .workflow_service
        .list_workflows(&tenant_id, None, None, None)
        .await
        .unwrap_or_default();
    let wf_name_map: HashMap<String, String> = workflows
        .iter()
        .map(|w| (w.id.clone(), w.name.clone()))
        .collect();

    let executions = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let recent_failed: Vec<_> = executions
        .iter()
        .filter(|e| e.started_at >= since && matches!(e.status, ExecutionStatus::Failed))
        .collect();

    // key = truncated error message (first 120 chars)
    let mut error_map: HashMap<String, TopErrorEntry> = HashMap::new();
    let mut total_failed_nodes = 0usize;

    for ex in &recent_failed {
        if let Ok(full) = state.execution_service.get(&tenant_id, &ex.id).await {
            for nr in &full.node_results {
                if matches!(nr.status, execution_core::NodeStatus::Failed) {
                    total_failed_nodes += 1;
                    let raw = nr.error.as_deref().unwrap_or("unknown error");
                    let key: String = raw.chars().take(120).collect();
                    let entry = error_map.entry(key.clone()).or_insert(TopErrorEntry {
                        error_message: key,
                        count: 0,
                        node_type: nr.node_type.clone(),
                        workflow_id: ex.workflow_id.clone(),
                        workflow_name: wf_name_map
                            .get(&ex.workflow_id)
                            .cloned()
                            .unwrap_or_default(),
                        last_seen: 0,
                    });
                    entry.count += 1;
                    if ex.started_at > entry.last_seen {
                        entry.last_seen = ex.started_at;
                    }
                }
            }
        }
    }

    let distinct_error_types = error_map.len();
    let mut top_errors: Vec<_> = error_map.into_values().collect();
    top_errors.sort_by(|a, b| b.count.cmp(&a.count));
    top_errors.truncate(20);

    Json(ErrorAnalysisResponse {
        top_errors,
        total_failed_nodes,
        distinct_error_types,
    })
}

// ── Variables ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct VariableQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct SetVariableBody {
    value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct IncrementVariableBody {
    #[serde(default = "default_increment_by")]
    by: f64,
}

fn default_increment_by() -> f64 {
    1.0
}

async fn list_variables_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<VariableQuery>,
) -> Json<Vec<crate::variables::Variable>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(state.variable_store.list(&tenant_id, &workflow_id).await)
}

async fn get_variable_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, key)): Path<(String, String)>,
    Query(query): Query<VariableQuery>,
) -> Result<Json<crate::variables::Variable>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .variable_store
        .get(&tenant_id, &workflow_id, &key)
        .await
        .map(Json)
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "VariableNotFound".to_string(),
        })
}

async fn set_variable_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, key)): Path<(String, String)>,
    Query(query): Query<VariableQuery>,
    Json(body): Json<SetVariableBody>,
) -> Json<crate::variables::Variable> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(
        state
            .variable_store
            .set(&tenant_id, &workflow_id, &key, body.value)
            .await,
    )
}

async fn delete_variable_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, key)): Path<(String, String)>,
    Query(query): Query<VariableQuery>,
) -> StatusCode {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    if state
        .variable_store
        .delete(&tenant_id, &workflow_id, &key)
        .await
    {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn increment_variable_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, key)): Path<(String, String)>,
    Query(query): Query<VariableQuery>,
    Json(body): Json<IncrementVariableBody>,
) -> Json<crate::variables::Variable> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(
        state
            .variable_store
            .increment(&tenant_id, &workflow_id, &key, body.by)
            .await,
    )
}

// ── Workspace / Project ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct WorkspaceQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct CreateWorkspaceBody {
    tenant_id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateProjectBody {
    tenant_id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
}

async fn list_workspaces_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<Vec<crate::workspace::WorkspaceRecord>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(state.workspace_store.list_workspaces(&tenant_id).await)
}

async fn create_workspace_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CreateWorkspaceBody>,
) -> (StatusCode, Json<crate::workspace::WorkspaceRecord>) {
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let record = state
        .workspace_store
        .create_workspace(&body.tenant_id, &body.name, body.description.as_deref())
        .await;
    (StatusCode::CREATED, Json(record))
}

async fn delete_workspace_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workspace_id): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> StatusCode {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    if state
        .workspace_store
        .delete_workspace(&tenant_id, &workspace_id)
        .await
    {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn list_projects_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workspace_id): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<Vec<crate::workspace::ProjectRecord>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(
        state
            .workspace_store
            .list_projects(&tenant_id, &workspace_id)
            .await,
    )
}

async fn create_project_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workspace_id): Path<String>,
    Json(mut body): Json<CreateProjectBody>,
) -> Result<(StatusCode, Json<crate::workspace::ProjectRecord>), ApiError> {
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let record = state
        .workspace_store
        .create_project(
            &body.tenant_id,
            &workspace_id,
            &body.name,
            body.description.as_deref(),
        )
        .await
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "WorkspaceNotFound".to_string(),
        })?;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn delete_project_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(project_id): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> StatusCode {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    if state
        .workspace_store
        .delete_project(&tenant_id, &project_id)
        .await
    {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn list_webhooks_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<CredentialQuery>,
) -> Result<Json<Vec<crate::webhook::WebhookRecord>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let records = state
        .webhook_store
        .list_by_tenant(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(records))
}

async fn delete_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
    Query(query): Query<CredentialQuery>,
) -> Result<StatusCode, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .webhook_store
        .delete_by_token(&tenant_id, &token)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct DeliveryQuery {
    limit: Option<i64>,
}

async fn list_webhook_deliveries_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(q): Query<DeliveryQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let deliveries = state.webhook_store.list_deliveries(&token, limit).await;
    Json(deliveries)
}

#[derive(Debug, Deserialize)]
struct SetWebhookConditionBody {
    condition_expr: Option<String>,
}

async fn update_webhook_condition_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
    Json(body): Json<SetWebhookConditionBody>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_condition(&tenant_id, &token, body.condition_expr)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

#[derive(Debug, Deserialize)]
struct SetWebhookRateLimitBody {
    max_calls_per_minute: Option<u32>,
}

async fn update_webhook_rate_limit_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
    Json(body): Json<SetWebhookRateLimitBody>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_rate_limit(&tenant_id, &token, body.max_calls_per_minute)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

async fn rotate_webhook_secret_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    let new_secret = format!(
        "{}{}",
        uuid::Uuid::new_v4().to_string().replace('-', ""),
        uuid::Uuid::new_v4().to_string().replace('-', ""),
    );
    state
        .webhook_store
        .rotate_secret(&tenant_id, &token, new_secret.clone())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(serde_json::json!({ "secret": new_secret })))
}

async fn pause_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_paused(&tenant_id, &token, true)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

async fn resume_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_paused(&tenant_id, &token, false)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

#[derive(Deserialize)]
struct SetPayloadTransformBody {
    script: Option<String>,
}

async fn set_payload_transform_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
    Json(body): Json<SetPayloadTransformBody>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_payload_transform(&tenant_id, &token, body.script)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

async fn method_not_allowed() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

#[derive(Debug, Deserialize)]
struct TokenRequest {
    api_key: String,
    tenant_id: Option<String>,
    workspace_id: Option<String>,
    project_id: Option<String>,
    role: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    token: String,
    tenant_id: String,
    workspace_id: String,
    project_id: String,
    role: String,
}

async fn create_token(
    State(state): State<AppState>,
    Json(body): Json<TokenRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let role: crate::auth::Role = body
        .role
        .as_deref()
        .and_then(|r| r.parse().ok())
        .unwrap_or_default();

    // First check stored API keys (takes precedence so tenant_id is enforced).
    if let Some(stored) = state.api_key_store.validate(&body.api_key).await {
        let tenant_id = stored.tenant_id.clone();
        let workspace_id = body
            .workspace_id
            .unwrap_or_else(|| "workspace-1".to_string());
        let project_id = body.project_id.unwrap_or_else(|| "project-1".to_string());
        let exp = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + 7 * 24 * 3600;
        let role_str = role.as_str().to_string();
        let claims = Claims {
            sub: tenant_id.clone(),
            tenant_id: tenant_id.clone(),
            workspace_id: workspace_id.clone(),
            project_id: project_id.clone(),
            exp,
            role,
            user_id: None,
            email: None,
        };
        let token = sign_token(&claims).map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Failed to sign token".to_string(),
        })?;
        return Ok(Json(TokenResponse {
            token,
            tenant_id,
            workspace_id,
            project_id,
            role: role_str,
        }));
    }

    // Fall back to the static DEV_API_KEY.
    let expected_key = std::env::var("DEV_API_KEY").unwrap_or_else(|_| "dev".to_string());
    if body.api_key != expected_key {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "Invalid api_key".to_string(),
        });
    }
    let tenant_id = body.tenant_id.unwrap_or_else(|| "tenant-1".to_string());
    let workspace_id = body
        .workspace_id
        .unwrap_or_else(|| "workspace-1".to_string());
    let project_id = body.project_id.unwrap_or_else(|| "project-1".to_string());
    let exp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 7 * 24 * 3600;
    let role_str = role.as_str().to_string();
    let claims = Claims {
        sub: tenant_id.clone(),
        tenant_id: tenant_id.clone(),
        workspace_id: workspace_id.clone(),
        project_id: project_id.clone(),
        exp,
        role,
        user_id: None,
        email: None,
    };
    let token = sign_token(&claims).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Failed to sign token".to_string(),
    })?;
    Ok(Json(TokenResponse {
        token,
        tenant_id,
        workspace_id,
        project_id,
        role: role_str,
    }))
}

// ── User auth (register / login / me) ─────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    email: String,
    password: String,
    name: Option<String>,
    tenant_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    token: String,
    user: crate::users::PublicUser,
}

fn make_user_token(user: &crate::users::User) -> Result<String, ApiError> {
    let exp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 7 * 24 * 3600;
    let claims = Claims {
        sub: user.id.clone(),
        tenant_id: user.tenant_id.clone(),
        workspace_id: "workspace-1".to_string(),
        project_id: "project-1".to_string(),
        exp,
        role: crate::auth::Role::Editor,
        user_id: Some(user.id.clone()),
        email: Some(user.email.clone()),
    };
    sign_token(&claims).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Failed to sign token".to_string(),
    })
}

async fn register_user(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let tenant_id = body.tenant_id.unwrap_or_else(|| "tenant-1".to_string());
    let store = &state.user_store;
    let user = tokio::task::spawn_blocking({
        let email = body.email.clone();
        let password = body.password.clone();
        let name = body.name.clone();
        let store = Arc::clone(store);
        move || store.create(&email, &password, name.as_deref(), &tenant_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .map_err(|e| match e {
        crate::users::UserError::EmailAlreadyExists => ApiError {
            status: StatusCode::CONFLICT,
            message: "Email already registered".to_string(),
        },
        other => ApiError {
            status: StatusCode::BAD_REQUEST,
            message: other.to_string(),
        },
    })?;

    // Fire verification email non-blocking
    {
        let ver_store = Arc::clone(&state.verification_store);
        let email_client = Arc::clone(&state.email_client);
        let uid = user.id.clone();
        let em = user.email.clone();
        tokio::spawn(async move {
            let ver = tokio::task::spawn_blocking(move || ver_store.create(&uid, &em, 24))
                .await
                .ok();
            if let Some(ver) = ver {
                email_client
                    .send_email_verification(&ver.email, &ver.token, ver.expires_at)
                    .await;
            }
        });
    }

    let token = make_user_token(&user)?;
    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            user: crate::users::PublicUser::from(&user),
        }),
    ))
}

async fn login_user(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let store = Arc::clone(&state.user_store);
    let user = tokio::task::spawn_blocking({
        let email = body.email.clone();
        let password = body.password.clone();
        move || store.verify_password(&email, &password)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .map_err(|e| match e {
        crate::users::UserError::InvalidCredentials => ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "Invalid email or password".to_string(),
        },
        other => ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: other.to_string(),
        },
    })?;

    let token = make_user_token(&user)?;
    Ok(Json(AuthResponse {
        token,
        user: crate::users::PublicUser::from(&user),
    }))
}

// ── Enterprise SSO (OIDC) ───────────────────────────────────────────────────

fn sso_redirect_base() -> String {
    std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:38080".to_string())
}

fn sso_frontend_url() -> String {
    std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3100".to_string())
}

fn sso_callback_uri(slug: &str) -> String {
    format!(
        "{}/v1/sso/{}/callback",
        sso_redirect_base().trim_end_matches('/'),
        slug
    )
}

/// Public list of enabled SSO connections — used to render login buttons.
async fn sso_public_handler(
    State(state): State<AppState>,
) -> Json<Vec<crate::sso::PublicSsoConnection>> {
    Json(state.sso_store.list_enabled_public().await)
}

#[derive(Deserialize)]
struct CreateSsoBody {
    slug: String,
    provider: String,
    /// "oidc" (default), "feishu", "dingtalk", or "wechat_work".
    kind: Option<String>,
    /// Required for OIDC; ignored by the custom-OAuth2 providers.
    issuer: Option<String>,
    client_id: String,
    client_secret: String,
    /// WeChat Work agent id (only used when kind == "wechat_work").
    agent_id: Option<String>,
    scopes: Option<String>,
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

#[derive(Deserialize)]
struct UpdateSsoBody {
    enabled: bool,
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

fn ai_runtime_base() -> Result<String, ApiError> {
    std::env::var("AI_RUNTIME_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches('/').to_string())
        .ok_or_else(|| ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "AI_RUNTIME_BASE_URL is not configured".to_string(),
        })
}

async fn rag_forward_json(
    builder: reqwest::RequestBuilder,
) -> Result<Json<serde_json::Value>, ApiError> {
    let resp = builder.send().await.map_err(|e| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message: format!("AI runtime unreachable: {e}"),
    })?;
    let status = resp.status();
    let val: serde_json::Value = resp.json().await.map_err(|e| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message: format!("AI runtime returned a bad response: {e}"),
    })?;
    if !status.is_success() {
        return Err(ApiError {
            status: StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            message: val
                .get("detail")
                .and_then(|d| d.as_str())
                .unwrap_or("AI runtime error")
                .to_string(),
        });
    }
    Ok(Json(val))
}

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

#[derive(Deserialize)]
struct RagDocsQuery {
    kb: String,
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

#[derive(Deserialize)]
struct RagIngestBody {
    kb: String,
    doc_id: String,
    text: String,
    #[serde(default)]
    chunk_size: Option<u32>,
    #[serde(default)]
    overlap: Option<u32>,
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

fn sso_redirect(location: &str) -> Response {
    Response::builder()
        .status(StatusCode::FOUND)
        .header("Location", location)
        .body(axum::body::Body::empty())
        .unwrap()
}

fn sso_error_redirect(message: &str) -> Response {
    let url = format!(
        "{}/?sso_error={}",
        sso_frontend_url().trim_end_matches('/'),
        urlencode(message)
    );
    sso_redirect(&url)
}

/// Percent-encode a string for safe inclusion in a URL query value.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `GET /v1/sso/:slug/login` — begin SP-initiated OIDC login (redirect to IdP).
/// Provision (find-or-create) the SSO user in `tenant_id`, issue a Trigix JWT,
/// and redirect the browser back to the SPA with it. Shared by the OIDC and the
/// custom-OAuth2 callback paths.
async fn sso_finish_login(
    state: &AppState,
    tenant_id: &str,
    email: &str,
    name: Option<String>,
) -> Response {
    let store = Arc::clone(&state.user_store);
    let email = email.to_lowercase();
    let tenant = tenant_id.to_string();
    let user = tokio::task::spawn_blocking(move || {
        if let Some(u) = store.find_by_email(&email) {
            return Ok(u);
        }
        let random_pw = uuid::Uuid::new_v4().to_string();
        let created = store.create(&email, &random_pw, name.as_deref(), &tenant)?;
        // Identity is asserted by the IdP, so mark the email verified.
        let _ = store.mark_email_verified(&created.id);
        store
            .find_by_id(&created.id)
            .ok_or(crate::users::UserError::NotFound)
    })
    .await;
    let user = match user {
        Ok(Ok(u)) => u,
        _ => return sso_error_redirect("failed to provision SSO user"),
    };
    let token = match make_user_token(&user) {
        Ok(t) => t,
        Err(_) => return sso_error_redirect("failed to issue session token"),
    };
    let dest = format!(
        "{}/?sso_token={}",
        sso_frontend_url().trim_end_matches('/'),
        urlencode(&token)
    );
    sso_redirect(&dest)
}

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

#[derive(Deserialize)]
struct SsoCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
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

async fn me_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<crate::users::PublicUser>, ApiError> {
    let user_id = claims
        .as_ref()
        .and_then(|c| c.user_id.as_deref())
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "Not authenticated as a user".to_string(),
        })?;

    let store = Arc::clone(&state.user_store);
    let user = tokio::task::spawn_blocking(move || store.find_by_id(&user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "User not found".to_string(),
        })?;

    Ok(Json(crate::users::PublicUser::from(&user)))
}

#[derive(Debug, Deserialize)]
struct UpdateMeBody {
    name: Option<String>,
    current_password: Option<String>,
    new_password: Option<String>,
}

async fn update_me_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<UpdateMeBody>,
) -> Result<Json<crate::users::PublicUser>, ApiError> {
    let user_id = require_user_id(&claims)?;

    if body.new_password.is_some() && body.current_password.is_none() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "current_password required to change password".to_string(),
        });
    }

    if let (Some(old_pw), Some(new_pw)) = (
        body.current_password.as_deref(),
        body.new_password.as_deref(),
    ) {
        let store = Arc::clone(&state.user_store);
        let uid = user_id.clone();
        let old_pw = old_pw.to_string();
        let new_pw = new_pw.to_string();
        tokio::task::spawn_blocking(move || store.update_password(&uid, &old_pw, &new_pw))
            .await
            .map_err(|_| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "Task join error".to_string(),
            })?
            .map_err(|e| match e {
                crate::users::UserError::InvalidCredentials => ApiError {
                    status: StatusCode::UNAUTHORIZED,
                    message: "Current password is incorrect".to_string(),
                },
                other => ApiError {
                    status: StatusCode::BAD_REQUEST,
                    message: other.to_string(),
                },
            })?;
    }

    if let Some(name) = body.name.as_deref() {
        let store = Arc::clone(&state.user_store);
        let uid = user_id.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || store.update_name(&uid, &name))
            .await
            .map_err(|_| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "Task join error".to_string(),
            })?
            .map_err(|e| ApiError {
                status: StatusCode::BAD_REQUEST,
                message: e.to_string(),
            })?;
    }

    let store = Arc::clone(&state.user_store);
    let uid = user_id.clone();
    let user = tokio::task::spawn_blocking(move || store.find_by_id(&uid))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "User not found".to_string(),
        })?;

    Ok(Json(crate::users::PublicUser::from(&user)))
}

async fn get_notifications_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<crate::notification_prefs::NotificationPrefs>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let prefs_store = Arc::clone(&state.notification_prefs_store);
    let prefs = tokio::task::spawn_blocking(move || prefs_store.get(&user_id))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?;
    Ok(Json(prefs))
}

#[derive(Debug, Deserialize)]
struct UpdateNotificationsBody {
    #[serde(default)]
    email_on_failure: bool,
    #[serde(default)]
    email_on_success: bool,
}

async fn put_notifications_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<UpdateNotificationsBody>,
) -> Result<Json<crate::notification_prefs::NotificationPrefs>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let prefs = crate::notification_prefs::NotificationPrefs {
        user_id: user_id.clone(),
        email_on_failure: body.email_on_failure,
        email_on_success: body.email_on_success,
    };
    let prefs_store = Arc::clone(&state.notification_prefs_store);
    let prefs_clone = prefs.clone();
    tokio::task::spawn_blocking(move || prefs_store.upsert(prefs_clone))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?;
    Ok(Json(prefs))
}

// ── Admin: user management ────────────────────────────────────────────────

async fn admin_list_users_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<crate::users::PublicUser>>, ApiError> {
    require_admin(&claims)?;
    let tenant_id = claims
        .as_ref()
        .map(|c| c.tenant_id.as_str())
        .unwrap_or("tenant-1")
        .to_string();
    let store = Arc::clone(&state.user_store);
    let users = tokio::task::spawn_blocking(move || store.list_by_tenant(&tenant_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(Json(users))
}

async fn admin_delete_user_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    axum::extract::Path(user_id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let caller_id = require_user_id(&claims)?;
    if user_id == caller_id {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Cannot delete your own account".to_string(),
        });
    }
    let store = Arc::clone(&state.user_store);
    tokio::task::spawn_blocking(move || store.delete_user(&user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|e| match e {
            crate::users::UserError::NotFound => ApiError {
                status: StatusCode::NOT_FOUND,
                message: "User not found".to_string(),
            },
            other => ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: other.to_string(),
            },
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Admin: invitations ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CreateInvitationBody {
    email: String,
    #[serde(default = "default_editor_role")]
    role: String,
    #[serde(default = "default_invite_ttl_hours")]
    expires_hours: u64,
}

fn default_invite_ttl_hours() -> u64 {
    72
}

async fn admin_create_invitation_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CreateInvitationBody>,
) -> Result<(StatusCode, Json<crate::invitations::Invitation>), ApiError> {
    require_admin(&claims)?;
    let tenant_id = claims
        .as_ref()
        .map(|c| c.tenant_id.as_str())
        .unwrap_or("tenant-1")
        .to_string();
    let store = Arc::clone(&state.invite_store);
    let email = body.email.clone();
    let role = body.role.clone();
    let expires_hours = body.expires_hours;
    let inv =
        tokio::task::spawn_blocking(move || store.create(&email, &role, &tenant_id, expires_hours))
            .await
            .map_err(|_| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "Task join error".to_string(),
            })?;
    // Send invitation email (non-blocking, best-effort)
    let email_client = Arc::clone(&state.email_client);
    let inv_token = inv.token.clone();
    let inv_email = inv.email.clone();
    let inv_role = inv.role.clone();
    let inv_expires = inv.expires_at;
    tokio::spawn(async move {
        email_client
            .send_invitation(&inv_email, &inv_token, &inv_role, inv_expires)
            .await;
    });
    Ok((StatusCode::CREATED, Json(inv)))
}

async fn admin_list_invitations_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<crate::invitations::Invitation>>, ApiError> {
    require_admin(&claims)?;
    let tenant_id = claims
        .as_ref()
        .map(|c| c.tenant_id.as_str())
        .unwrap_or("tenant-1")
        .to_string();
    let store = Arc::clone(&state.invite_store);
    let list = tokio::task::spawn_blocking(move || store.list_by_tenant(&tenant_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(Json(list))
}

async fn admin_delete_invitation_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    axum::extract::Path(invite_id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let store = Arc::clone(&state.invite_store);
    tokio::task::spawn_blocking(move || store.delete(&invite_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Invitation not found".to_string(),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// Public: look up an invite by token (used by frontend before showing accept form)
async fn get_invitation_handler(
    State(state): State<AppState>,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = Arc::clone(&state.invite_store);
    let inv = tokio::task::spawn_blocking(move || store.find_by_token(&token))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Invitation not found".to_string(),
        })?;
    if !inv.is_valid() {
        return Err(ApiError {
            status: StatusCode::GONE,
            message: "Invitation has expired or already been used".to_string(),
        });
    }
    Ok(Json(
        serde_json::json!({ "email": inv.email, "role": inv.role, "valid": true }),
    ))
}

#[derive(Debug, Deserialize)]
struct AcceptInviteBody {
    token: String,
    password: String,
    name: Option<String>,
}

async fn accept_invite_handler(
    State(state): State<AppState>,
    Json(body): Json<AcceptInviteBody>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    // Validate and consume the invite
    let invite_store = Arc::clone(&state.invite_store);
    let token = body.token.clone();
    let inv = tokio::task::spawn_blocking(move || invite_store.mark_used(&token))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|e| match e {
            crate::invitations::InviteError::NotFound => ApiError {
                status: StatusCode::NOT_FOUND,
                message: "Invitation not found".to_string(),
            },
            crate::invitations::InviteError::AlreadyUsed => ApiError {
                status: StatusCode::GONE,
                message: "Invitation already used".to_string(),
            },
            crate::invitations::InviteError::Expired => ApiError {
                status: StatusCode::GONE,
                message: "Invitation has expired".to_string(),
            },
        })?;

    // Register the user with the invited email + tenant
    let user_store = Arc::clone(&state.user_store);
    let email = inv.email.clone();
    let password = body.password.clone();
    let name = body.name.clone();
    let tenant_id = inv.tenant_id.clone();
    let user = tokio::task::spawn_blocking(move || {
        user_store.create(&email, &password, name.as_deref(), &tenant_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .map_err(|e| match e {
        crate::users::UserError::EmailAlreadyExists => ApiError {
            status: StatusCode::CONFLICT,
            message: "Email already registered".to_string(),
        },
        other => ApiError {
            status: StatusCode::BAD_REQUEST,
            message: other.to_string(),
        },
    })?;

    let token = make_user_token(&user)?;
    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            user: crate::users::PublicUser::from(&user),
        }),
    ))
}

// ── Password reset ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ForgotPasswordBody {
    email: String,
}

#[derive(serde::Serialize)]
struct ForgotPasswordResponse {
    message: String,
    /// The reset token (only included in dev/test environments for convenience).
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    expires_at: i64,
}

async fn forgot_password_handler(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordBody>,
) -> Result<Json<ForgotPasswordResponse>, ApiError> {
    let email = body.email.trim().to_lowercase();
    let user_store = Arc::clone(&state.user_store);
    let em = email.clone();
    let user = tokio::task::spawn_blocking(move || user_store.find_by_email(&em))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?;

    // Always return 200 to avoid email enumeration
    let (token_val, expires_at) = match user {
        Some(u) => {
            let reset_store = Arc::clone(&state.reset_store);
            let uid = u.id.clone();
            let em2 = email.clone();
            let reset = tokio::task::spawn_blocking(move || reset_store.create(&uid, &em2, 2))
                .await
                .map_err(|_| ApiError::internal("Task join error"))?;
            let tok = reset.token.clone();
            let exp = reset.expires_at;
            state
                .email_client
                .send_password_reset(&email, &tok, exp)
                .await;
            (Some(tok), exp)
        }
        None => {
            let exp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
                + 7200;
            (None, exp)
        }
    };

    // In prod/auth-required mode suppress the token from the response
    let expose_token = !auth_required();
    Ok(Json(ForgotPasswordResponse {
        message: "If an account exists with that email, a reset link has been sent.".to_string(),
        token: if expose_token { token_val } else { None },
        expires_at,
    }))
}

#[derive(Debug, Deserialize)]
struct ResetPasswordBody {
    token: String,
    new_password: String,
}

async fn reset_password_handler(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.new_password.len() < 6 {
        return Err(ApiError::bad_request(
            "Password must be at least 6 characters",
        ));
    }
    let reset_store = Arc::clone(&state.reset_store);
    let token = body.token.clone();
    let reset = tokio::task::spawn_blocking(move || reset_store.mark_used(&token))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?
        .map_err(|e| match e {
            crate::password_reset::ResetError::NotFound => {
                ApiError::not_found("Reset token not found or already used")
            }
            crate::password_reset::ResetError::AlreadyUsed => ApiError {
                status: StatusCode::GONE,
                message: "Reset token already used".to_string(),
            },
            crate::password_reset::ResetError::Expired => ApiError {
                status: StatusCode::GONE,
                message: "Reset token has expired".to_string(),
            },
            crate::password_reset::ResetError::StoreUnavailable => {
                ApiError::internal("Store unavailable")
            }
        })?;

    let user_store = Arc::clone(&state.user_store);
    let uid = reset.user_id.clone();
    let new_pw = body.new_password.clone();
    tokio::task::spawn_blocking(move || user_store.reset_password(&uid, &new_pw))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?
        .map_err(|_| ApiError::internal("Failed to update password"))?;

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "Password updated successfully" }),
    ))
}

#[derive(Debug, Deserialize)]
struct VerifyEmailBody {
    token: String,
}

async fn verify_email_handler(
    State(state): State<AppState>,
    Json(body): Json<VerifyEmailBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    use crate::email_verification::VerificationError;
    let ver_store = Arc::clone(&state.verification_store);
    let token = body.token.clone();
    let ver = tokio::task::spawn_blocking(move || ver_store.mark_used(&token))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?
        .map_err(|e| match e {
            VerificationError::NotFound => {
                ApiError::not_found("Verification token not found or already used")
            }
            VerificationError::AlreadyUsed => ApiError {
                status: StatusCode::GONE,
                message: "Verification token already used".to_string(),
            },
            VerificationError::Expired => ApiError {
                status: StatusCode::GONE,
                message: "Verification token has expired".to_string(),
            },
            VerificationError::StoreUnavailable => ApiError::internal("Store unavailable"),
        })?;

    let user_store = Arc::clone(&state.user_store);
    let uid = ver.user_id.clone();
    tokio::task::spawn_blocking(move || user_store.mark_email_verified(&uid))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?
        .map_err(|_| ApiError::internal("Failed to mark email verified"))?;

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "Email verified successfully" }),
    ))
}

#[derive(Debug, Deserialize)]
struct ResendVerificationBody {
    email: String,
}

async fn resend_verification_handler(
    State(state): State<AppState>,
    Json(body): Json<ResendVerificationBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let email = body.email.trim().to_lowercase();
    let user_store = Arc::clone(&state.user_store);
    let em = email.clone();
    let user = tokio::task::spawn_blocking(move || user_store.find_by_email(&em))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?;

    // Always 200 to avoid email enumeration
    if let Some(u) = user {
        if !u.email_verified {
            let ver_store = Arc::clone(&state.verification_store);
            let email_client = Arc::clone(&state.email_client);
            let uid = u.id.clone();
            let em2 = email.clone();
            tokio::spawn(async move {
                let ver = tokio::task::spawn_blocking(move || ver_store.create(&uid, &em2, 24))
                    .await
                    .ok();
                if let Some(ver) = ver {
                    email_client
                        .send_email_verification(&ver.email, &ver.token, ver.expires_at)
                        .await;
                }
            });
        }
    }

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "If an unverified account exists with that email, a verification link has been sent." }),
    ))
}

// ── Organization management ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CreateOrgBody {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AddMemberBody {
    user_id: String,
    #[serde(default = "default_editor_role")]
    role: String,
}

fn default_editor_role() -> String {
    "editor".to_string()
}

fn require_user_id(claims: &Option<Claims>) -> Result<String, ApiError> {
    claims
        .as_ref()
        .and_then(|c| c.user_id.clone())
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "Must be authenticated as a user".to_string(),
        })
}

async fn list_orgs_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<crate::orgs::OrgRecord>>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let orgs = tokio::task::spawn_blocking(move || store.list_for_user(&user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(Json(orgs))
}

async fn create_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CreateOrgBody>,
) -> Result<(StatusCode, Json<crate::orgs::OrgRecord>), ApiError> {
    let user_id = require_user_id(&claims)?;
    let org_id = uuid::Uuid::new_v4().to_string();
    let store = Arc::clone(&state.org_store);
    let org = tokio::task::spawn_blocking(move || store.create(&org_id, &body.name, &user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?;
    Ok((StatusCode::CREATED, Json(org)))
}

async fn get_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
) -> Result<Json<crate::orgs::OrgRecord>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();
    let member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        let user_id = user_id.clone();
        move || store.get_member(&org_id2, &user_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?;

    if member.is_none() {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Not a member of this organization".to_string(),
        });
    }

    let org = tokio::task::spawn_blocking(move || store.find_by_id(&org_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Organization not found".to_string(),
        })?;
    Ok(Json(org))
}

async fn delete_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let user_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();

    // Only the org owner or an admin member can delete
    let org = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.find_by_id(&org_id2)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "Organization not found".to_string(),
    })?;

    if org.owner_id != user_id {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Only the owner can delete an organization".to_string(),
        });
    }

    tokio::task::spawn_blocking(move || store.delete(&org_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_org_members_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
) -> Result<Json<Vec<crate::orgs::OrgMember>>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();

    // Must be a member to list members
    let member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.get_member(&org_id2, &user_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?;

    if member.is_none() {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Not a member of this organization".to_string(),
        });
    }

    let members = tokio::task::spawn_blocking(move || store.list_members(&org_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(Json(members))
}

async fn add_org_member_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
    Json(body): Json<AddMemberBody>,
) -> Result<(StatusCode, Json<crate::orgs::OrgMember>), ApiError> {
    let caller_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();

    // Must be an admin member to add members
    let caller_member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.get_member(&org_id2, &caller_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?;

    match caller_member {
        Some(m) if m.role == "admin" => {}
        _ => {
            return Err(ApiError {
                status: StatusCode::FORBIDDEN,
                message: "Only admin members can add members".to_string(),
            })
        }
    }

    let member =
        tokio::task::spawn_blocking(move || store.add_member(&org_id, &body.user_id, &body.role))
            .await
            .map_err(|_| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "Task join error".to_string(),
            })?
            .map_err(|e| match e {
                crate::orgs::OrgError::AlreadyMember => ApiError {
                    status: StatusCode::CONFLICT,
                    message: "User is already a member".to_string(),
                },
                other => ApiError {
                    status: StatusCode::BAD_REQUEST,
                    message: other.to_string(),
                },
            })?;
    Ok((StatusCode::CREATED, Json(member)))
}

async fn remove_org_member_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((org_id, target_user_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let caller_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();
    let caller_id2 = caller_id.clone();

    let caller_member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.get_member(&org_id2, &caller_id2)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?;

    // Admins can remove anyone; members can only remove themselves
    let is_admin = caller_member.map(|m| m.role == "admin").unwrap_or(false);
    let is_self = caller_id == target_user_id;
    if !is_admin && !is_self {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Not authorized to remove this member".to_string(),
        });
    }

    tokio::task::spawn_blocking(move || store.remove_member(&org_id, &target_user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Member not found".to_string(),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

/// Issues a new JWT scoped to the given org's tenant_id with the caller's membership role.
async fn switch_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let user_email = claims.as_ref().and_then(|c| c.email.clone());
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();
    let user_id2 = user_id.clone();

    let member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.get_member(&org_id2, &user_id2)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::FORBIDDEN,
        message: "Not a member of this organization".to_string(),
    })?;

    let role: crate::auth::Role = member.role.parse().unwrap_or_default();
    let exp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 7 * 24 * 3600;
    let new_claims = Claims {
        sub: user_id.clone(),
        tenant_id: org_id.clone(),
        workspace_id: "workspace-1".to_string(),
        project_id: "project-1".to_string(),
        exp,
        role: role.clone(),
        user_id: Some(user_id),
        email: user_email,
    };
    let token = sign_token(&new_claims).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Failed to sign token".to_string(),
    })?;
    Ok(Json(serde_json::json!({
        "token": token,
        "org_id": org_id,
        "tenant_id": org_id,
        "role": role.as_str(),
    })))
}

// ── API Key management ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ApiKeyQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct CreateApiKeyBody {
    tenant_id: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct CreateApiKeyResponse {
    #[serde(flatten)]
    record: crate::api_keys::ApiKeyRecord,
    /// Plaintext key — returned only at creation time.
    key: String,
}

async fn list_api_keys_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<ApiKeyQuery>,
) -> Json<Vec<crate::api_keys::ApiKeyRecord>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(state.api_key_store.list(&tenant_id).await)
}

async fn create_api_key_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CreateApiKeyBody>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), ApiError> {
    require_admin(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let raw_key = crate::api_keys::generate_api_key();
    let record = state
        .api_key_store
        .create(&body.tenant_id, &body.name, &raw_key)
        .await;
    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            record,
            key: raw_key,
        }),
    ))
}

async fn delete_api_key_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(key_id): Path<String>,
    Query(query): Query<ApiKeyQuery>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    if state.api_key_store.delete(&tenant_id, &key_id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}

pub fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ").map(|s| s.to_string())
}

/// Decode the Authorization header and return claims, or None if no/invalid token.
/// When AUTH_REQUIRED=true in the environment, callers should reject requests without claims.
pub fn extract_claims(headers: &axum::http::HeaderMap) -> Option<Claims> {
    let token = extract_bearer(headers)?;
    verify_token(&token)
}

/// Return the effective tenant_id for a request.
/// When AUTH_REQUIRED=true and valid JWT claims are present, the JWT's tenant_id takes
/// precedence over any client-supplied value (prevents IDOR / tenant spoofing).
fn effective_tenant_id(claims: &Option<Claims>, supplied: &str) -> String {
    let auth_required = std::env::var("AUTH_REQUIRED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    effective_tenant_id_with_flag(auth_required, claims, supplied)
}

fn effective_tenant_id_with_flag(
    auth_required: bool,
    claims: &Option<Claims>,
    supplied: &str,
) -> String {
    // JWT always wins when present — prevents tenant_id spoofing regardless of AUTH_REQUIRED.
    // Falls back to supplied only in dev mode (no JWT issued yet).
    match claims {
        Some(c) => c.tenant_id.clone(),
        None if auth_required => supplied.to_string(), // middleware already enforced 401
        None => supplied.to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct GetExecutionQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct GetWorkflowVersionQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct ListWorkflowsQuery {
    tenant_id: String,
    project_id: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
    tag: Option<String>,
    folder: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetWorkflowQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct ListWorkflowVersionsQuery {
    tenant_id: String,
    status: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ListExecutionsQuery {
    tenant_id: String,
    workflow_id: Option<String>,
    label: Option<String>,
    status: Option<String>,
    search: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct StartWorkflowVersionExecutionRequest {
    tenant_id: String,
    input_json: String,
    #[serde(default)]
    env_set: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    callback_url: Option<String>,
    #[serde(default)]
    dry_run: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct StartWorkflowExecutionRequest {
    tenant_id: String,
    input_json: String,
    #[serde(default)]
    env_set: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    callback_url: Option<String>,
    #[serde(default)]
    dry_run: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CredentialQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct AuditLogQuery {
    tenant_id: String,
    limit: Option<usize>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    resource_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExportWorkflowQuery {
    tenant_id: String,
}

#[derive(Debug, Serialize)]
struct WorkflowExport {
    name: String,
    graph: workflow_core::WorkflowGraph,
    exported_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    readme: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DuplicateWorkflowBody {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct ImportWorkflowBody {
    tenant_id: String,
    workspace_id: String,
    project_id: String,
    name: Option<String>,
    graph: workflow_core::WorkflowGraph,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    readme: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CreateCredentialBody {
    tenant_id: String,
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct ApprovalBody {
    #[allow(dead_code)]
    tenant_id: Option<String>,
    #[serde(default)]
    comment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CancelExecutionBody {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct RetryExecutionBody {
    tenant_id: String,
    #[serde(default)]
    input_json: Option<String>,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateWebhookBody {
    tenant_id: String,
}

#[derive(Debug, Serialize)]
struct WebhookResponse {
    token: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret: Option<String>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(msg: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.to_string(),
        }
    }
    fn forbidden(msg: &str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: msg.to_string(),
        }
    }
    fn not_found(msg: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.to_string(),
        }
    }
    fn internal(msg: &str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.to_string(),
        }
    }
}

fn auth_required() -> bool {
    std::env::var("AUTH_REQUIRED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

fn require_write_inner(
    claims: &Option<crate::auth::Claims>,
    enforced: bool,
) -> Result<(), ApiError> {
    if !enforced {
        return Ok(());
    }
    match claims {
        Some(c) if c.can_write() => Ok(()),
        Some(_) => Err(ApiError::forbidden(
            "Viewer role cannot perform write operations",
        )),
        None => Err(ApiError::forbidden("Authentication required")),
    }
}

fn require_admin_inner(
    claims: &Option<crate::auth::Claims>,
    enforced: bool,
) -> Result<(), ApiError> {
    if !enforced {
        return Ok(());
    }
    match claims {
        Some(c) if c.is_admin() => Ok(()),
        Some(_) => Err(ApiError::forbidden("Admin role required")),
        None => Err(ApiError::forbidden("Authentication required")),
    }
}

/// Return Err(Forbidden) if AUTH_REQUIRED is set and the caller's role is below Editor.
fn require_write(claims: &Option<crate::auth::Claims>) -> Result<(), ApiError> {
    require_write_inner(claims, auth_required())
}

/// Return Err(Forbidden) if AUTH_REQUIRED is set and the caller's role is below Admin.
fn require_admin(claims: &Option<crate::auth::Claims>) -> Result<(), ApiError> {
    require_admin_inner(claims, auth_required())
}

impl From<ExecutionError> for ApiError {
    fn from(error: ExecutionError) -> Self {
        let status = match error {
            ExecutionError::NotFound => StatusCode::NOT_FOUND,
            ExecutionError::StoreUnavailable | ExecutionError::ExecutorUnavailable => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            ExecutionError::MissingTenant
            | ExecutionError::MissingWorkflow
            | ExecutionError::MissingWorkflowVersion
            | ExecutionError::WorkflowVersionMismatch
            | ExecutionError::InvalidGraph
            | ExecutionError::InvalidInput => StatusCode::BAD_REQUEST,
            ExecutionError::InputSchemaViolation(_) => StatusCode::UNPROCESSABLE_ENTITY,
        };
        let message = match &error {
            ExecutionError::InputSchemaViolation(msg) => msg.clone(),
            _ => format!("{error:?}"),
        };

        Self { status, message }
    }
}

impl From<CredentialError> for ApiError {
    fn from(e: CredentialError) -> Self {
        let (status, message) = match e {
            CredentialError::NotFound => (StatusCode::NOT_FOUND, "CredentialNotFound"),
            CredentialError::NameTaken => (StatusCode::CONFLICT, "CredentialNameTaken"),
            CredentialError::StoreUnavailable => {
                (StatusCode::INTERNAL_SERVER_ERROR, "StoreUnavailable")
            }
        };
        Self {
            status,
            message: message.to_string(),
        }
    }
}

impl From<EnvVarError> for ApiError {
    fn from(e: EnvVarError) -> Self {
        let (status, message) = match e {
            EnvVarError::NotFound => (StatusCode::NOT_FOUND, "EnvVarNotFound"),
            EnvVarError::StoreUnavailable => {
                (StatusCode::INTERNAL_SERVER_ERROR, "StoreUnavailable")
            }
        };
        Self {
            status,
            message: message.to_string(),
        }
    }
}

impl From<WebhookError> for ApiError {
    fn from(e: WebhookError) -> Self {
        let (status, message) = match e {
            WebhookError::NotFound => (StatusCode::NOT_FOUND, "WebhookNotFound"),
            WebhookError::StoreUnavailable => {
                (StatusCode::INTERNAL_SERVER_ERROR, "StoreUnavailable")
            }
        };
        Self {
            status,
            message: message.to_string(),
        }
    }
}

impl From<WorkflowError> for ApiError {
    fn from(error: WorkflowError) -> Self {
        let status = match error {
            WorkflowError::NotFound => StatusCode::NOT_FOUND,
            WorkflowError::StoreUnavailable => StatusCode::INTERNAL_SERVER_ERROR,
            WorkflowError::MissingTenant
            | WorkflowError::MissingWorkspace
            | WorkflowError::MissingProject
            | WorkflowError::MissingWorkflow
            | WorkflowError::MissingName
            | WorkflowError::MissingWorkflowVersion
            | WorkflowError::InvalidGraph
            | WorkflowError::InvalidStatus
            | WorkflowError::InvalidLimit
            | WorkflowError::ArchivedWorkflow
            | WorkflowError::DraftVersion
            | WorkflowError::NoPublishedVersion
            | WorkflowError::LockedWorkflow => StatusCode::BAD_REQUEST,
        };

        Self {
            status,
            message: format!("{error:?}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

// ── Cron preview ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CronPreviewRequest {
    expression: String,
    #[serde(default = "default_cron_count")]
    count: usize,
}
fn default_cron_count() -> usize {
    5
}

#[derive(Serialize)]
struct CronPreviewResponse {
    next_times: Vec<String>,
    error: Option<String>,
}

async fn cron_preview_handler(Json(req): Json<CronPreviewRequest>) -> impl IntoResponse {
    use chrono::Utc;
    use cron::Schedule;
    use std::str::FromStr;

    let count = req.count.min(10);
    match Schedule::from_str(&req.expression) {
        Err(e) => Json(CronPreviewResponse {
            next_times: vec![],
            error: Some(format!("Invalid expression: {e}")),
        }),
        Ok(schedule) => {
            let times: Vec<String> = schedule
                .upcoming(Utc)
                .take(count)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .collect();
            Json(CronPreviewResponse {
                next_times: times,
                error: None,
            })
        }
    }
}

// ── Test Cases ──────────────────────────────────────────────────────────────

async fn list_test_cases_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Json<Vec<serde_json::Value>> {
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    let cases = state.test_case_store.list(&tenant_id, &workflow_id).await;
    Json(
        cases
            .into_iter()
            .map(|tc| serde_json::to_value(&tc).unwrap_or_default())
            .collect(),
    )
}

async fn create_test_case_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<CreateTestCaseRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let tenant_id = request.tenant_id.clone();
    let tc = state
        .test_case_store
        .create(&tenant_id, &workflow_id, request)
        .await
        .map_err(|_| ApiError::internal("test_case_create_failed"))?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(&tc).unwrap_or_default()),
    ))
}

async fn get_test_case_handler(
    State(state): State<AppState>,
    Path(test_case_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tc = state
        .test_case_store
        .get(&test_case_id)
        .await
        .map_err(|e| match e {
            TestCaseError::NotFound => ApiError::not_found("test_case"),
            _ => ApiError::internal("test_case_store"),
        })?;
    Ok(Json(serde_json::to_value(&tc).unwrap_or_default()))
}

async fn update_test_case_handler(
    State(state): State<AppState>,
    Path(test_case_id): Path<String>,
    Json(request): Json<UpdateTestCaseRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tc = state
        .test_case_store
        .update(&test_case_id, request)
        .await
        .map_err(|e| match e {
            TestCaseError::NotFound => ApiError::not_found("test_case"),
            _ => ApiError::internal("test_case_store"),
        })?;
    Ok(Json(serde_json::to_value(&tc).unwrap_or_default()))
}

async fn delete_test_case_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(test_case_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    state
        .test_case_store
        .delete(&test_case_id)
        .await
        .map_err(|e| match e {
            TestCaseError::NotFound => ApiError::not_found("test_case"),
            _ => ApiError::internal("test_case_store"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
struct TestCaseRunResult {
    test_case_id: String,
    execution_id: String,
    status: String,
    passed: bool,
    output_json: Option<String>,
    expected_output: Option<String>,
}

async fn run_test_case_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(test_case_id): Path<String>,
) -> Result<Json<TestCaseRunResult>, ApiError> {
    let tc = state
        .test_case_store
        .get(&test_case_id)
        .await
        .map_err(|e| match e {
            TestCaseError::NotFound => ApiError::not_found("test_case"),
            _ => ApiError::internal("test_case_store"),
        })?;
    // Verify caller owns this test case's tenant (returns 404 to avoid leaking existence).
    let caller_tenant = effective_tenant_id(&claims, &tc.tenant_id);
    if caller_tenant != tc.tenant_id {
        return Err(ApiError::not_found("test_case"));
    }
    let workflow = state
        .workflow_service
        .get_workflow(&tc.tenant_id, &tc.workflow_id)
        .await?;
    let version_id = workflow
        .latest_version_id
        .ok_or(WorkflowError::NoPublishedVersion)?;
    let version = state
        .workflow_service
        .get_version(&tc.tenant_id, &version_id)
        .await?;
    let graph = resolve_graph_credentials(
        version.graph,
        &state.credential_store,
        &state.env_store,
        &tc.tenant_id,
        DEFAULT_SET,
    )
    .await;
    let graph = inject_sub_workflow_graphs(
        graph,
        &state.workflow_service,
        &state.credential_store,
        &tc.tenant_id,
    )
    .await;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: tc.tenant_id.clone(),
            workflow_id: tc.workflow_id.clone(),
            workflow_version_id: version_id,
            graph,
            input_json: tc.input_json.clone(),
            label: Some(format!("test:{}", &tc.name)),
            callback_url: None,
            trigger_type: Some("test".to_string()),
            dry_run: false,
            retried_from: None,
        })
        .await?;
    let passed = if let (Some(expected), Some(actual)) = (&tc.expected_output, &record.output_json)
    {
        let ev: serde_json::Value = serde_json::from_str(expected).unwrap_or_default();
        let av: serde_json::Value = serde_json::from_str(actual).unwrap_or_default();
        ev == av
    } else {
        tc.expected_output.is_none()
    };
    Ok(Json(TestCaseRunResult {
        test_case_id: tc.id,
        execution_id: record.id,
        status: format!("{:?}", record.status).to_lowercase(),
        passed,
        output_json: record.output_json,
        expected_output: tc.expected_output,
    }))
}

// ── Event Subscriptions ────────────────────────────────────────────────────

async fn list_event_subscriptions_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    let subs = state
        .subscription_store
        .list(&tenant_id)
        .await
        .map_err(|_| ApiError::internal("subscription_store"))?;
    Ok(Json(
        subs.into_iter()
            .map(|s| serde_json::to_value(&s).unwrap_or_default())
            .collect(),
    ))
}

async fn create_event_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut req): Json<CreateSubscriptionRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    req.tenant_id = effective_tenant_id(&claims, &req.tenant_id);
    let sub = state
        .subscription_store
        .create(req)
        .await
        .map_err(|e| match e {
            SubscriptionError::InvalidUrl => {
                ApiError::bad_request("url must start with http or https")
            }
            _ => ApiError::internal("subscription_store"),
        })?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(&sub).unwrap_or_default()),
    ))
}

async fn delete_event_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(sub_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    state
        .subscription_store
        .delete(&tenant_id, &sub_id)
        .await
        .map_err(|e| match e {
            SubscriptionError::NotFound => ApiError::not_found("event_subscription"),
            _ => ApiError::internal("subscription_store"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Workflow Comments ──────────────────────────────────────────────────────

async fn list_workflow_comments_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    let comments = state
        .comment_store
        .list(&tenant_id, &workflow_id)
        .await
        .map_err(|_| ApiError::internal("comment_store"))?;
    Ok(Json(
        comments
            .into_iter()
            .map(|c| serde_json::to_value(&c).unwrap_or_default())
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
struct CreateCommentBody {
    tenant_id: String,
    author: String,
    body: String,
}

async fn create_workflow_comment_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(req): Json<CreateCommentBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let tenant_id = effective_tenant_id(&claims, &req.tenant_id);
    use crate::comments::CommentError;
    let comment = state
        .comment_store
        .create(CreateCommentRequest {
            tenant_id,
            workflow_id,
            author: req.author,
            body: req.body,
        })
        .await
        .map_err(|e| match e {
            CommentError::EmptyBody => ApiError::bad_request("comment body must not be empty"),
            _ => ApiError::internal("comment_store"),
        })?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(&comment).unwrap_or_default()),
    ))
}

#[derive(Debug, Deserialize)]
struct EditCommentBody {
    tenant_id: String,
    body: String,
}

async fn edit_workflow_comment_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(comment_id): Path<String>,
    Json(req): Json<EditCommentBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &req.tenant_id);
    use crate::comments::CommentError;
    let comment = state
        .comment_store
        .edit(
            &tenant_id,
            &comment_id,
            EditCommentRequest {
                tenant_id: tenant_id.clone(),
                body: req.body,
            },
        )
        .await
        .map_err(|e| match e {
            CommentError::NotFound => ApiError::not_found("comment"),
            CommentError::EmptyBody => ApiError::bad_request("comment body must not be empty"),
            _ => ApiError::internal("comment_store"),
        })?;
    Ok(Json(serde_json::to_value(&comment).unwrap_or_default()))
}

async fn delete_workflow_comment_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(comment_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    use crate::comments::CommentError;
    state
        .comment_store
        .delete(&tenant_id, &comment_id)
        .await
        .map_err(|e| match e {
            CommentError::NotFound => ApiError::not_found("comment"),
            _ => ApiError::internal("comment_store"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Form Publisher ─────────────────────────────────────────────────────────

async fn publish_form_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<PublishFormRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let workflow = state
        .workflow_service
        .get_workflow(&request.tenant_id, &workflow_id)
        .await?;
    let input_schema = if let Some(version_id) = &workflow.latest_version_id {
        let version = state
            .workflow_service
            .get_version(&request.tenant_id, version_id)
            .await
            .ok();
        version
            .and_then(|v| {
                v.graph
                    .nodes
                    .iter()
                    .find(|n| n.node_type == workflow_core::NodeType::Trigger)
                    .and_then(|n| n.config.clone())
                    .and_then(|c| c.get("input_schema").cloned())
            })
            .unwrap_or(serde_json::json!([]))
    } else {
        serde_json::json!([])
    };
    let tenant_id = request.tenant_id.clone();
    let record = state
        .form_store
        .publish_form(&tenant_id, &workflow_id, request, input_schema)
        .await
        .map_err(|_| ApiError::internal("form_publish_failed"))?;
    Ok(Json(serde_json::json!({
        "token": record.token,
        "title": record.title,
        "workflow_id": record.workflow_id,
    })))
}

async fn list_forms_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Json<Vec<serde_json::Value>> {
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    let forms = state
        .form_store
        .list_by_workflow(&tenant_id, &workflow_id)
        .await;
    Json(
        forms
            .into_iter()
            .map(|f| {
                serde_json::json!({
                    "token": f.token,
                    "title": f.title,
                    "description": f.description,
                    "created_at": f.created_at,
                })
            })
            .collect(),
    )
}

async fn get_form_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let form = state.form_store.get(&token).await.map_err(|e| match e {
        FormError::NotFound => ApiError::not_found("form_token"),
        _ => ApiError::internal("form_store"),
    })?;
    Ok(Json(serde_json::json!({
        "token": form.token,
        "title": form.title,
        "description": form.description,
        "workflow_id": form.workflow_id,
        "input_schema": form.input_schema,
        "created_at": form.created_at,
    })))
}

async fn delete_form_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    state.form_store.delete(&token).await.map_err(|e| match e {
        FormError::NotFound => ApiError::not_found("form_token"),
        _ => ApiError::internal("form_store"),
    })?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct FormSubmitBody {
    #[serde(default = "empty_json")]
    input_json: String,
}
fn empty_json() -> String {
    "{}".to_string()
}

async fn submit_form_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(body): Json<FormSubmitBody>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    let form = state.form_store.get(&token).await.map_err(|e| match e {
        FormError::NotFound => ApiError::not_found("form_token"),
        _ => ApiError::internal("form_store"),
    })?;
    let workflow = state
        .workflow_service
        .get_workflow(&form.tenant_id, &form.workflow_id)
        .await?;
    let version_id = workflow
        .latest_version_id
        .ok_or(WorkflowError::NoPublishedVersion)?;
    let version = state
        .workflow_service
        .get_version(&form.tenant_id, &version_id)
        .await?;
    let graph = resolve_graph_credentials(
        version.graph,
        &state.credential_store,
        &state.env_store,
        &form.tenant_id,
        DEFAULT_SET,
    )
    .await;
    let graph = inject_sub_workflow_graphs(
        graph,
        &state.workflow_service,
        &state.credential_store,
        &form.tenant_id,
    )
    .await;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: form.tenant_id,
            workflow_id: form.workflow_id,
            workflow_version_id: version_id,
            graph,
            input_json: body.input_json,
            label: Some(format!("form:{}", &token[..token.len().min(12)])),
            callback_url: None,
            trigger_type: Some("form".to_string()),
            dry_run: false,
            retried_from: None,
        })
        .await?;
    Ok((StatusCode::ACCEPTED, Json(record)))
}

// ── In-App Notification handlers ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct NotifQuery {
    tenant_id: Option<String>,
    #[serde(default = "default_notif_limit")]
    limit: usize,
}
fn default_notif_limit() -> usize {
    50
}

async fn list_notifications_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<NotifQuery>,
) -> Json<serde_json::Value> {
    let tenant_id = effective_tenant_id(&claims, query.tenant_id.as_deref().unwrap_or(""));
    let user_id = claims.as_ref().and_then(|c| c.user_id.as_deref());
    let items = state
        .notification_store
        .list(&tenant_id, user_id, query.limit);
    let unread = state.notification_store.unread_count(&tenant_id, user_id);
    Json(serde_json::json!({ "notifications": items, "unread_count": unread }))
}

async fn mark_notification_read_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(notif_id): Path<String>,
    Query(q): Query<TenantQuery>,
) -> impl IntoResponse {
    let tenant_id = effective_tenant_id(&claims, &q.tenant_id);
    let found = state.notification_store.mark_read(&notif_id, &tenant_id);
    if found {
        (StatusCode::OK, Json(serde_json::json!({"ok": true})))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
    }
}

async fn mark_all_notifications_read_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(q): Query<TenantQuery>,
) -> Json<serde_json::Value> {
    let tenant_id = effective_tenant_id(&claims, &q.tenant_id);
    let user_id = claims.as_ref().and_then(|c| c.user_id.as_deref());
    state.notification_store.mark_all_read(&tenant_id, user_id);
    Json(serde_json::json!({"ok": true}))
}

async fn delete_notification_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(notif_id): Path<String>,
    Query(q): Query<TenantQuery>,
) -> impl IntoResponse {
    let tenant_id = effective_tenant_id(&claims, &q.tenant_id);
    let found = state.notification_store.delete(&notif_id, &tenant_id);
    if found {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response()
    }
}

// Helper to fire a notification (non-blocking)
pub(crate) fn push_notification(
    state: &AppState,
    tenant_id: &str,
    user_id: Option<&str>,
    title: &str,
    body: &str,
    level: &str,
) {
    state
        .notification_store
        .create(tenant_id, user_id, title, body, level);
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
