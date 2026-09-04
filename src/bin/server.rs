use mnemorium::infrastructure::{inbound::rest::bootstrap, logging};
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    logging::setup();

    let app = bootstrap::setup_routes();

    info!("Starting Mnemorium server");

    match TcpListener::bind("0.0.0.0:4080").await {
        Ok(listener) => {
            match axum::serve(listener, app).await {
                Ok(()) => {
                    info!("Mnemorium server started");
                }
                Err(err) => {
                    error!("Fail to start axum server: {}", err);
                }
            }

            info!("Mnemorium server stopped");
        }
        Err(err) => {
            error!("Fail to bind TcpListener to port: {}", err);
        }
    }

    Ok(())
}
