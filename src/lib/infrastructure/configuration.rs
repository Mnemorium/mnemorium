use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};

use crate::domain::port::secret_generator::SecretGenerator as _;
use crate::infrastructure::outbound::random::secret_generator::ChaChaSecretGenerator;

/// Path to the default configuration file.
const DEFAULT_CONFIG_PATH: &str = "default.yaml";
/// Path to the user configuration file.
const USER_CONFIG_PATH: &str = "config.yaml";
/// Path to the `SQLite3` database file.
const DEFAULT_SQLITE3_PATH: &str = "mnemorium.db";
/// Maximum number of connections to the database.
const DEFAULT_SQLITE3_MAX_CONN: u32 = 1;

/// Length of the generated JWT signing secret.
const JWT_SECRET_LENGTH: u32 = 32;
/// Lifetime of a JWT token, in seconds.
const DEFAULT_JWT_TTL: u64 = 3600;

/// Length of the generated site-wide pepper.
const PEPPER_LENGTH: u32 = 32;

/// Application configuration loaded from a YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    /// Security-related configuration.
    pub security: Security,
    /// `SQLite3` database configuration.
    pub sqlite3: Sqlite3,
}

/// Security-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Security {
    /// Jwt configuration.
    pub jwt: Jwt,
    /// Site-wide secret mixed into password hashes.
    pub pepper: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sqlite3 {
    /// Maximum number of connections to the database.
    pub max_connections: u32,
    /// Path to the `SQLite3` database file.
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwt {
    /// Secret key used to sign and verify JWT tokens.
    pub secret: String,
    /// Lifetime of a JWT token, in seconds.
    pub ttl: u64,
}

impl Configuration {
    /// Load the configuration from the YAML file at `path`.
    ///
    /// When the file does not exist yet, it is created with default/generated
    /// values and return it immediately.
    ///
    /// # Errors
    ///
    /// Returns an error when the pepper cannot be generated, the file cannot be
    /// written, or the YAML file cannot be parsed.
    pub async fn try_new() -> Result<Self, anyhow::Error> {
        if !Path::new(DEFAULT_CONFIG_PATH).exists() {
            let chacha = ChaChaSecretGenerator::new();

            let bytes = chacha.generate(PEPPER_LENGTH).await?;

            let mut hex = String::with_capacity(bytes.len().saturating_mul(2));
            for byte in bytes {
                let _result = write!(hex, "{byte:02x}");
            }

            let jwt_bytes = chacha.generate(JWT_SECRET_LENGTH).await?;

            let mut jwt_hex = String::with_capacity(jwt_bytes.len().saturating_mul(2));
            for byte in jwt_bytes {
                let _result = write!(jwt_hex, "{byte:02x}");
            }

            fs::write(
                DEFAULT_CONFIG_PATH,
                yaml_serde::to_string(&Configuration {
                    security: Security {
                        jwt: Jwt {
                            secret: jwt_hex,
                            ttl: DEFAULT_JWT_TTL,
                        },
                        pepper: hex,
                    },
                    sqlite3: Sqlite3 {
                        max_connections: DEFAULT_SQLITE3_MAX_CONN,
                        path: DEFAULT_SQLITE3_PATH.to_owned(),
                    },
                })?,
            )?;
        }
        let settings: Configuration = Config::builder()
            .add_source(File::with_name(DEFAULT_CONFIG_PATH))
            .add_source(File::with_name(USER_CONFIG_PATH).required(false))
            .add_source(Environment::with_prefix("mnemorium"))
            .build()?
            .try_deserialize()?;

        Ok(settings)
    }
}
