use std::future::pending;
use std::sync::Arc;

use config::Config;
use config::Environment;
use config::File;
use mnemorium::application::port::UseCaseCatalog;
use mnemorium::application::port::initialize_root_admin::InitializeRootAdminUseCase as _;
use mnemorium::application::port::load_configuration::LoadConfigurationUseCase as _;
use mnemorium::application::use_case::get_current_user::GetCurrentUser;
use mnemorium::application::use_case::get_user::GetUser;
use mnemorium::application::use_case::initialize_root_admin::InitializeRootAdmin;
use mnemorium::application::use_case::load_configuration::LoadConfiguration;
use mnemorium::application::use_case::login_user::LoginUser as LoginUserUseCase;
use mnemorium::application::use_case::patch_credential::PatchCredential as PatchCredentialUseCase;
use mnemorium::application::use_case::register_user::RegisterUser;
use mnemorium::application::use_case::update_user::UpdateUser;
use mnemorium::infrastructure::inbound::rest::app_state::AppState;
use mnemorium::infrastructure::inbound::rest::handler;
use mnemorium::infrastructure::logging;
use mnemorium::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
use mnemorium::infrastructure::outbound::config::configuration_source::ConfigConfigurationSource;
use mnemorium::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
use mnemorium::infrastructure::outbound::random::password_generator::RandomPasswordGenerator;
use mnemorium::infrastructure::outbound::random::secret_generator::ChaChaSecretGenerator;
use mnemorium::infrastructure::outbound::sqlx::configuration_repository::SqlxConfigurationRepository;
use mnemorium::infrastructure::outbound::sqlx::credential_repository::SqlxCredentialRepository;
use mnemorium::infrastructure::outbound::sqlx::sqlite3::init_db;
use mnemorium::infrastructure::outbound::sqlx::user_repository::SqlxUserRepository;
use tokio::net::TcpListener;
use tokio::signal::ctrl_c;

use tokio::signal::unix::{SignalKind, signal};

use tracing::{error, info, warn};

/// Settings required to open the database before it can be read.
struct BootstrapSqlite3Settings {
    /// Maximum number of connections to the database.
    max_connections: u32,
    /// Path to the `SQLite3` database file.
    path: String,
}

/// Resolve the sqlite3 settings needed to open the database.
///
/// Read from the configuration file and the environment only: the database
/// row cannot participate before the database is opened. Defaults to
/// `mnemorium.db` with a single connection.
///
/// # Errors
///
/// Returns an error when the configuration file cannot be read or parsed.
#[expect(
    clippy::single_call_fn,
    reason = "bootstrap is a distinct pre-database phase of main; inlining it would bury the composition root under file parsing details"
)]
fn bootstrap_sqlite3_settings() -> Result<BootstrapSqlite3Settings, anyhow::Error> {
    const DEFAULT_SQLITE3_MAX_CONN: u32 = 1;
    const DEFAULT_SQLITE3_PATH: &str = "mnemorium.db";

    let settings = Config::builder()
        .add_source(File::with_name("config.yaml").required(false))
        .add_source(
            Environment::with_prefix("mnemorium")
                .separator("__")
                .try_parsing(true)
                .ignore_empty(true),
        )
        .build()?;

    let path = settings
        .get_string("persistence.sqlite3.path")
        .unwrap_or_else(|_| DEFAULT_SQLITE3_PATH.to_owned());
    let max_connections = settings
        .get_int("persistence.sqlite3.max_connections")
        .ok()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SQLITE3_MAX_CONN);

    Ok(BootstrapSqlite3Settings {
        max_connections,
        path,
    })
}

#[tokio::main]
#[expect(
    clippy::too_many_lines,
    reason = "main is the composition root wiring every dependency and use case; extracting any of its single-use blocks collides with clippy::single_call_fn"
)]
async fn main() -> Result<(), anyhow::Error> {
    logging::setup();

    let bootstrap = bootstrap_sqlite3_settings()?;
    let pool = init_db(&bootstrap.path, bootstrap.max_connections).await?;

    let configuration_repository = Arc::new(SqlxConfigurationRepository::new(pool.clone()));
    let configuration_source = Arc::new(ConfigConfigurationSource::new(Arc::clone(
        &configuration_repository,
    )));
    let load_configuration = Arc::new(LoadConfiguration::new(
        Arc::clone(&configuration_repository),
        configuration_source,
        Arc::new(ChaChaSecretGenerator::new()),
    ));
    let configuration = load_configuration.execute().await?.configuration().clone();

    if configuration.persistence().sqlite3().path() != bootstrap.path
        || configuration.persistence().sqlite3().max_connections() != bootstrap.max_connections
    {
        warn!("sqlite3 settings changed in the configuration; restart to apply them");
    }

    let user_repository = Arc::new(SqlxUserRepository::new(pool.clone()));
    let credential_repository = Arc::new(SqlxCredentialRepository::new(pool));

    let password_hasher = Arc::new(Argon2PasswordHasher::new(
        configuration.security().pepper().as_bytes().to_vec(),
    ));
    let register_user = Arc::new(RegisterUser::new(
        Arc::clone(&user_repository),
        Arc::clone(&credential_repository),
        Arc::clone(&password_hasher),
    ));
    let initialize_root_admin = Arc::new(InitializeRootAdmin::new(
        Arc::clone(&user_repository),
        Arc::clone(&credential_repository),
        Arc::clone(&password_hasher),
        Arc::new(RandomPasswordGenerator::new()),
    ));
    let token_provider = Arc::new(JwtTokenProvider::new(
        configuration.security().jwt().secret().to_owned(),
        configuration.security().jwt().ttl(),
    ));
    let get_current_user = Arc::new(GetCurrentUser::new(Arc::clone(&user_repository)));
    let get_user = Arc::new(GetUser::new(Arc::clone(&user_repository)));
    let update_user = Arc::new(UpdateUser::new(Arc::clone(&user_repository)));
    let patch_credential = Arc::new(PatchCredentialUseCase::new(
        Arc::clone(&user_repository),
        Arc::clone(&credential_repository),
        Arc::clone(&password_hasher),
        Arc::clone(&configuration_repository),
    ));
    let login_user = Arc::new(LoginUserUseCase::new(
        user_repository,
        credential_repository,
        password_hasher,
        Arc::clone(&token_provider),
    ));

    #[expect(
        clippy::print_stdout,
        reason = "the root admin default password is a sensitive one-time credential; it must be printed to stdout only and never written to the file-based logs"
    )]
    match initialize_root_admin.execute().await {
        Ok(Some(response)) => {
            if configuration.security().log_root_admin_password() {
                println!(
                    "Root admin initialized; use the default password to authenticate and change it: '{}'",
                    response.default_password()
                );
            } else {
                info!("Root admin initialized; the default password logging is disabled");
            }
        }
        Ok(None) => info!("Root admin already initialized"),
        Err(error) => return Err(error.into()),
    }

    let state = AppState::new(token_provider);
    let catalog = UseCaseCatalog::new(
        get_current_user,
        get_user,
        login_user,
        patch_credential,
        register_user,
        update_user,
    );

    let app = handler::setup_routes(&state, &catalog);

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
