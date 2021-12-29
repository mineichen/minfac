use crate::{
    service_provider::ServiceProviderImmutableState,
    strategy::{Identifyable, Strategy},
    AnyStrategy, GenericServiceCollection, ServiceProducer, ServiceProvider, WeakServiceProvider, ProducerValidationResult,
};
use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};
use core::{clone::Clone, marker::PhantomData};
use once_cell::sync::OnceCell;

/// Performs all checks to build a ServiceProvider on premise that an instance of type T will be available.
/// Therefore, multiple ServiceProvider with a different base can be created very efficiently.
/// This base could e.g. be the ApplicationSettings for the DomainServices or the HttpContext, if one ServiceProvider
/// is generated per HTTP-Request in a WebApi
/// ```
/// use {minfac::{Registered, ServiceCollection}};
///
/// let mut collection = ServiceCollection::new();
/// collection.with::<Registered<i32>>().register(|v| v as i64);
/// let factory = collection.build_factory().expect("Config should be valid");
/// let provider1 = factory.build(1);
/// let provider2 = factory.build(2);
///
/// assert_eq!(Some(1i64), provider1.get::<i64>());
/// assert_eq!(Some(2i64), provider2.get::<i64>());
/// ```
pub struct ServiceProviderFactory<T: Clone + Send + Sync, TS: Strategy = AnyStrategy> {
    service_states_count: usize,
    immutable_state: Arc<crate::service_provider::ServiceProviderImmutableState<TS>>,
    anticipated: PhantomData<T>,
}

pub struct ServiceProviderFactoryBuilder<TS: Strategy> {
    collection: GenericServiceCollection<TS>,
    providers: Vec<WeakServiceProvider<TS>>,
}

impl<TS: Strategy> ServiceProviderFactoryBuilder<TS> {
    pub fn create(
        collection: GenericServiceCollection<TS>,
        first_parent: WeakServiceProvider<TS>,
    ) -> Self {
        Self {
            collection,
            providers: vec![first_parent],
        }
    }
    pub fn build_factory<T: Identifyable<TS::Id> + Clone + Send + Sync>(
        self,
    ) -> Result<ServiceProviderFactory<T, TS>, super::BuildError<TS>> {
        ServiceProviderFactory::<T, TS>::create(self.collection, self.providers)
    }
}

impl<TS: Strategy, T: Identifyable<TS::Id> + Clone + Send + Sync> ServiceProviderFactory<T, TS> {
    pub fn create(
        mut collection: GenericServiceCollection<TS>,
        parents: Vec<WeakServiceProvider<TS>>,
    ) -> Result<Self, super::BuildError<TS>> {
        let parent_service_factories: Vec<_> = parents
            .iter()
            .flat_map(|parent| unsafe { parent.clone_producers() })
            .collect();

        collection
            .producer_factories
            .push(ServiceProducer::<TS>::new::<T>(
                ServiceProvider::<TS>::build_service_producer_for_base::<T>(),
            ));

        let ProducerValidationResult { producers, types, service_states_count} =
            collection.validate_producers(parent_service_factories)?;

        let immutable_state = Arc::new(ServiceProviderImmutableState::<TS>::new(
            types, producers, parents,
        ));

        Ok(ServiceProviderFactory::<_, TS> {
            service_states_count,
            immutable_state,
            anticipated: PhantomData,
        })
    }

    /// The ServiceProvider should always be assigned to a variable.
    /// Otherwise, a requested shared service it will outlive its ServiceProvider,
    /// resulting in a panic if debug_assertions are enabled
    /// ```
    /// # // don't actually run the test, because it fails for "cargo test --release"
    /// # // #[cfg(debug_assertions)] is still enabled for doctest, but not for the actual library
    /// # fn no_op() {
    /// use {minfac::{Registered, ServiceCollection}, std::sync::Arc};
    /// let result = std::panic::catch_unwind(|| {
    ///     let mut collection = ServiceCollection::new();
    ///     collection.register_shared(|| Arc::new(42));
    ///     let factory = collection.build_factory().expect("Configuration is valid");
    ///     let x = factory.build(1).get::<Arc<i32>>(); // ServiceProvider is dropped too early
    /// });
    /// assert!(result.is_err());
    /// # }
    /// ```
    pub fn build(&self, remaining: T) -> ServiceProvider<TS> {
        let shared_services = alloc::vec![OnceCell::new(); self.service_states_count];

        ServiceProvider::new(
            self.immutable_state.clone(),
            shared_services,
            Some(Box::new(remaining)),
        )
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{BuildError, Registered, ServiceCollection},
        core::{
            any::TypeId,
            sync::atomic::{AtomicI32, Ordering},
        },
    };

    #[test]
    fn services_are_returned_in_correct_order() {
        let mut parent_collection = ServiceCollection::new();
        parent_collection.register(|| 0);
        let parent_provider = parent_collection
            .build()
            .expect("Building parent failed unexpectedly");

        let mut child_collection = ServiceCollection::new();
        child_collection.register(|| 1);
        let child_factory = child_collection
            .with_parent(&parent_provider)
            .build_factory::<i32>()
            .unwrap();
        let child_provider = child_factory.build(2);
        let iterator = child_provider.get_all::<i32>();

        assert_eq!(alloc::vec!(0, 1, 2), iterator.collect::<Vec<_>>());
    }

    #[test]
    fn uses_same_parent_arc_for_two_providers_from_the_same_child_factory() {
        let mut parent_provider = ServiceCollection::new();
        parent_provider.register_shared(|| Arc::new(AtomicI32::new(42)));
        let parent = parent_provider
            .build()
            .expect("Building parent failed unexpectedly");

        let mut child_provider = ServiceCollection::new();
        child_provider
            .with::<Registered<Arc<AtomicI32>>>()
            .register(|i| Box::new(i));
        let child_factory = child_provider
            .with_parent(&parent)
            .build_factory::<i64>()
            .unwrap();
        let child1_value = child_factory.build(1).get::<Box<Arc<AtomicI32>>>().unwrap();
        let child2_value = child_factory.build(2).get::<Arc<AtomicI32>>().unwrap();
        child1_value.fetch_add(1, Ordering::Relaxed);
        assert_eq!(43, child2_value.load(Ordering::Relaxed));
    }

    #[test]
    fn arcs_with_dependencies_are_not_shared_between_two_provider_produced_by_the_same_factory() {
        let mut collection = ServiceCollection::new();
        collection
            .with::<Registered<i32>>()
            .register_shared(|s| Arc::new(s as i64));
        let factory = collection.build_factory().unwrap();
        let provider1 = factory.build(1);

        std::thread::spawn(move || {
            assert_eq!(Some(Arc::new(1i64)), provider1.get::<Arc<i64>>());
        })
        .join()
        .unwrap();

        std::thread::spawn(move || {
            let provider2 = factory.build(2);
            assert_eq!(Some(Arc::new(2i64)), provider2.get::<Arc<i64>>());
        })
        .join()
        .unwrap();
    }

    #[test]
    fn arcs_without_dependencies_are_not_shared_between_two_provider_produced_by_the_same_factory()
    {
        let mut collection = ServiceCollection::new();
        collection.register_shared(|| Arc::new(AtomicI32::new(1)));

        let result = collection.build_factory().map(|factory| {
            let first_factory = factory.build(());
            let first = first_factory.get::<Arc<AtomicI32>>().unwrap();
            assert_eq!(1, first.fetch_add(1, Ordering::Relaxed));

            let first = first_factory.get::<Arc<AtomicI32>>().unwrap();

            let second_factory = factory.build(());
            let second = second_factory.get::<Arc<AtomicI32>>().unwrap();

            (
                first.load(Ordering::Relaxed),
                second.load(Ordering::Relaxed),
            )
        });

        assert_eq!(Ok((2, 1)), result);
    }

    #[test]
    fn create_provider_with_factory() {
        let mut collection = ServiceCollection::new();
        collection.with::<Registered<i32>>().register(|s| s as i64);
        let result = collection
            .build_factory()
            .map(|factory| factory.build(42i32).get());
        assert_eq!(Ok(Some(42i64)), result);
    }

    #[test]
    fn create_provider_with_factory_fails_for_missing_dependency() {
        let mut collection = ServiceCollection::new();
        collection.with::<Registered<i32>>().register(|s| s as i64);
        if let Err(BuildError::MissingDependency { id, name }) = collection.build_factory::<u32>() {
            assert_eq!(id, TypeId::of::<i32>());
            assert_eq!(name, "i32");
        } else {
            panic!("Expected to have missing dependency error");
        }
    }
}
