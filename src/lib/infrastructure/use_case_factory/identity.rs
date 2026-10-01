use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory;
use crate::application::port::login_user::LoginUserUseCase;
use crate::application::port::patch_credential::PatchCredentialUseCase;
use crate::application::port::register_user::RegisterUserUseCase;
use crate::application::use_case::login_user::LoginUser;
use crate::application::use_case::patch_credential::PatchCredential;
use crate::application::use_case::register_user::RegisterUser;
use crate::domain::model::configuration::Configuration;
use crate::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

/// Builds the Identity use cases from the live configuration.
///
/// Every accessor rebuilds the configuration-derived adapters (password hasher,
/// token provider) from the current configuration, so a runtime configuration
/// change is picked up by the next request.
pub struct RuntimeIdentityUseCaseFactory {
    /// Live application configuration.
    configuration: Arc<ArcSwap<Configuration>>,
    /// Factory opening the unit of work wrapping the Identity use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeIdentityUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub fn new(
        configuration: Arc<ArcSwap<Configuration>>,
        unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
    ) -> Self {
        Self {
            configuration,
            unit_of_work_factory,
        }
    }
}

impl IdentityUseCaseFactory for RuntimeIdentityUseCaseFactory {
    fn login_user(&self) -> Arc<dyn LoginUserUseCase> {
        let live = self.configuration.load();
        let password_hasher = Arc::new(Argon2PasswordHasher::new(
            live.security().pepper().as_bytes().to_vec(),
        ));
        let token_provider = Arc::new(JwtTokenProvider::new(
            live.security().jwt().secret().to_owned(),
            live.security().jwt().ttl(),
        ));
        Arc::new(LoginUser::new(
            Arc::clone(&self.unit_of_work_factory),
            password_hasher,
            token_provider,
        ))
    }

    fn patch_credential(&self) -> Arc<dyn PatchCredentialUseCase> {
        let live = self.configuration.load();
        let password_hasher = Arc::new(Argon2PasswordHasher::new(
            live.security().pepper().as_bytes().to_vec(),
        ));
        Arc::new(PatchCredential::new(
            Arc::clone(&self.unit_of_work_factory),
            password_hasher,
        ))
    }

    fn register_user(&self) -> Arc<dyn RegisterUserUseCase> {
        let live = self.configuration.load();
        let password_hasher = Arc::new(Argon2PasswordHasher::new(
            live.security().pepper().as_bytes().to_vec(),
        ));
        Arc::new(RegisterUser::new(
            Arc::clone(&self.unit_of_work_factory),
            password_hasher,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::iter::repeat_n;
    use std::sync::Arc;

    use arc_swap::ArcSwap;
    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory as _;
    use crate::application::port::login_user::LoginUserCommand;
    use crate::application::port::login_user::LoginUserError;
    use crate::application::port::patch_credential::PatchCredentialCommand;
    use crate::application::port::register_user::RegisterUserCommand;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::jwt::Jwt;
    use crate::domain::model::logging::Logging;
    use crate::domain::model::logging::Rotation;
    use crate::domain::model::persistence::Persistence;
    use crate::domain::model::security::Security;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::model::user::Role;
    use crate::domain::port::password_hasher::PasswordHasher as _;
    use crate::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
    use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

    use super::RuntimeIdentityUseCaseFactory;

    /// A 64-character hexadecimal string, valid for secrets and peppers.
    fn hex64(character: char) -> String {
        repeat_n(character, 64).collect()
    }

    /// An in-memory database migrated to the current schema, capped at one
    /// connection so a unit of work owns the only connection.
    async fn migrated_pool() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(pool)
    }

    /// A valid configuration built around `pepper`, `secret` and `ttl`.
    fn configuration(
        pepper: &str,
        secret: &str,
        ttl: u64,
    ) -> Result<Configuration, Box<dyn Error>> {
        let jwt = Jwt::try_new(secret.to_owned(), ttl)?;
        let security = Security::try_new(jwt, pepper.to_owned(), true)?;
        let sqlite3 = Sqlite3::try_new(":memory:".to_owned(), 1)?;
        let logging = Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?;
        Ok(Configuration::new(
            Persistence::new(sqlite3),
            security,
            logging,
        ))
    }

    /// Insert a credential row directly into `pool`.
    async fn seed_credential(
        pool: &SqlitePool,
        id: i64,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(id)
            .bind(password_hash)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Insert a user row directly into `pool`.
    async fn seed_user(
        pool: &SqlitePool,
        id: i64,
        role: &str,
        username: &str,
        credential_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (?1, ?2, ?3, NULL, ?4)",
        )
        .bind(id)
        .bind(role)
        .bind(username)
        .bind(credential_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Read the password hash behind `username`'s credential.
    async fn credential_hash(pool: &SqlitePool, username: &str) -> Result<String, sqlx::Error> {
        let credential_id: i64 =
            sqlx::query_scalar("SELECT credential_id FROM user WHERE username = ?1")
                .bind(username)
                .fetch_one(pool)
                .await?;
        read_credential_hash(pool, credential_id).await
    }

    /// Read the password hash behind credential `id`.
    async fn read_credential_hash(pool: &SqlitePool, id: i64) -> Result<String, sqlx::Error> {
        sqlx::query_scalar("SELECT password_hash FROM credential WHERE credential_id = ?1")
            .bind(id)
            .fetch_one(pool)
            .await
    }

    #[tokio::test]
    async fn login_user_after_configuration_swap_uses_live_ttl_and_pepper()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = migrated_pool().await?;
        let pepper = hex64('1');
        let secret = hex64('a');
        let hasher = Argon2PasswordHasher::new(pepper.as_bytes().to_vec());
        let hash = hasher.hash_password("LivePepper!1").await?;
        seed_credential(&pool, 1, &hash).await?;
        seed_user(&pool, 1, "STANDARD", "alice", 1).await?;
        let live = Arc::new(ArcSwap::from_pointee(configuration(
            &pepper, &secret, 3600,
        )?));
        let factory = RuntimeIdentityUseCaseFactory::new(
            Arc::clone(&live),
            Arc::new(SqlxUnitOfWorkFactory::new(pool.clone())),
        );

        // Act
        let first = factory
            .login_user()
            .execute(LoginUserCommand::new(
                "alice".to_owned(),
                "LivePepper!1".to_owned(),
            ))
            .await?;
        live.store(Arc::new(configuration(&pepper, &secret, 7200)?));
        let second = factory
            .login_user()
            .execute(LoginUserCommand::new(
                "alice".to_owned(),
                "LivePepper!1".to_owned(),
            ))
            .await?;
        live.store(Arc::new(configuration(&hex64('2'), &secret, 7200)?));
        let stale = factory
            .login_user()
            .execute(LoginUserCommand::new(
                "alice".to_owned(),
                "LivePepper!1".to_owned(),
            ))
            .await;

        // Assert
        assert_eq!(first.expires_in(), 3600);
        assert_eq!(second.expires_in(), 7200);
        assert!(matches!(stale, Err(LoginUserError::InvalidPassword)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_after_pepper_swap_hashes_with_live_pepper() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let pool = migrated_pool().await?;
        let pepper_one = hex64('1');
        let pepper_two = hex64('2');
        let secret = hex64('a');
        seed_credential(&pool, 0, "root-hash").await?;
        seed_user(&pool, 0, "ADMIN", "root", 0).await?;
        let live = Arc::new(ArcSwap::from_pointee(configuration(
            &pepper_one,
            &secret,
            3600,
        )?));
        let factory = RuntimeIdentityUseCaseFactory::new(
            Arc::clone(&live),
            Arc::new(SqlxUnitOfWorkFactory::new(pool.clone())),
        );

        // Act
        factory
            .register_user()
            .execute(RegisterUserCommand::new(
                0,
                "alice".to_owned(),
                None,
                "Register!1".to_owned(),
                Role::Standard,
            ))
            .await?;
        let alice_hash = credential_hash(&pool, "alice").await?;
        live.store(Arc::new(configuration(&pepper_two, &secret, 3600)?));
        factory
            .register_user()
            .execute(RegisterUserCommand::new(
                0,
                "bobby".to_owned(),
                None,
                "Register!2".to_owned(),
                Role::Standard,
            ))
            .await?;
        let bob_hash = credential_hash(&pool, "bobby").await?;
        let first_hasher = Argon2PasswordHasher::new(pepper_one.as_bytes().to_vec());
        let second_hasher = Argon2PasswordHasher::new(pepper_two.as_bytes().to_vec());

        // Assert
        assert!(
            first_hasher
                .verify_password("Register!1", &alice_hash)
                .await?
        );
        assert!(
            second_hasher
                .verify_password("Register!2", &bob_hash)
                .await?
        );
        assert!(
            !first_hasher
                .verify_password("Register!2", &bob_hash)
                .await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_after_pepper_swap_hashes_with_live_pepper()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = migrated_pool().await?;
        let pepper_one = hex64('1');
        let pepper_two = hex64('2');
        let secret = hex64('a');
        seed_credential(&pool, 0, "root-hash").await?;
        seed_credential(&pool, 2, "target-old-hash").await?;
        seed_user(&pool, 0, "ADMIN", "root", 0).await?;
        let live = Arc::new(ArcSwap::from_pointee(configuration(
            &pepper_one,
            &secret,
            3600,
        )?));
        let factory = RuntimeIdentityUseCaseFactory::new(
            Arc::clone(&live),
            Arc::new(SqlxUnitOfWorkFactory::new(pool.clone())),
        );

        // Act
        factory
            .patch_credential()
            .execute(PatchCredentialCommand::new(0, 2, "Patch!111".to_owned()))
            .await?;
        let first_hash = read_credential_hash(&pool, 2).await?;
        live.store(Arc::new(configuration(&pepper_two, &secret, 3600)?));
        factory
            .patch_credential()
            .execute(PatchCredentialCommand::new(0, 2, "Patch!222".to_owned()))
            .await?;
        let second_hash = read_credential_hash(&pool, 2).await?;
        let first_hasher = Argon2PasswordHasher::new(pepper_one.as_bytes().to_vec());
        let second_hasher = Argon2PasswordHasher::new(pepper_two.as_bytes().to_vec());

        // Assert
        assert!(
            first_hasher
                .verify_password("Patch!111", &first_hash)
                .await?
        );
        assert!(
            second_hasher
                .verify_password("Patch!222", &second_hash)
                .await?
        );
        assert!(
            !first_hasher
                .verify_password("Patch!222", &second_hash)
                .await?
        );
        Ok(())
    }
}
