#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("previactl suporta apenas Linux no momento.");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
mod models;
#[cfg(target_os = "linux")]
mod routes;
#[cfg(target_os = "linux")]
mod services;

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() {
    if let Err(err) = routes::cli::run().await {
        eprintln!("erro: {err:#}");
        std::process::exit(1);
    }
}
