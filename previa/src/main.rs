#[tokio::main]
async fn main() {
    if let Err(err) = previa::run().await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
