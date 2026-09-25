use std::future::pending;
use std::sync::Arc;

use arc_swap::ArcSwap;
use mnemorium::application::port::initialize_root_admin::InitializeRootAdminUseCase as _;
use mnemorium::application::port::load_configuration::LoadConfigurationUseCase as _;
use mnemorium::application::use_case::initialize_root_admin::InitializeRootAdmin;
use mnemorium::application::use_case::load_configuration::LoadConfiguration;
use mnemorium::infrastructure::inbound::rest::app_state::AppState;
use mnemorium::infrastructure::inbound::rest::handler;
use mnemorium::infrastructure::logging;
use mnemorium::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
use mnemorium::infrastructure::outbound::config::bootstrap::bootstrap_sqlite3;
use mnemorium::infrastructure::outbound::config::configuration_source::ConfigConfigurationSource;
use mnemorium::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
use mnemorium::infrastructure::outbound::random::password_generator::RandomPasswordGenerator;
use mnemorium::infrastructure::outbound::random::secret_generator::ChaChaSecretGenerator;
use mnemorium::infrastructure::outbound::sqlx::sqlite3::init_db;
use mnemorium::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;
use mnemorium::infrastructure::use_case_factory::identity::RuntimeIdentityUseCaseFactory;
use mnemorium::infrastructure::use_case_factory::user::RuntimeUserUseCaseFactory;
use tokio::net::TcpListener;
use tokio::signal::ctrl_c;

use tokio::signal::unix::{SignalKind, signal};

use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    logging::setup();

    let sqlite3 = bootstrap_sqlite3()?;
    let pool = init_db(&sqlite3).await?;

    let unit_of_work_factory = Arc::new(SqlxUnitOfWorkFactory::new(pool));
    let configuration_source = Arc::new(ConfigConfigurationSource::new());
    let load_configuration = Arc::new(LoadConfiguration::new(
        Arc::clone(&unit_of_work_factory),
        configuration_source,
        Arc::new(ChaChaSecretGenerator::new()),
    ));
    let loaded_configuration = load_configuration.execute().await?.configuration().clone();
    let configuration = Arc::new(ArcSwap::from_pointee(loaded_configuration));

    if configuration.load().persistence().sqlite3() != &sqlite3 {
        warn!("sqlite3 settings changed in the configuration; restart to apply them");
    }

    let password_hasher = Arc::new(Argon2PasswordHasher::new(
        configuration.load().security().pepper().as_bytes().to_vec(),
    ));
    let initialize_root_admin = Arc::new(InitializeRootAdmin::new(
        Arc::clone(&unit_of_work_factory),
        password_hasher,
        Arc::new(RandomPasswordGenerator::new()),
    ));

    #[expect(
        clippy::print_stdout,
        reason = "the root admin default password is a sensitive one-time credential; it must be printed to stdout only and never written to the file-based logs"
    )]
    match initialize_root_admin.execute().await {
        Ok(Some(response)) => {
            if configuration.load().security().log_root_admin_password() {
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

    // TODO(hot-reload): the auth middleware holds a `JwtTokenProvider` built from
    // the startup configuration. When the runtime configuration update lands, the
    // middleware must read the live configuration too (config-aware provider or a
    // per-request build), otherwise issued and validated secrets diverge.
    let token_provider = Arc::new(JwtTokenProvider::new(
        configuration.load().security().jwt().secret().to_owned(),
        configuration.load().security().jwt().ttl(),
    ));

    let identity_use_case_factory = Arc::new(RuntimeIdentityUseCaseFactory::new(
        Arc::clone(&configuration),
        Arc::clone(&unit_of_work_factory),
    ));
    let user_use_case_factory = Arc::new(RuntimeUserUseCaseFactory::new(Arc::clone(
        &unit_of_work_factory,
    )));

    let state = AppState::new(
        configuration,
        identity_use_case_factory,
        token_provider,
        user_use_case_factory,
    );

    let app = handler::setup_routes(&state);

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
