use config::Config;
use config::Environment;
use config::File;

use crate::application::use_case::load_configuration::DEFAULT_SQLITE3_MAX_CONN;
use crate::application::use_case::load_configuration::DEFAULT_SQLITE3_PATH;
use crate::domain::model::sqlite3::Sqlite3;
use crate::infrastructure::outbound::config::configuration_source::USER_CONFIG_PATH;

// TODO(refactor): the bootstrap reads the file and the environment here because
// the datastore path is needed before the configuration singleton (which lives
// in the datastore) can be read. If the bootstrap grows beyond the persistence
// settings, promote it to a use case (a `LoadBootstrapConfiguration` port +
// adapter run by `main` before the pool is opened), or expose it as a method on
// the `ConfigurationSource` port. Either would move this layering behind a port
// and let `main` stop touching the `config` crate entirely. The file and
// environment layering is intentionally duplicated with
// `ConfigConfigurationSource` (see the extraction rule in the StyleGuide); keep
// the source list and order identical.
/// Read the persistence settings from the user configuration file and the
/// environment, before the datastore is reachable.
///
/// The datastore path is needed to open the pool, but the configuration
/// singleton row lives behind that pool: this bootstrap reads only the file and
/// the environment, so the pool can be opened before `LoadConfiguration` runs.
///
/// # Errors
///
/// Returns an error when the layered sources cannot be read or parsed, or when
/// the resolved settings are invalid.
pub fn bootstrap_sqlite3() -> anyhow::Result<Sqlite3> {
    let settings = Config::builder()
        .add_source(File::with_name(USER_CONFIG_PATH).required(false))
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
    Ok(Sqlite3::try_new(path, max_connections)?)
}
