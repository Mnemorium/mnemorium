use std::sync::Arc;

use mnemorium::application::use_case::register_user::RegisterUser;
use mnemorium::infrastructure::inbound::rest::app_state::AppState;
use mnemorium::infrastructure::inbound::rest::bootstrap;
use mnemorium::infrastructure::logging;
use mnemorium::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
use mnemorium::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
use mnemorium::infrastructure::outbound::sqlx::credential_repository::SqlxCredentialRepository;
use mnemorium::infrastructure::outbound::sqlx::user_repository::SqlxUserRepository;
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    logging::setup();

    let user_repository = Arc::new(SqlxUserRepository::new());
    let credential_repository = Arc::new(SqlxCredentialRepository::new());
    let password_hasher = Arc::new(Argon2PasswordHasher::new());
    let register_user = Arc::new(RegisterUser::new(
        user_repository,
        credential_repository,
        password_hasher,
    ));
    let token_provider = Arc::new(JwtTokenProvider::new());

    let state = AppState::new(register_user, token_provider);

    let app = bootstrap::setup_routes(state);

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
