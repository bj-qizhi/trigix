#[tokio::main]
async fn main() {
    let addr =
        std::env::var("EXECUTOR_HTTP_ADDR").unwrap_or_else(|_| "127.0.0.1:38090".to_string());
    let ai_runtime_base_url = std::env::var("AI_RUNTIME_BASE_URL").ok();

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind executor HTTP listener");

    println!("velara executor listening on {addr}");
    if let Some(ref url) = ai_runtime_base_url {
        println!("  AI Runtime: {url}");
    }

    axum::serve(
        listener,
        velara_executor::http::router_with_config(ai_runtime_base_url),
    )
    .await
    .expect("serve executor HTTP API");
}
