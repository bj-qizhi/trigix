// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

#[tokio::main]
async fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info".into());
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "text".to_string());
    if log_format == "json" {
        tracing_subscriber::fmt().json().with_env_filter(filter).init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let addr =
        std::env::var("PLATFORM_HTTP_ADDR").unwrap_or_else(|_| "127.0.0.1:38080".to_string());

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind platform HTTP listener");

    let db_max_conn: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(5);

    tracing::info!(
        addr = %addr,
        log_format = %log_format,
        db_max_connections = db_max_conn,
        version = env!("CARGO_PKG_VERSION"),
        "trigix platform starting"
    );

    let router = match std::env::var("DATABASE_URL") {
        Ok(database_url) => {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(db_max_conn)
                .connect(&database_url)
                .await
                .expect("connect platform database");
            sqlx::migrate!("../../infra/postgres/migrations")
                .run(&pool)
                .await
                .expect("run database migrations");

            let store = trigix_platform::execution::PlatformExecutionStore::postgres(
                trigix_platform::execution::PostgresExecutionStore::new(pool.clone()),
            );
            let gate = std::sync::Arc::new(trigix_executor::approval::ApprovalGate::default());
            let token_usage_store = std::sync::Arc::new(
                trigix_platform::token_usage::PlatformTokenUsageStore::postgres(
                    trigix_platform::token_usage::PostgresTokenUsageStore::new(pool.clone()),
                ),
            );
            let cache = trigix_platform::cache::CacheClient::from_env().await;
            let executor = if std::env::var("USE_QUEUE").as_deref() == Ok("true") {
                trigix_platform::execution::PlatformExecutorClient::queue(cache.clone())
            } else {
                match std::env::var("EXECUTOR_BASE_URL") {
                    Ok(base_url) => trigix_platform::execution::PlatformExecutorClient::http(
                        base_url,
                        store.clone(),
                    ),
                    Err(_) => trigix_platform::execution::PlatformExecutorClient::inline_with_gate_and_usage(
                        store.clone(),
                        std::sync::Arc::clone(&gate),
                        std::sync::Arc::clone(&token_usage_store),
                    ),
                }
            };
            let execution_service = trigix_platform::execution::ExecutionService::new(
                store,
                executor,
            );
            let workflow_service = trigix_platform::workflow::WorkflowService::new(
                trigix_platform::workflow::PlatformWorkflowVersionStore::postgres(
                    trigix_platform::workflow::PostgresWorkflowVersionStore::new(pool.clone()),
                ),
            );
            let credential_store =
                trigix_platform::credentials::PlatformCredentialStore::postgres(
                    trigix_platform::credentials::PostgresCredentialStore::new(pool.clone()),
                );
            let env_store = trigix_platform::env_vars::PlatformEnvVarStore::postgres(
                trigix_platform::env_vars::PostgresEnvVarStore::new(pool.clone()),
            );
            let audit_store = trigix_platform::audit::PlatformAuditStore::postgres(
                trigix_platform::audit::PostgresAuditStore::new(pool.clone()),
            );
            let webhook_store = trigix_platform::webhook::PlatformWebhookStore::postgres(
                trigix_platform::webhook::PostgresWebhookStore::new(pool.clone()),
            );
            let schedule_store = trigix_platform::scheduler::PlatformScheduleStore::postgres(pool.clone());
            schedule_store.bootstrap_from_postgres().await;
            let workspace_store = trigix_platform::workspace::PlatformWorkspaceStore::postgres(
                trigix_platform::workspace::PostgresWorkspaceStore::new(pool.clone()),
            );
            let variable_store = trigix_platform::variables::PlatformVariableStore::postgres(
                trigix_platform::variables::PostgresVariableStore::new(pool.clone()),
            );
            let api_key_store = trigix_platform::api_keys::PlatformApiKeyStore::postgres(
                trigix_platform::api_keys::PostgresApiKeyStore::new(pool.clone()),
            );
            let form_store = trigix_platform::form::PlatformFormStore::postgres(pool.clone());
            let test_case_store = trigix_platform::test_cases::PlatformTestCaseStore::postgres(pool.clone());
            let comment_store = trigix_platform::comments::PlatformCommentStore::postgres(pool.clone());
            let subscription_store = trigix_platform::event_subscriptions::PlatformSubscriptionStore::postgres(pool.clone());
            let user_store = trigix_platform::users::PlatformUserStore::postgres(pool.clone());
            let org_store = trigix_platform::orgs::PlatformOrgStore::postgres(pool.clone());
            let invite_store = trigix_platform::invitations::PlatformInviteStore::postgres(pool.clone());
            let reset_store = trigix_platform::password_reset::PlatformPasswordResetStore::postgres(pool.clone());
            let verification_store = trigix_platform::email_verification::PlatformEmailVerificationStore::postgres(pool.clone());
            let notification_prefs_store = trigix_platform::notification_prefs::PlatformNotificationPrefsStore::postgres(pool.clone());
            let billing_store = trigix_platform::billing::PlatformBillingStore::postgres(pool);
            let email_client = trigix_platform::email::EmailClient::from_env();
            trigix_platform::http::router_with_all_stores(
                execution_service,
                workflow_service,
                gate,
                credential_store,
                env_store,
                audit_store,
                webhook_store,
                schedule_store,
                workspace_store,
                variable_store,
                api_key_store,
                std::sync::Arc::try_unwrap(token_usage_store)
                    .unwrap_or_else(|arc| (*arc).clone()),
                form_store,
                test_case_store,
                comment_store,
                subscription_store,
                cache,
                user_store,
                org_store,
                invite_store,
                reset_store,
                verification_store,
                notification_prefs_store,
                email_client,
                billing_store,
            )
        }
        Err(_) => {
            let store = trigix_platform::execution::PlatformExecutionStore::memory();
            let executor = match std::env::var("EXECUTOR_BASE_URL") {
                Ok(base_url) => trigix_platform::execution::PlatformExecutorClient::http(
                    base_url,
                    store.clone(),
                ),
                Err(_) => trigix_platform::execution::PlatformExecutorClient::inline(
                    store.clone(),
                ),
            };
            trigix_platform::http::router_with_store_and_executor(
                store,
                trigix_platform::workflow::PlatformWorkflowVersionStore::memory_with_dev_seed(),
                executor,
            )
        }
    };

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve platform HTTP API");
}

async fn shutdown_signal() {
    use std::sync::atomic::Ordering;

    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    trigix_platform::http::DRAINING.store(true, Ordering::SeqCst);
    tracing::info!("Shutdown signal received; draining in-flight executions…");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let running = trigix_platform::http::METRIC_EXEC_RUNNING.load(Ordering::Relaxed);
        if running == 0 {
            break;
        }
        if std::time::Instant::now() >= deadline {
            tracing::warn!(running, "Drain timeout reached; forcing shutdown");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    tracing::info!("Drain complete; server shutting down");
}
