use tonic::initialize_tracing;
use tracing::Level;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_tracing(Level::INFO);

    Ok(())
}
