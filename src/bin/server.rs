use std::future::pending;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mnemorium::application::port::ensure_storage_directories::EnsureStorageDirectoriesCommand;
use mnemorium::application::port::ensure_storage_directories::EnsureStorageDirectoriesUseCase as _;
use mnemorium::application::port::initialize_root_admin::InitializeRootAdminUseCase as _;
use mnemorium::application::port::load_configuration::LoadConfigurationUseCase as _;
use mnemorium::application::port::load_state::DATABASE_PATH;
use mnemorium::application::port::load_state::LoadStateUseCase as _;
use mnemorium::application::port::load_state::STATE_PATH;
use mnemorium::application::security_event;
use mnemorium::application::use_case::ensure_storage_directories::EnsureStorageDirectories;
use mnemorium::application::use_case::initialize_root_admin::InitializeRootAdmin;
use mnemorium::application::use_case::load_configuration::LoadConfiguration;
use mnemorium::application::use_case::load_state::LoadState;
use mnemorium::infrastructure::inbound::rest::app_state::AppState;
use mnemorium::infrastructure::inbound::rest::handler;
use mnemorium::infrastructure::inbound::rest::middleware::rate_limit::LoginRateLimiter;
use mnemorium::infrastructure::logging;
use mnemorium::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
use mnemorium::infrastructure::outbound::envy::configuration::EnvConfigurationSource;
use mnemorium::infrastructure::outbound::envy::state_seeds::EnvStateSeedSource;
use mnemorium::infrastructure::outbound::file_system::configuration_source::FileConfigurationSource;
use mnemorium::infrastructure::outbound::file_system::file_storage::FileSystemStorage;
use mnemorium::infrastructure::outbound::file_system::instance_state_store::FileInstanceStateStore;
use mnemorium::infrastructure::outbound::media_probe::LocalMediaProbe;
use mnemorium::infrastructure::outbound::random::password_generator::RandomPasswordGenerator;
use mnemorium::infrastructure::outbound::random::secret_generator::ChaChaSecretGenerator;
use mnemorium::infrastructure::outbound::sqlx::sqlite3::init_db;
use mnemorium::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;
use mnemorium::infrastructure::use_case_factory::asset::RuntimeAssetUseCaseFactory;
use mnemorium::infrastructure::use_case_factory::identity::RuntimeIdentityUseCaseFactory;
use mnemorium::infrastructure::use_case_factory::library::RuntimeLibraryUseCaseFactory;
use mnemorium::infrastructure::use_case_factory::user::RuntimeUserUseCaseFactory;
use tokio::net::TcpListener;
use tokio::signal::ctrl_c;
use tokio::time::sleep;

use tokio::signal::unix::{SignalKind, signal};

use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Identity first: it is existential, and a datastore without identity must
    // fail rather than silently regenerate the key material.
    let loaded_state = LoadState::new(
        Arc::new(FileInstanceStateStore::new(PathBuf::from(STATE_PATH))),
        Arc::new(ChaChaSecretGenerator::new()),
        Arc::new(EnvStateSeedSource::new()),
    )
    .execute()
    .await?;
    let instance_state_created = loaded_state.created();
    let pepper_generated = loaded_state.pepper_generated();
    let jwt_secret_generated = loaded_state.jwt_secret_generated();
    let instance_state = Arc::new(loaded_state.state().clone());

    // Configuration is derivable and datastore-free, so it resolves next.
    let load_configuration = Arc::new(LoadConfiguration::new(
        Arc::new(FileConfigurationSource::new()),
        Arc::new(EnvConfigurationSource::new()),
    ));
    let loaded_configuration = load_configuration.execute().await?;
    let configuration_file_created = loaded_configuration.file_created();
    let configuration = Arc::new(loaded_configuration.configuration().clone());

    let _log_guard = logging::setup(configuration.logging())?;

    // The lifecycle events are emitted here, after `logging::setup`, so they
    // reach a sink (`OBS-006`); identity creation happens before configuration
    // because logging settings are themselves configuration.
    if instance_state_created {
        security_event::system_object("instance_state", "create");
        if pepper_generated {
            security_event::secret_initialized("pepper");
        }
        if jwt_secret_generated {
            security_event::secret_initialized("jwt_secret");
        }
    }
    if configuration_file_created {
        warn!(
            storage_root = configuration.asset().storage().root(),
            datastore = DATABASE_PATH,
            "the configuration file did not exist; wrote the built-in defaults"
        );
    }
    if !loaded_configuration.applied().is_empty() {
        info!(
            settings = ?loaded_configuration.applied(),
            "the environment overrode these configuration settings; its values take precedence over config.yaml"
        );
    }

    let pool = init_db(
        DATABASE_PATH,
        configuration.persistence().sqlite3().max_connections(),
    )
    .await?;

    let unit_of_work_factory = Arc::new(SqlxUnitOfWorkFactory::new(pool));

    let password_hasher = Arc::new(Argon2PasswordHasher::new(
        instance_state.pepper().as_bytes().to_vec(),
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

    let identity_use_case_factory = Arc::new(RuntimeIdentityUseCaseFactory::new(
        Arc::clone(&configuration),
        Arc::clone(&instance_state),
        Arc::clone(&unit_of_work_factory),
    ));
    let user_use_case_factory = Arc::new(RuntimeUserUseCaseFactory::new(Arc::clone(
        &unit_of_work_factory,
    )));
    let library_use_case_factory = Arc::new(RuntimeLibraryUseCaseFactory::new(
        Arc::clone(&configuration),
        Arc::clone(&unit_of_work_factory),
    ));

    // The startup storage directories are created once from the root the Asset
    // and Library factories read.
    let storage_root = PathBuf::from(configuration.asset().storage().root());
    let file_storage = Arc::new(FileSystemStorage::new());
    EnsureStorageDirectories::new(Arc::clone(&file_storage), storage_root)
        .execute(EnsureStorageDirectoriesCommand::new())
        .await?;

    let asset_use_case_factory = Arc::new(RuntimeAssetUseCaseFactory::new(
        Arc::clone(&configuration),
        Arc::clone(&unit_of_work_factory),
        LocalMediaProbe::new_permits(),
    ));

    let login_rate_limiter = Arc::new(LoginRateLimiter::new(configuration.security().rate_limit()));

    let state = AppState::new(
        asset_use_case_factory,
        configuration,
        identity_use_case_factory,
        instance_state,
        library_use_case_factory,
        user_use_case_factory,
    );

    let app = handler::setup_routes(&state, &login_rate_limiter);

    let rate_limit_cleanup_interval = Duration::from_mins(1);
    tokio::spawn(async move {
        loop {
            sleep(rate_limit_cleanup_interval).await;
            login_rate_limiter.prune();
        }
    });

    // Server startup, readiness and shutdown are lifecycle events (`OBS-006`):
    // the catalog declares them at `info` with fields `component` and `outcome`.
    info!(
        target: "security",
        event = "lifecycle",
        component = "server",
        outcome = "starting",
        "starting Mnemorium server"
    );

    match TcpListener::bind("0.0.0.0:4080").await {
        Ok(listener) => {
            info!(
                target: "security",
                event = "lifecycle",
                component = "server",
                outcome = "started",
                "Mnemorium server listening on 0.0.0.0:4080"
            );

            let shutdown_signal = async {
                let interrupt = async {
                    if let Err(err) = ctrl_c().await {
                        error!(kind = ?err.kind(), "Failed to install SIGINT handler");
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
                            error!(kind = ?err.kind(), "Failed to install SIGTERM handler");
                            pending::<()>().await;
                        }
                    }
                };

                tokio::select! {
                    () = interrupt => {}
                    () = terminate => {}
                }

                info!(
                    target: "security",
                    event = "lifecycle",
                    component = "server",
                    outcome = "draining",
                    "shutdown signal received; draining in-flight requests"
                );
            };

            match axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown_signal)
            .await
            {
                Ok(()) => {
                    info!(
                        target: "security",
                        event = "lifecycle",
                        component = "server",
                        outcome = "stopped",
                        "Mnemorium server stopped"
                    );
                }
                Err(err) => {
                    error!(kind = ?err.kind(), "Fail to run axum server");
                }
            }
        }
        Err(err) => {
            error!(kind = ?err.kind(), "Fail to bind TcpListener to port");
        }
    }

    Ok(())
}
