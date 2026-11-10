use std::sync::Arc;

use crate::application::port::{
    DeleteUserUseCase, GetCurrentUserUseCase, GetUserUseCase, ListUsersUseCase, UpdateUserUseCase,
    UserUseCaseFactory,
};
use crate::application::use_case::{DeleteUser, GetCurrentUser, GetUser, ListUsers, UpdateUser};
use crate::infrastructure::outbound::file_system::FileSystemStorage;
use crate::infrastructure::outbound::sqlx::SqlxUnitOfWorkFactory;

/// Builds the User use cases over the shared unit of work factory.
pub(crate) struct RuntimeUserUseCaseFactory {
    /// Factory opening the unit of work wrapping the User use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeUserUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl UserUseCaseFactory for RuntimeUserUseCaseFactory {
    fn delete_user(&self) -> Arc<dyn DeleteUserUseCase> {
        // The file storage adapter is stateless and rebuilt per accessor.
        Arc::new(DeleteUser::new(
            Arc::clone(&self.unit_of_work_factory),
            Arc::new(FileSystemStorage::new()),
        ))
    }

    fn get_current_user(&self) -> Arc<dyn GetCurrentUserUseCase> {
        Arc::new(GetCurrentUser::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_user(&self) -> Arc<dyn GetUserUseCase> {
        Arc::new(GetUser::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn list_users(&self) -> Arc<dyn ListUsersUseCase> {
        Arc::new(ListUsers::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn update_user(&self) -> Arc<dyn UpdateUserUseCase> {
        Arc::new(UpdateUser::new(Arc::clone(&self.unit_of_work_factory)))
    }
}
