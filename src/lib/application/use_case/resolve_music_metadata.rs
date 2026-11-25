use crate::domain::model::{Action, ResourceKind, ResourceRef};

use crate::application::authorization::authorize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    ResolveMusicMetadataCommand, ResolveMusicMetadataError, ResolveMusicMetadataResponse,
    ResolveMusicMetadataUseCase,
};
use crate::domain::port::{
    IdentityUnitOfWork, MusicMetadataProvider, UnitOfWork as _, UnitOfWorkFactory, UserFilter,
    UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for resolving one music target's metadata.
///
/// The use case owns the availability rule: with no configured provider it
/// reports [`ResolveMusicMetadataError::Unavailable`], which is distinct from a
/// provider that does not know the target. A reference another provider issued
/// is [`ResolveMusicMetadataError::UnknownProvider`]. It resolves once through
/// the configured provider and never persists or logs the target. It is guarded
/// by `ViewMetadata` on the catalogue: the resolution addresses no catalogue resource, so a
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

                // The denial is a security decision the use case owns, so it logs
                // the catalogued `authz_failed` event (`OBS-002`, `OBS-006`); the
                // caller still receives the concealed not-found result.
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Catalogue, 0),
                    Action::ViewMetadata,
                    None,
                )
                .await
                .map_err(|_| ResolveMusicMetadataError::NotPermitted)?;

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
    use super::*;
    use crate::domain::model::{
        ArtistCredit, ExternalRef, MusicKind, MusicMetadata, MusicTarget, ProductionType,
        ProviderId,
    };
    use rstest::fixture;

    use crate::domain::port::{
        ExternalServiceError, MockMusicMetadataProvider, MockResourceRepository,
    };
    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

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

    /// The target every test resolves.
    #[fixture]
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
    #[fixture]
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

    // proves: SCN-METADATA-ENRICHMENT-023-D — Business rule
    #[tokio::test]
    async fn resolve_music_metadata_reports_unavailable_without_a_provider()
    -> Result<(), Box<dyn Error>> {
        // Arrange: no music provider is configured.
        let harness = (move |resources: MockResourceRepository,
                             provider: Option<MockMusicMetadataProvider>|
              -> Harness {
            let unit_of_work = TestUnitOfWork {
                resources,
                ..TestUnitOfWork::with_any_caller()
            };
            let committed = Arc::clone(&unit_of_work.committed);
            let rolled_back = Arc::clone(&unit_of_work.rolled_back);
            let factory = Arc::new(TestFactory::new(unit_of_work));

            Harness {
                committed,
                rolled_back,
                use_case: ResolveMusicMetadata::new(factory, provider.map(Arc::new)),
            }
        })(
            (move || -> crate::domain::port::MockResourceRepository {
                let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })();
                resources
                    .expect_delete()
                    .returning(|_| Box::pin(async { Ok(true) }));
                resources
            })(),
            None,
        );

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
    // proves: SCN-METADATA-ENRICHMENT-008-A — Business rule
    #[tokio::test]
    async fn resolve_music_metadata_reports_a_reference_another_provider_issued()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the configured provider is MusicBrainz, the target names
        // another provider.
        let mut provider = (move || -> MockMusicMetadataProvider {
            let mut provider = MockMusicMetadataProvider::new();
            provider
                .expect_provider()
                .returning(|| ProviderId::MusicBrainz);
            provider
        })();
        provider.expect_lookup().times(0);
        let harness = (move |resources: MockResourceRepository,
                             provider: Option<MockMusicMetadataProvider>|
              -> Harness {
            let unit_of_work = TestUnitOfWork {
                resources,
                ..TestUnitOfWork::with_any_caller()
            };
            let committed = Arc::clone(&unit_of_work.committed);
            let rolled_back = Arc::clone(&unit_of_work.rolled_back);
            let factory = Arc::new(TestFactory::new(unit_of_work));

            Harness {
                committed,
                rolled_back,
                use_case: ResolveMusicMetadata::new(factory, provider.map(Arc::new)),
            }
        })(
            (move || -> crate::domain::port::MockResourceRepository {
                let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })();
                resources
                    .expect_delete()
                    .returning(|_| Box::pin(async { Ok(true) }));
                resources
            })(),
            Some(provider),
        );
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

    // proves: SCN-METADATA-ENRICHMENT-023-E — Negative
    #[tokio::test]
    async fn resolve_music_metadata_reports_a_missing_item_as_no_response()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the provider does not know the target.
        let mut provider = (move || -> MockMusicMetadataProvider {
            let mut provider = MockMusicMetadataProvider::new();
            provider
                .expect_provider()
                .returning(|| ProviderId::MusicBrainz);
            provider
        })();
        provider
            .expect_lookup()
            .times(1)
            .withf(|target| target.kind() == MusicKind::Album)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = (move |resources: MockResourceRepository,
                             provider: Option<MockMusicMetadataProvider>|
              -> Harness {
            let unit_of_work = TestUnitOfWork {
                resources,
                ..TestUnitOfWork::with_any_caller()
            };
            let committed = Arc::clone(&unit_of_work.committed);
            let rolled_back = Arc::clone(&unit_of_work.rolled_back);
            let factory = Arc::new(TestFactory::new(unit_of_work));

            Harness {
                committed,
                rolled_back,
                use_case: ResolveMusicMetadata::new(factory, provider.map(Arc::new)),
            }
        })(
            (move || -> crate::domain::port::MockResourceRepository {
                let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })();
                resources
                    .expect_delete()
                    .returning(|_| Box::pin(async { Ok(true) }));
                resources
            })(),
            Some(provider),
        );

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

    // proves: SCN-METADATA-ENRICHMENT-004-A — Business rule
    // proves: SCN-METADATA-ENRICHMENT-023-B — Positive
    #[tokio::test]
    async fn resolve_music_metadata_returns_the_providers_document() -> Result<(), Box<dyn Error>> {
        // Arrange: the provider resolves the target into album metadata.
        let expected = album()?;
        let mut provider = (move || -> MockMusicMetadataProvider {
            let mut provider = MockMusicMetadataProvider::new();
            provider
                .expect_provider()
                .returning(|| ProviderId::MusicBrainz);
            provider
        })();
        provider.expect_lookup().times(1).return_once(move |_| {
            let copy = expected.clone();
            Box::pin(async move { Ok(Some(copy)) })
        });
        let harness = (move |resources: MockResourceRepository,
                             provider: Option<MockMusicMetadataProvider>|
              -> Harness {
            let unit_of_work = TestUnitOfWork {
                resources,
                ..TestUnitOfWork::with_any_caller()
            };
            let committed = Arc::clone(&unit_of_work.committed);
            let rolled_back = Arc::clone(&unit_of_work.rolled_back);
            let factory = Arc::new(TestFactory::new(unit_of_work));

            Harness {
                committed,
                rolled_back,
                use_case: ResolveMusicMetadata::new(factory, provider.map(Arc::new)),
            }
        })(
            (move || -> crate::domain::port::MockResourceRepository {
                let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })();
                resources
                    .expect_delete()
                    .returning(|_| Box::pin(async { Ok(true) }));
                resources
            })(),
            Some(provider),
        );

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

    // proves: SCN-METADATA-ENRICHMENT-008-B — Error handling
    #[tokio::test]
    async fn resolve_music_metadata_reports_a_provider_failure() -> Result<(), Box<dyn Error>> {
        // Arrange: the configured provider fails.
        let mut provider = (move || -> MockMusicMetadataProvider {
            let mut provider = MockMusicMetadataProvider::new();
            provider
                .expect_provider()
                .returning(|| ProviderId::MusicBrainz);
            provider
        })();
        provider
            .expect_lookup()
            .times(1)
            .returning(|_| Box::pin(async { Err(ExternalServiceError::RateLimited) }));
        let harness = (move |resources: MockResourceRepository,
                             provider: Option<MockMusicMetadataProvider>|
              -> Harness {
            let unit_of_work = TestUnitOfWork {
                resources,
                ..TestUnitOfWork::with_any_caller()
            };
            let committed = Arc::clone(&unit_of_work.committed);
            let rolled_back = Arc::clone(&unit_of_work.rolled_back);
            let factory = Arc::new(TestFactory::new(unit_of_work));

            Harness {
                committed,
                rolled_back,
                use_case: ResolveMusicMetadata::new(factory, provider.map(Arc::new)),
            }
        })(
            (move || -> crate::domain::port::MockResourceRepository {
                let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })();
                resources
                    .expect_delete()
                    .returning(|_| Box::pin(async { Ok(true) }));
                resources
            })(),
            Some(provider),
        );

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
