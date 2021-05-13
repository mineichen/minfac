use {
    super::*,
    crate::{ServiceCollection, ServiceProducer, ServiceProvider, ServiceProviderImmutableState},
    alloc::sync::Arc,
    core::{any::Any, clone::Clone, marker::PhantomData},
    once_cell::sync::OnceCell,
};

/// Performs all checks to build a ServiceProvider on premise that an instance of type T will be available.
/// Therefore, multiple ServiceProvider with a different base can be created very efficiently.
/// This base could e.g. be the ApplicationSettings for the DomainServices or the HttpContext, if one ServiceProvider
/// is generated per HTTP-Request in the WebApi
pub struct ServiceProviderFactory<T: Any + Clone> {
    service_states_count: usize,
    immutable_state: Arc<ServiceProviderImmutableState>,
    anticipated: PhantomData<T>,
}

pub struct ServiceProviderFactoryBuilder {
    collection: ServiceCollection,
    providers: Vec<WeakServiceProvider>,
}

impl ServiceProviderFactoryBuilder {
    pub fn create(collection: ServiceCollection, first_parent: WeakServiceProvider) -> Self {
        Self {
            collection,
            providers: alloc::vec!(first_parent),
        }
    }
    pub fn build<T: Any + Clone>(self) -> Result<ServiceProviderFactory<T>, super::BuildError> {
        ServiceProviderFactory::create(self.collection, self.providers)
    }
}

impl<T: Any + Clone> ServiceProviderFactory<T> {
    pub fn create(
        mut collection: ServiceCollection,
        parents: Vec<WeakServiceProvider>,
    ) -> Result<Self, super::BuildError> {
        let parent_service_factories: Vec<_> = parents
            .iter()
            .flat_map(|parent| {
                parent
                    .0
                    .immutable_state
                    .producers
                    .iter()
                    .map(move |parent_producer| {
                        // parents are part of ServiceProviderImmutableState to live as long as the inherited UntypedFn
                        let factory = unsafe { parent_producer.bind(&parent.0) };
                        ServiceProducer::new_with_type(
                            Box::new(move |_| Ok(factory)),
                            *parent_producer.get_result_type_id(),
                        )
                    })
            })
            .collect();

        let factory: crate::UntypedFnFactory = Box::new(move |_service_state_counter| {
            let creator: Box<dyn Fn(&ServiceProvider) -> T> =
                Box::new(move |provider| match &provider.service_states.base {
                    Some(x) => x.downcast_ref::<T>().unwrap().clone(),
                    None => panic!("Expected ServiceProviderFactory to set a value for `base`"),
                });
            Ok(creator.into())
        });

        collection
            .producer_factories
            .push(ServiceProducer::new::<T>(factory));

        let (producers, service_states_count) =
            collection.validate_producers(parent_service_factories)?;

        let immutable_state = Arc::new(ServiceProviderImmutableState {
            producers,
            _parents: parents,
        });

        Ok(ServiceProviderFactory {
            service_states_count,
            immutable_state,
            anticipated: PhantomData,
        })
    }

    ///
    /// S
    ///
    /// The ServiceProvider should always be assigned to a variable.
    /// Otherwise, a requested shared service it will outlive its ServiceProvider,
    /// resulting in a panic if debug_assertions are enabled
    /// ```
    /// use {ioc_rs::{Registered, ServiceCollection}, std::sync::Arc};
    /// assert!(std::panic::catch_unwind(|| {
    ///     let mut collection = ServiceCollection::new();
    ///     collection.register_shared(|| Arc::new(42));
    ///     let factory = collection.build_factory().expect("Configuration is valid");
    ///     let x = factory.build(1).get::<Registered<Arc<i32>>>(); // ServiceProvider is dropped too early
    /// }).is_err());
    /// ```
    ///
    pub fn build(&self, remaining: T) -> ServiceProvider {
        let shared_services = alloc::vec![OnceCell::new(); self.service_states_count];

        ServiceProvider {
            service_states: Arc::new(ServiceProviderMutableState {
                shared_services,
                base: Some(Box::new(remaining)),
            }),
            immutable_state: self.immutable_state.clone(),
            is_root: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{AllRegistered, BuildError, Registered},
        core::sync::atomic::{AtomicI32, Ordering},
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
            .build::<i32>()
            .unwrap();
        let child_provider = child_factory.build(2);
        let iterator = child_provider.get::<AllRegistered<i32>>();

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
        let child_factory = child_provider.with_parent(&parent).build::<i64>().unwrap();
        let child1_value = child_factory
            .build(1)
            .get::<Registered<Box<Arc<AtomicI32>>>>()
            .unwrap();
        let child2_value = child_factory
            .build(2)
            .get::<Registered<Arc<AtomicI32>>>()
            .unwrap();
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
        let provider2 = factory.build(2);

        assert_eq!(
            Some(Arc::new(1i64)),
            provider1.get::<Registered<Arc<i64>>>()
        );
        assert_eq!(
            Some(Arc::new(2i64)),
            provider2.get::<Registered<Arc<i64>>>()
        );
    }

    #[test]
    fn arcs_without_dependencies_are_not_shared_between_two_provider_produced_by_the_same_factory()
    {
        let mut collection = ServiceCollection::new();
        collection.register_shared(|| Arc::new(AtomicI32::new(1)));

        let result = collection.build_factory().map(|factory| {
            let first_factory = factory.build(());
            let first = first_factory.get::<Registered<Arc<AtomicI32>>>().unwrap();
            assert_eq!(1, first.fetch_add(1, Ordering::Relaxed));

            let first = first_factory.get::<Registered<Arc<AtomicI32>>>().unwrap();

            let second_factory = factory.build(());
            let second = second_factory.get::<Registered<Arc<AtomicI32>>>().unwrap();

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
            .map(|factory| factory.build(42).get::<Registered<i64>>());
        assert_eq!(Ok(Some(42i64)), result);
    }

    #[test]
    fn create_provider_with_factory_fails_for_missing_dependency() {
        let mut collection = ServiceCollection::new();
        collection.with::<Registered<i32>>().register(|s| s as i64);
        if let Err(BuildError::MissingDependency(infos)) = collection.build_factory::<u32>() {
            assert_eq!(
                infos,
                crate::MissingDependencyType {
                    id: core::any::TypeId::of::<Registered<i32>>(),
                    name: "ioc_rs::Registered<i32>"
                }
            );
        } else {
            panic!("Expected to have missing dependency error");
        }
    }
}
