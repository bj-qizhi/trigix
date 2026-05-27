#[tokio::main]
async fn main() {
    let addr =
        std::env::var("PLATFORM_HTTP_ADDR").unwrap_or_else(|_| "127.0.0.1:38080".to_string());
    let (store, workflow_store) = match std::env::var("DATABASE_URL") {
        Ok(database_url) => {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(&database_url)
                .await
                .expect("connect platform database");
            sqlx::migrate!("../../infra/postgres/migrations")
                .run(&pool)
                .await
                .expect("run database migrations");
            (
                agentflow_platform::execution::PlatformExecutionStore::postgres(
                    agentflow_platform::execution::PostgresExecutionStore::new(pool.clone()),
                ),
                agentflow_platform::workflow::PlatformWorkflowVersionStore::postgres(
                    agentflow_platform::workflow::PostgresWorkflowVersionStore::new(pool),
                ),
            )
        }
        Err(_) => (
            agentflow_platform::execution::PlatformExecutionStore::memory(),
            agentflow_platform::workflow::PlatformWorkflowVersionStore::memory_with_dev_seed(),
        ),
    };
    let executor = match std::env::var("EXECUTOR_BASE_URL") {
        Ok(base_url) => {
            agentflow_platform::execution::PlatformExecutorClient::http(base_url, store.clone())
        }
        Err(_) => agentflow_platform::execution::PlatformExecutorClient::inline(store.clone()),
    };
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind platform HTTP listener");

    println!("agentflow platform listening on {addr}");
    axum::serve(
        listener,
        agentflow_platform::http::router_with_store_and_executor(store, workflow_store, executor),
    )
    .await
    .expect("serve platform HTTP API");
}
