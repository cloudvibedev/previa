#[tokio::main]
async fn main() {
    if let Err(err) = previactl::run().await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
