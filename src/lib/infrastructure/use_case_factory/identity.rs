use std::sync::Arc;

use crate::application::port::{
    GetPermissionSummaryUseCase, GetResourcePermissionUseCase, IdentityUseCaseFactory,
    LoginUserUseCase, PatchCredentialUseCase, PutResourcePermissionUseCase, RegenerateTokenUseCase,
    RegisterUserUseCase, RevokeShareUseCase, ShareResourceUseCase,
};
use crate::application::use_case::{
    GetPermissionSummary, GetResourcePermission, LoginUser, PatchCredential, PutResourcePermission,
    RegenerateToken, RegisterUser, RevokeShare, ShareResource,
};
use crate::domain::model::{Configuration, InstanceState};
use crate::infrastructure::outbound::KeyedLoginAttemptLimiter;
use crate::infrastructure::outbound::argon2::Argon2PasswordHasher;
use crate::infrastructure::outbound::jwt::JwtTokenProvider;
use crate::infrastructure::outbound::sqlx::SqlxUnitOfWorkFactory;
use crate::infrastructure::rate_limit::LoginAttemptLimit;

/// Builds the Identity use cases from the immutable configuration and identity.
///
/// Every accessor rebuilds the configuration-derived adapters and wraps the
/// process-lifetime account limit in a fresh login limiter. The pepper and the
/// signing secret come from the immutable instance identity, so building the
/// password hasher from them once per use case is correct; the token lifetime
/// still comes from the configuration.
pub(crate) struct RuntimeIdentityUseCaseFactory {
    /// Immutable application configuration.
    configuration: Arc<Configuration>,
    /// Immutable instance identity carrying the secrets.
    instance_state: Arc<InstanceState>,
    /// Process-lifetime limiter counting login attempts per account.
    login_attempt_limit: Arc<LoginAttemptLimit>,
    /// Factory opening the unit of work wrapping the Identity use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeIdentityUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub(crate) fn new(
        configuration: Arc<Configuration>,
        instance_state: Arc<InstanceState>,
        login_attempt_limit: Arc<LoginAttemptLimit>,
        unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
    ) -> Self {
        Self {
            configuration,
            instance_state,
            login_attempt_limit,
            unit_of_work_factory,
        }
    }

    /// Build the password hasher from the instance pepper.
    fn password_hasher(&self) -> Arc<Argon2PasswordHasher> {
        Arc::new(Argon2PasswordHasher::new(
            self.instance_state.pepper().as_bytes().to_vec(),
        ))
    }

    /// Build the token provider from the identity and the configured lifetime.
    fn token_provider(&self) -> Arc<JwtTokenProvider> {
        Arc::new(JwtTokenProvider::new(
            self.instance_state.jwt_secret().to_owned(),
            self.configuration.security().jwt().ttl(),
        ))
    }
}

impl IdentityUseCaseFactory for RuntimeIdentityUseCaseFactory {
    fn get_permission_summary(&self) -> Arc<dyn GetPermissionSummaryUseCase> {
        Arc::new(GetPermissionSummary::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn get_resource_permission(&self) -> Arc<dyn GetResourcePermissionUseCase> {
        Arc::new(GetResourcePermission::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn login_user(&self) -> Arc<dyn LoginUserUseCase> {
        Arc::new(LoginUser::new(
            Arc::clone(&self.unit_of_work_factory),
            self.password_hasher(),
            self.token_provider(),
            Arc::new(KeyedLoginAttemptLimiter::new(Arc::clone(
                &self.login_attempt_limit,
            ))),
        ))
    }

    fn patch_credential(&self) -> Arc<dyn PatchCredentialUseCase> {
        Arc::new(PatchCredential::new(
            Arc::clone(&self.unit_of_work_factory),
            self.password_hasher(),
        ))
    }

    fn put_resource_permission(&self) -> Arc<dyn PutResourcePermissionUseCase> {
        Arc::new(PutResourcePermission::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn regenerate_token(&self) -> Arc<dyn RegenerateTokenUseCase> {
        Arc::new(RegenerateToken::new(
            Arc::clone(&self.unit_of_work_factory),
            self.token_provider(),
        ))
    }

    fn register_user(&self) -> Arc<dyn RegisterUserUseCase> {
        Arc::new(RegisterUser::new(
            Arc::clone(&self.unit_of_work_factory),
            self.password_hasher(),
        ))
    }

    fn revoke_share(&self) -> Arc<dyn RevokeShareUseCase> {
        Arc::new(RevokeShare::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn share_resource(&self) -> Arc<dyn ShareResourceUseCase> {
        Arc::new(ShareResource::new(Arc::clone(&self.unit_of_work_factory)))
    }
}
