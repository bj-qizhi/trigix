// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

mod admin;
mod analytics;
mod auth;
mod billing;
mod credentials;
mod custom_nodes;
mod executions;
mod forms;
mod orgs;
mod rag;
mod sso;
mod system;
mod webhooks;
mod workflows;
mod workspaces;

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
    custom_node_store: Arc<crate::custom_nodes::PlatformCustomNodeStore>,
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
        custom_node_store: Arc::new(crate::custom_nodes::PlatformCustomNodeStore::default()),
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
        .merge(admin::routes())
        .merge(analytics::routes())
        .merge(auth::routes())
        .merge(billing::routes())
        .merge(credentials::routes())
        .merge(custom_nodes::routes())
        .merge(executions::routes())
        .merge(forms::routes())
        .merge(orgs::routes())
        .merge(rag::routes())
        .merge(sso::routes())
        .merge(system::routes())
        .merge(webhooks::routes())
        .merge(workflows::routes())
        .merge(workspaces::routes())
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
        custom_node_store: Arc::new(crate::custom_nodes::PlatformCustomNodeStore::default()),
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
    custom_node_store: crate::custom_nodes::PlatformCustomNodeStore,
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
        custom_node_store: Arc::new(custom_node_store),
    };
    spawn_schedule_runner(state.clone());
    spawn_execution_timeout_guard(state.clone());
    if state.cache.is_available() {
        spawn_queue_worker(state.clone());
    }
    build_router(state)
}

#[derive(serde::Serialize)]
struct HealthDetail {
    status: &'static str,
    version: &'static str,
    database: bool,
    cache: bool,
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

#[derive(serde::Serialize)]
struct QueueDepthResponse {
    queue_depth: Option<u64>,
    stream: &'static str,
    dead_letter_depth: Option<u64>,
    dead_letter_stream: &'static str,
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

#[derive(serde::Serialize)]
struct DlqRequeueResponse {
    requeued: usize,
}

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

#[derive(Deserialize)]
struct HistoryQuery {
    #[serde(default = "default_history_months")]
    months: usize,
}
fn default_history_months() -> usize {
    6
}

#[derive(Deserialize)]
struct CheckoutBody {
    tier: String,
}

#[derive(Debug, Deserialize)]
struct SetQuotaBody {
    tier: String,
}

#[derive(Debug, Deserialize)]
struct McpToolRequest {
    tool: String,
    input: Option<serde_json::Value>,
    tenant_id: Option<String>,
}

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

#[derive(Debug, Deserialize)]
struct BatchStartBody {
    requests: Vec<StartExecutionRequest>,
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

#[derive(Debug, Deserialize)]
struct PatchExecutionBody {
    #[serde(default)]
    tenant_id: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SetExecutionNoteBody {
    #[serde(default)]
    tenant_id: String,
    note: Option<String>,
}

#[derive(Deserialize)]
struct SetVisibilityBody {
    #[serde(default)]
    tenant_id: String,
    visibility: String,
}

#[derive(Deserialize)]
struct MoveWorkflowBody {
    tenant_id: String,
    #[serde(default)]
    folder: Option<String>,
}

#[derive(serde::Serialize)]
struct WorkflowStats {
    total: usize,
    succeeded: usize,
    failed: usize,
    running: usize,
    avg_duration_secs: Option<f64>,
}

#[derive(serde::Serialize)]
struct WorkflowEstimate {
    sample_count: usize,
    p50_secs: Option<f64>,
    p95_secs: Option<f64>,
    min_secs: Option<f64>,
    max_secs: Option<f64>,
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

#[derive(Debug, Deserialize)]
struct TokenUsageQuery {
    tenant_id: String,
    #[serde(default)]
    days: Option<u64>,
}

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

#[derive(Deserialize)]
struct DeliveryQuery {
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct SetWebhookConditionBody {
    condition_expr: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SetWebhookRateLimitBody {
    max_calls_per_minute: Option<u32>,
}

#[derive(Deserialize)]
struct SetPayloadTransformBody {
    script: Option<String>,
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

#[derive(Deserialize)]
struct UpdateSsoBody {
    enabled: bool,
}

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

#[derive(Deserialize)]
struct RagDocsQuery {
    kb: String,
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

#[derive(Deserialize)]
struct CustomNodeBody {
    slug: String,
    label: String,
    #[serde(default)]
    description: String,
    endpoint: String,
    #[serde(default)]
    config_schema: serde_json::Value,
}

#[derive(Deserialize)]
struct ImportManifestBody {
    base_url: String,
}

#[derive(Deserialize)]
struct ManifestNode {
    slug: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    description: Option<String>,
    endpoint: String,
    #[serde(default)]
    config_schema: serde_json::Value,
}

#[derive(Deserialize)]
struct ManifestResponse {
    nodes: Vec<ManifestNode>,
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

#[derive(Deserialize)]
struct SsoCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateMeBody {
    name: Option<String>,
    current_password: Option<String>,
    new_password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateNotificationsBody {
    #[serde(default)]
    email_on_failure: bool,
    #[serde(default)]
    email_on_success: bool,
}

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

#[derive(Debug, Deserialize)]
struct AcceptInviteBody {
    token: String,
    password: String,
    name: Option<String>,
}

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

#[derive(Debug, Deserialize)]
struct ResetPasswordBody {
    token: String,
    new_password: String,
}

#[derive(Debug, Deserialize)]
struct VerifyEmailBody {
    token: String,
}

#[derive(Debug, Deserialize)]
struct ResendVerificationBody {
    email: String,
}

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

#[derive(Debug, Serialize)]
struct TestCaseRunResult {
    test_case_id: String,
    execution_id: String,
    status: String,
    passed: bool,
    output_json: Option<String>,
    expected_output: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateCommentBody {
    tenant_id: String,
    author: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct EditCommentBody {
    tenant_id: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct FormSubmitBody {
    #[serde(default = "empty_json")]
    input_json: String,
}
fn empty_json() -> String {
    "{}".to_string()
}

#[derive(Debug, Deserialize)]
struct NotifQuery {
    tenant_id: Option<String>,
    #[serde(default = "default_notif_limit")]
    limit: usize,
}
fn default_notif_limit() -> usize {
    50
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
