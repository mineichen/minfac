use {
    super::*,
    crate::{
        ServiceCollection, 
        ServiceProducer,
        ServiceProvider,
        ServiceProviderImmutableState,
        untyped::UntypedPointer},
    core::{any::Any, clone::Clone, marker::PhantomData},
    once_cell::sync::OnceCell,
    alloc::sync::Arc,
};

/// Does all checks to build a ServiceProvider on premise that an instance of T will be available.
/// Therefore multiple ServiceProvider with different scoped information like HttpRequest can be created very efficiently
pub struct ServiceProviderFactory<T: Any + Clone> {
    service_states_count: usize,
    immutable_state: Arc<ServiceProviderImmutableState>,
    anticipated: PhantomData<T>,
}

pub struct ServiceProviderFactoryBuilder {
    collection: ServiceCollection,
    providers: Vec<ServiceProvider>
}

impl ServiceProviderFactoryBuilder {
    pub fn create(collection: ServiceCollection, first_parent: ServiceProvider) -> Self {
        Self {
            collection,
            providers: alloc::vec!(first_parent)
        }        
    }
    pub fn build<T: Any + Clone>(self) -> Result<ServiceProviderFactory<T>, super::BuildError> {
        ServiceProviderFactory::create(self.collection, self.providers)
    }
}

const ANTICIPATED_SERVICE_POSITION : usize = 0;

impl<T: Any + Clone> ServiceProviderFactory<T> {
    pub fn create(mut collection: ServiceCollection, parents: Vec<ServiceProvider>) -> Result<Self, super::BuildError> {
        let parent_service_factories: Vec<_> = parents.iter()
            .flat_map(|parent| {
                parent
                .immutable_state
                .producers.iter()
                .map(move |parent_producer| {
                    // parents are part of ServiceProviderImmutableState to live as long as the inherited UntypedFn
                    let factory = unsafe { parent_producer.bind(parent)};
                    ServiceProducer::new_with_type(Box::new(move |_| Ok((factory, None))), *parent_producer.get_result_type_id())
                })
            })
            .collect();
        
        let factory: crate::UntypedFnFactory = Box::new(move |_service_state_counter| {
            let creator: Box<dyn Fn(&ServiceProvider) -> T> = Box::new(move |provider| {
                provider.get_or_initialize_pos(ANTICIPATED_SERVICE_POSITION, || unreachable!())
            });
            Ok((creator.into(), None))
        });

        collection.producer_factories.push(ServiceProducer::new::<T>(factory));

        let mut service_states_count = 1;
        let producers = collection.validate_producers(parent_service_factories, &mut service_states_count)?;
        
        let immutable_state = Arc::new(ServiceProviderImmutableState {
            producers,
            _parents: parents
        });

        Ok(ServiceProviderFactory {
            service_states_count,
            immutable_state,
            anticipated: PhantomData,
        })
    }

    pub fn build(&self, remaining: T) -> ServiceProvider {
        let service_states = alloc::vec![OnceCell::new(); self.service_states_count];

        service_states.get(ANTICIPATED_SERVICE_POSITION)
            .unwrap()
            .get_or_init(|| UntypedPointer::new(remaining));

        ServiceProvider {
            service_states: Arc::new(service_states),
            immutable_state: self.immutable_state.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{BuildError, Dynamic, DynamicServices},
        core::cell::RefCell,
    };

    // todo: Test dropping ServiceProviderFactory doesn't try to free uninitialized

    #[test]
    fn services_are_returned_in_correct_order() {
        let mut parent_provider = ServiceCollection::new();
        parent_provider.register(|| 0);
        let parent = parent_provider.build().expect("Building parent failed unexpectedly");

        let mut child_provider = ServiceCollection::new();
        child_provider.register(|| 1);
        let child_factory = child_provider.with_parent(parent).build::<i32>().unwrap();
        let iterator = child_factory.build(2).get::<DynamicServices<i32>>();
        assert_eq!(alloc::vec!(0, 1, 2), iterator.collect::<Vec<_>>());
    }

    #[test]
    fn uses_same_parent_arc_for_two_providers_from_the_same_child_factory() {
        let mut parent_provider = ServiceCollection::new();
        parent_provider.register_arc(|| Arc::new(RefCell::new(42)));
        let parent = parent_provider.build().expect("Building parent failed unexpectedly");

        let mut child_provider = ServiceCollection::new();
        child_provider.with::<Dynamic<Arc<RefCell<i32>>>>().register(|i| Box::new(i));
        let child_factory = child_provider.with_parent(parent).build::<i64>().unwrap();
        let child1 = child_factory.build(1).get::<Dynamic<Box<Arc<RefCell<i32>>>>>().unwrap();
        let child2 = child_factory.build(2).get::<Dynamic<Arc<RefCell<i32>>>>().unwrap();
        *child1.borrow_mut() = 43;
        assert_eq!(43, *child2.borrow_mut());
    }

    #[test]
    fn arcs_with_dependencies_are_not_shared_between_two_provider_produced_by_the_same_factory() {
        let mut collection = ServiceCollection::new();
        collection
            .with::<Dynamic<i32>>()
            .register_arc(|s| Arc::new(s as i64));
        let result = collection.build_factory().map(|factory| {
            (
                factory.build(1).get::<Dynamic<Arc<i64>>>(),
                factory.build(2).get::<Dynamic<Arc<i64>>>(),
            )
        });

        assert_eq!(Ok((Some(Arc::new(1i64)), Some(Arc::new(2i64)))), result);
    }

    #[test]
    fn arcs_without_dependencies_are_not_shared_between_two_provider_produced_by_the_same_factory()
    {
        let mut collection = ServiceCollection::new();
        collection.register_arc(|| Arc::new(RefCell::new(1)));

        let result = collection.build_factory().map(|factory| {
            let first_factory = factory.build(());
            let first = first_factory.get::<Dynamic<Arc<RefCell<i32>>>>().unwrap();
            assert_eq!(1, first.replace(2));

            let first = first_factory.get::<Dynamic<Arc<RefCell<i32>>>>().unwrap();

            let second = factory
                .build(())
                .get::<Dynamic<Arc<RefCell<i32>>>>()
                .unwrap();

            (first.take(), second.take())
        });

        assert_eq!(Ok((2, 1)), result);
    }

    #[test]
    fn create_provider_with_factory() {
        let mut collection = ServiceCollection::new();
        collection.with::<Dynamic<i32>>().register(|s| s as i64);
        let result = collection
            .build_factory()
            .map(|factory| factory.build(42).get::<Dynamic<i64>>());
        assert_eq!(Ok(Some(42i64)), result);
    }

    #[test]
    fn create_provider_with_factory_fails_for_missing_dependency() {
        let mut collection = ServiceCollection::new();
        collection.with::<Dynamic<i32>>().register(|s| s as i64);
        if let Err(BuildError::MissingDependency(infos)) = collection.build_factory::<u32>() {
            assert_eq!(
                infos,
                crate::MissingDependencyType {
                    id: core::any::TypeId::of::<Dynamic<i32>>(),
                    name: "ioc_rs::Dynamic<i32>"
                }
            );
        } else {
            panic!("Expected to have missing dependency error");
        }
    }
}
