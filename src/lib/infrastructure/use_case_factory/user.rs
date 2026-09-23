use std::sync::Arc;

use crate::application::port::get_current_user::GetCurrentUserUseCase;
use crate::application::port::get_user::GetUserUseCase;
use crate::application::port::update_user::UpdateUserUseCase;
use crate::application::port::user_use_case_factory::UserUseCaseFactory;
use crate::application::use_case::get_current_user::GetCurrentUser;
use crate::application::use_case::get_user::GetUser;
use crate::application::use_case::update_user::UpdateUser;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

/// Builds the User use cases over the shared unit of work factory.
pub struct RuntimeUserUseCaseFactory {
    /// Factory opening the unit of work wrapping the User use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeUserUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl UserUseCaseFactory for RuntimeUserUseCaseFactory {
    fn get_current_user(&self) -> Arc<dyn GetCurrentUserUseCase> {
        Arc::new(GetCurrentUser::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_user_by_id(&self) -> Arc<dyn GetUserUseCase> {
        Arc::new(GetUser::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn update_user(&self) -> Arc<dyn UpdateUserUseCase> {
        Arc::new(UpdateUser::new(Arc::clone(&self.unit_of_work_factory)))
    }
}
