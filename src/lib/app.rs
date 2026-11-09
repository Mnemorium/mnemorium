use std::future::pending;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::signal::ctrl_c;
use tokio::signal::unix::{SignalKind, signal};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::application::port::asset_use_case_factory::AssetUseCaseFactory;
use crate::application::port::ensure_storage_directories::{
    EnsureStorageDirectoriesCommand, EnsureStorageDirectoriesUseCase as _,
};
use crate::application::port::initialize_root_admin::InitializeRootAdminUseCase as _;
use crate::application::port::library_use_case_factory::LibraryUseCaseFactory;
use crate::application::port::load_configuration::LoadConfigurationUseCase as _;
use crate::application::port::load_state::{DATABASE_PATH, LoadStateUseCase as _, STATE_PATH};
use crate::application::security_event;
use crate::application::use_case::ensure_storage_directories::EnsureStorageDirectories;
use crate::application::use_case::initialize_root_admin::InitializeRootAdmin;
use crate::application::use_case::load_configuration::LoadConfiguration;
use crate::application::use_case::load_state::LoadState;
use crate::application::use_case::upload_layout::MEDIA_ROOT;
#[cfg(feature = "test-seed")]
use crate::application::use_case::upload_layout::media_root;
use crate::infrastructure::inbound::media_ingest::MediaIngestRunner;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler;
use crate::infrastructure::inbound::rest::middleware::rate_limit::LoginRateLimiter;
use crate::infrastructure::inbound::upload_reaper::UploadReaperRunner;
use crate::infrastructure::logging;
use crate::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
use crate::infrastructure::outbound::envy::configuration::EnvConfigurationSource;
use crate::infrastructure::outbound::envy::state_seeds::EnvStateSeedSource;
use crate::infrastructure::outbound::file_system::configuration_source::FileConfigurationSource;
use crate::infrastructure::outbound::file_system::file_storage::FileSystemStorage;
use crate::infrastructure::outbound::file_system::instance_state_store::FileInstanceStateStore;
use crate::infrastructure::outbound::media_probe::LocalMediaProbe;
use crate::infrastructure::outbound::random::password_generator::RandomPasswordGenerator;
use crate::infrastructure::outbound::random::secret_generator::ChaChaSecretGenerator;
use crate::infrastructure::outbound::sqlx::init_db;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;
#[cfg(feature = "test-seed")]
use crate::infrastructure::seed;
use crate::infrastructure::use_case_factory::asset::RuntimeAssetUseCaseFactory;
use crate::infrastructure::use_case_factory::identity::RuntimeIdentityUseCaseFactory;
use crate::infrastructure::use_case_factory::library::RuntimeLibraryUseCaseFactory;
use crate::infrastructure::use_case_factory::user::RuntimeUserUseCaseFactory;

/// Run the Mnemorium server until a shutdown signal arrives.
///
/// # Errors
///
/// Returns an error when the instance identity, the configuration, the
/// datastore or the logging subscriber cannot be established.
pub async fn run() -> anyhow::Result<()> {
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

    #[cfg(feature = "test-seed")]
    let seed_pool = pool.clone();

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
    let probe_permits = LocalMediaProbe::new_permits();
    let library_use_case_factory: Arc<dyn LibraryUseCaseFactory> =
        Arc::new(RuntimeLibraryUseCaseFactory::new(
            Arc::clone(&unit_of_work_factory),
            Arc::clone(&probe_permits),
            Arc::clone(&configuration),
        ));

    #[cfg(feature = "test-seed")]
    seed::apply(&seed_pool, instance_state.pepper(), &media_root()).await?;

    let file_storage = Arc::new(FileSystemStorage::new());
    let startup = configuration.library().startup();
    let media_root_wait = Duration::from_secs(startup.media_root_wait_seconds());
    let storage_directories = EnsureStorageDirectories::new(Arc::clone(&file_storage))
        .execute(EnsureStorageDirectoriesCommand::new().waiting(
            startup.media_root_wait_seconds(),
            startup.fail_without_media_root(),
        ))
        .await?;
    if startup.fail_without_media_root() && !storage_directories.media_root_present() {
        return Err(anyhow::anyhow!(
            "the library root '{MEDIA_ROOT}' is absent; startup refuses to run while library.startup.fail_without_media_root is set"
        ));
    }

    let asset_use_case_factory = Arc::new(RuntimeAssetUseCaseFactory::new(
        Arc::clone(&configuration),
        Arc::clone(&instance_state),
        Arc::clone(&unit_of_work_factory),
        Arc::clone(&probe_permits),
    ));

    let login_rate_limiter = Arc::new(LoginRateLimiter::new(configuration.security().rate_limit()));

    // The reaper reads its interval once, at startup (`STY-RUST-076`), and the
    // factory each tick for the live batch size.
    let upload_reap_interval =
        Duration::from_secs(configuration.asset().upload().reap_interval_seconds());
    let reaper_factory: Arc<dyn AssetUseCaseFactory> = Arc::clone(&asset_use_case_factory) as _;

    let state = AppState::new(
        asset_use_case_factory,
        configuration,
        identity_use_case_factory,
        instance_state,
        Arc::clone(&library_use_case_factory),
        user_use_case_factory,
    );

    let app = handler::setup_routes(&state, &login_rate_limiter);

    // The library ingest starts only after the router is built, so readiness
    // never waits for a large library. The scan and the observer both drain on
    // the shutdown signal below.
    let media_ingest = Arc::new(
        MediaIngestRunner::new(
            library_use_case_factory,
            storage_directories.media_root_present(),
            media_root_wait,
        )
        .start(),
    );

    // The upload reaper starts with the server and never delays readiness: its
    // first pass runs after one full interval.
    let upload_reaper =
        Arc::new(UploadReaperRunner::new(reaper_factory, upload_reap_interval).start());

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

            let media_ingest_signal = Arc::clone(&media_ingest);
            let upload_reaper_signal = Arc::clone(&upload_reaper);
            let shutdown_signal = async move {
                let interrupt = async {
                    if let Err(err) = ctrl_c().await {
                        error!(kind = ?err.kind(), "Failed to install SIGINT handler");
                        pending::<()>().await;
                    }
                };

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
                // Stop the background drivers before axum drains, so neither a
                // scan, an observer nor a reaping pass outlives the process.
                media_ingest_signal.shutdown();
                upload_reaper_signal.shutdown();
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
            media_ingest.wait().await;
            upload_reaper.wait().await;
        }
        Err(err) => {
            media_ingest.shutdown();
            upload_reaper.shutdown();
            media_ingest.wait().await;
            upload_reaper.wait().await;
            error!(kind = ?err.kind(), "Fail to bind TcpListener to port");
        }
    }

    Ok(())
}
