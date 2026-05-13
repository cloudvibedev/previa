mod models;
mod routes;
mod services;

use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let address = std::env::var("ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_owned());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(55980);
    let bind_addr = format!("{address}:{port}");
    let app = routes::build_app(services::reservations::ReservationStore::from_env());

    let listener = TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind kubernetes plugin");
    info!("previa-kubernetes-plugin listening on {}", bind_addr);
    axum::serve(listener, app)
        .await
        .expect("failed to start kubernetes plugin");
}
