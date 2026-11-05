use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::resolve_music_metadata::ResolveMusicMetadataCommand;
use crate::application::port::resolve_music_metadata::ResolveMusicMetadataError;
use crate::application::port::resolve_music_metadata::ResolveMusicMetadataResponse;
use crate::application::port::resolve_music_metadata::ResolveMusicMetadataUseCase;
use crate::application::security_event;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::music_metadata_provider::MusicMetadataProvider;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

/// Use case implementation for resolving one music target's metadata.
///
/// The use case owns the availability rule: with no configured provider it
/// reports [`ResolveMusicMetadataError::Unavailable`], which is distinct from a
/// provider that does not know the target. A reference another provider issued
/// is [`ResolveMusicMetadataError::UnknownProvider`]. It resolves once through
/// the configured provider and never persists or logs the target. It is guarded
/// by `library.read`: the resolution addresses no catalogue resource, so a
/// denial conceals the whole surface rather than one album.
pub(crate) struct ResolveMusicMetadata<F, P> {
    /// Configured provider, absent when music identification is unavailable.
    metadata_provider: Option<Arc<P>>,
    /// Factory opening the unit of work wrapping the caller load and the lookup.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: MusicMetadataProvider> ResolveMusicMetadata<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, metadata_provider: Option<Arc<P>>) -> Self {
        Self {
            metadata_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, P> ResolveMusicMetadataUseCase for ResolveMusicMetadata<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    P: MusicMetadataProvider,
{
    fn execute<'future>(
        &'future self,
        command: ResolveMusicMetadataCommand,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        Option<ResolveMusicMetadataResponse>,
                        ResolveMusicMetadataError,
                    >,
                > + Send
                + 'future,
        >,
    > {
        let metadata_provider = self.metadata_provider.as_ref().map(Arc::clone);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ResolveMusicMetadataError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ResolveMusicMetadataError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ResolveMusicMetadataError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| ResolveMusicMetadataError::Unknown(error.into()))?;

                // The denial is a security decision the use case owns, so it logs
                // the catalogued `authz_failed` event (`OBS-002`, `OBS-006`); the
                // caller still receives the concealed not-found result.
                if let Decision::Deny(denial) =
                    Authorization::decide_library_surface(&caller, &permissions)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(ResolveMusicMetadataError::NotPermitted);
                }

                let Some(provider) = metadata_provider else {
                    return Err(ResolveMusicMetadataError::Unavailable);
                };
                if command.target().reference().provider() != provider.provider() {
                    return Err(ResolveMusicMetadataError::UnknownProvider);
                }

                let metadata = provider
                    .lookup(command.target())
                    .await
                    .map_err(|error| ResolveMusicMetadataError::Unknown(error.into()))?;
                Ok(metadata.map(ResolveMusicMetadataResponse::new))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ResolveMusicMetadataError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;
    use tracing_test::traced_test;

    use crate::application::port::resolve_music_metadata::ResolveMusicMetadataCommand;
    use crate::application::port::resolve_music_metadata::ResolveMusicMetadataError;
    use crate::application::port::resolve_music_metadata::ResolveMusicMetadataUseCase as _;
    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::music_album::ProductionType;
    use crate::domain::model::music_candidate::MusicTarget;
    use crate::domain::model::music_metadata::ArtistCredit;
    use crate::domain::model::music_metadata::MusicMetadata;
    use crate::domain::model::music_query::MusicKind;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::Scope;
    use crate::domain::port::error::ExternalServiceError;
    use crate::domain::port::music_metadata_provider::MockMusicMetadataProvider;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::any_caller;
    use crate::test_helpers::no_permissions;

    use super::ResolveMusicMetadata;

    /// The caller every test resolves for.
    const CALLER_ID: i64 = 1;

    type UseCase = ResolveMusicMetadata<TestFactory, MockMusicMetadataProvider>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    /// Build a use case over the given permission repository.
    fn use_case_with(
        permissions: MockPermissionRepository,
        provider: Option<MockMusicMetadataProvider>,
    ) -> Harness {
        let unit_of_work = TestUnitOfWork {
            permissions,
            users: any_caller(),
            ..TestUnitOfWork::default()
        };
        let committed = Arc::clone(&unit_of_work.committed);
        let rolled_back = Arc::clone(&unit_of_work.rolled_back);
        let factory = Arc::new(TestFactory::new(unit_of_work));

        Harness {
            committed,
            rolled_back,
            use_case: ResolveMusicMetadata::new(factory, provider.map(Arc::new)),
        }
    }

    /// The target every test resolves.
    fn target() -> MusicTarget {
        MusicTarget::Album(ExternalRef::new(
            ProviderId::MusicBrainz,
            "75b3b57b-5f6f-3c1f-9b7e-9b2f0a2b4b6e".to_owned(),
        ))
    }

    /// The album document a provider reports for [`target`].
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture's date is not a calendar date.
    fn album() -> Result<MusicMetadata, Box<dyn Error>> {
        Ok(MusicMetadata::Album {
            artwork: None,
            artist_credits: vec![ArtistCredit::new("Miles Davis".to_owned(), None)],
            editions: Vec::new(),
            first_release_date: Some(
                NaiveDate::from_ymd_opt(1959, 8, 17).ok_or("the date must exist")?,
            ),
            genres: vec!["jazz".to_owned()],
            production_type: Some(ProductionType::Studio),
            title: "Kind of Blue".to_owned(),
        })
    }

    /// A provider mock that answers for `MusicBrainz`.
    fn musicbrainz_provider() -> MockMusicMetadataProvider {
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_provider()
            .returning(|| ProviderId::MusicBrainz);
        provider
    }

    #[tokio::test]
    async fn resolve_music_metadata_reports_unavailable_without_a_provider()
    -> Result<(), Box<dyn Error>> {
        // Arrange: no music provider is configured.
        let harness = use_case_with(no_permissions(), None);

        // Act
        let result = harness
            .use_case
            .execute(ResolveMusicMetadataCommand::new(CALLER_ID, target()))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ResolveMusicMetadataError::Unavailable)),
            "no configured provider must report unavailable, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn resolve_music_metadata_denies_a_caller_without_library_read()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the caller's library read is overridden to cover nothing.
        let mut permissions = MockPermissionRepository::new();
        let override_scope = vec![Permission::try_new(
            CALLER_ID,
            Action::LibraryRead,
            Scope::None,
        )?];
        permissions
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(override_scope) }));
        let mut provider = musicbrainz_provider();
        provider.expect_lookup().times(0);
        let harness = use_case_with(permissions, Some(provider));

        // Act
        let result = harness
            .use_case
            .execute(ResolveMusicMetadataCommand::new(CALLER_ID, target()))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ResolveMusicMetadataError::NotPermitted)),
            "a caller without library.read must be denied, got: {result:?}"
        );
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"authz_failed\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("a denial must log one authz_failed (found {count})"))
        });
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn resolve_music_metadata_reports_a_reference_another_provider_issued()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the configured provider is MusicBrainz, the target names
        // another provider.
        let mut provider = musicbrainz_provider();
        provider.expect_lookup().times(0);
        let harness = use_case_with(no_permissions(), Some(provider));
        let foreign = MusicTarget::Album(ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()));

        // Act
        let result = harness
            .use_case
            .execute(ResolveMusicMetadataCommand::new(CALLER_ID, foreign))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ResolveMusicMetadataError::UnknownProvider)),
            "a foreign reference must name an unknown provider, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn resolve_music_metadata_reports_a_missing_item_as_no_response()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the provider does not know the target.
        let mut provider = musicbrainz_provider();
        provider
            .expect_lookup()
            .times(1)
            .withf(|target| target.kind() == MusicKind::Album)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = use_case_with(no_permissions(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(ResolveMusicMetadataCommand::new(CALLER_ID, target()))
            .await?;

        // Assert
        assert!(
            response.is_none(),
            "a missing item is no response, not an error"
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn resolve_music_metadata_returns_the_providers_document() -> Result<(), Box<dyn Error>> {
        // Arrange: the provider resolves the target into album metadata.
        let expected = album()?;
        let mut provider = musicbrainz_provider();
        provider.expect_lookup().times(1).return_once(move |_| {
            let copy = expected.clone();
            Box::pin(async move { Ok(Some(copy)) })
        });
        let harness = use_case_with(no_permissions(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(ResolveMusicMetadataCommand::new(CALLER_ID, target()))
            .await?;

        // Assert
        assert_eq!(
            response.map(|resolved| resolved.metadata().clone()),
            Some(album()?)
        );
        Ok(())
    }

    #[tokio::test]
    async fn resolve_music_metadata_reports_a_provider_failure() -> Result<(), Box<dyn Error>> {
        // Arrange: the configured provider fails.
        let mut provider = musicbrainz_provider();
        provider
            .expect_lookup()
            .times(1)
            .returning(|_| Box::pin(async { Err(ExternalServiceError::RateLimited) }));
        let harness = use_case_with(no_permissions(), Some(provider));

        // Act
        let result = harness
            .use_case
            .execute(ResolveMusicMetadataCommand::new(CALLER_ID, target()))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ResolveMusicMetadataError::Unknown(_))),
            "a provider failure must be reported, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
