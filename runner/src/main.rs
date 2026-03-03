mod server;

use tokio::net::TcpListener;
use tracing::info;

use crate::server::build_app;
use crate::server::state::AppState;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let state = AppState::default();
    let address = std::env::var("ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_owned());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(55880);
    info!("runner startup config: ADDRESS={}, PORT={}", address, port);
    let bind_addr = format!("{}:{}", address, port);

    let app = build_app(state);

    let listener = TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind listener");
    let local_addr = listener
        .local_addr()
        .expect("failed to read local bind address");

    info!("previa-runner listening on http://{}", local_addr);
    axum::serve(listener, app)
        .await
        .expect("failed to start server");
}
