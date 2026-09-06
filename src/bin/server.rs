use std::future::pending;
use std::sync::Arc;

use mnemorium::application::use_case::login_user::LoginUser as LoginUserUseCase;
use mnemorium::application::use_case::register_user::RegisterUser;
use mnemorium::infrastructure::inbound::rest::app_state::AppState;
use mnemorium::infrastructure::inbound::rest::bootstrap;
use mnemorium::infrastructure::logging;
use mnemorium::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
use mnemorium::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
use mnemorium::infrastructure::outbound::sqlx::credential_repository::SqlxCredentialRepository;
use mnemorium::infrastructure::outbound::sqlx::sqlite3::init_db;
use mnemorium::infrastructure::outbound::sqlx::user_repository::SqlxUserRepository;
use tokio::net::TcpListener;
use tokio::signal::ctrl_c;

use tokio::signal::unix::{SignalKind, signal};

use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    logging::setup();

    // TODO: use configuration values
    let pool = init_db("mnemorium.db", 1).await?;

    let user_repository = Arc::new(SqlxUserRepository::new(pool.clone()));
    let credential_repository = Arc::new(SqlxCredentialRepository::new(pool));
    let password_hasher = Arc::new(Argon2PasswordHasher::new());
    let register_user = Arc::new(RegisterUser::new(
        Arc::clone(&user_repository),
        Arc::clone(&credential_repository),
        Arc::clone(&password_hasher),
    ));
    let token_provider = Arc::new(JwtTokenProvider::new());
    let login_user = Arc::new(LoginUserUseCase::new(
        user_repository,
        credential_repository,
        password_hasher,
        Arc::clone(&token_provider),
    ));

    let state = AppState::new(register_user, login_user, token_provider);

    let app = bootstrap::setup_routes(&state);

    info!("Starting Mnemorium server");

    match TcpListener::bind("0.0.0.0:4080").await {
        Ok(listener) => {
            info!("Mnemorium server listening on 0.0.0.0:4080");

            let shutdown_signal = async {
                let interrupt = async {
                    if let Err(err) = ctrl_c().await {
                        error!("Failed to install SIGINT handler: {}", err);
                        pending::<()>().await;
                    }
                };

                #[cfg(unix)]
                let terminate = async {
                    match signal(SignalKind::terminate()) {
                        Ok(mut stream) => {
                            let _signal = stream.recv().await;
                        }
                        Err(err) => {
                            error!("Failed to install SIGTERM handler: {}", err);
                            pending::<()>().await;
                        }
                    }
                };

                tokio::select! {
                    () = interrupt => {}
                    () = terminate => {}
                }

                info!("Shutdown signal received; draining in-flight requests");
            };

            match axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal)
                .await
            {
                Ok(()) => {
                    info!("Mnemorium server stopped");
                }
                Err(err) => {
                    error!("Fail to run axum server: {}", err);
                }
            }
        }
        Err(err) => {
            error!("Fail to bind TcpListener to port: {}", err);
        }
    }

    Ok(())
}
