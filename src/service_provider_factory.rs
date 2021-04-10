use {
    crate::{ServiceCollection, ServiceProvider, UntypedFn, UntypedPointer},
    core::{any::Any, clone::Clone, marker::PhantomData},
    once_cell::sync::OnceCell,
    std::sync::Arc,
};

/// Does all checks to build a ServiceProvider on premise that an instance of T will be available.
/// Therefore multiple ServiceProvider with different scoped information like HttpRequest can be created very efficiently
pub struct ServiceProviderFactory<T: Any + Clone> {
    service_states_count: usize,
    producers: Arc<Vec<UntypedFn>>,
    anticipated: PhantomData<T>,
}

impl<T: Any + Clone> ServiceProviderFactory<T> {
    pub fn create(mut collection: ServiceCollection) -> Result<Self, super::BuildError> {
        let factory: crate::UntypedFnFactory = Box::new(move |service_state_counter| {
            let service_state_idx: usize = *service_state_counter;
            *service_state_counter += 1;

            let creator: Box<dyn Fn(&ServiceProvider) -> T> = Box::new(move |provider| {
                provider.get_or_initialize_pos(service_state_idx, || unreachable!())
            });
            creator.into()
        });

        collection.producer_factories.push(factory);
        let (producers, service_states_count) = collection.validate_producers()?;

        Ok(ServiceProviderFactory {
            service_states_count,
            producers: Arc::new(producers),
            anticipated: PhantomData,
        })
    }

    pub fn build(&mut self, remaining: T) -> ServiceProvider {
        let service_states = vec![OnceCell::new(); self.service_states_count];

        service_states
            .last()
            .unwrap()
            .get_or_init(|| UntypedPointer::new(remaining));

        ServiceProvider {
            service_states: Arc::new(service_states),
            producers: self.producers.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{BuildError, Dynamic},
        std::cell::RefCell,
    };

    #[test]
    fn arcs_with_dependencies_are_not_shared_between_two_provider_produced_by_the_same_factory() {
        let mut collection = ServiceCollection::new();
        collection
            .with::<Dynamic<i32>>()
            .register_arc(|s| Arc::new(s as i64));
        let result = collection.build_factory().map(|mut factory| {
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

        let result = collection.build_factory().map(|mut factory| {
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
            .map(|mut factory| factory.build(42).get::<Dynamic<i64>>());
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
