use {
    crate::{ServiceCollection, ServiceProvider, UntypedFn},
    core::{any::Any, clone::Clone, marker::PhantomData},
    once_cell::sync::OnceCell,
    std::sync::Arc,
};

/// Does all checks to build a ServiceProvider on premise that an instance of T will be available.
/// Therefore multiple ServiceProvider with different scoped information like HttpRequest can be created very efficiently
pub struct ServiceProviderFactory<T: Any + Clone> {
    required_service_states: usize,
    producers: Arc<Vec<UntypedFn>>,
    anticipated: PhantomData<T>,
}

impl<T: Any + Clone> ServiceProviderFactory<T> {
    pub fn create(mut collection: ServiceCollection) -> Result<Self, super::BuildError> {
        let factory: crate::UntypedFnFactory = Box::new(move |&mut _service_state_counter| {
            let creator: Box<dyn Fn(&ServiceProvider) -> T> = Box::new(|provider| {
                let pointer: &Arc<T> = unsafe { core::mem::transmute(&provider.initial_state) };
                T::clone(pointer)
            });
            creator.into()
        });
        collection.producers.push(factory);
        let (producers, required_service_states) = collection.validate_producers()?;

        Ok(ServiceProviderFactory {
            required_service_states,
            producers: Arc::new(producers),
            anticipated: PhantomData,
        })
    }

    pub fn build(&mut self, remaining: T) -> ServiceProvider {
        let service_states = vec![OnceCell::new(); self.required_service_states];
        ServiceProvider {
            service_states: Arc::new(service_states),
            initial_state_destroyer: |state| unsafe {
                drop(core::mem::transmute::<_, Arc<T>>(state))
            },
            initial_state: unsafe { core::mem::transmute(Arc::new(remaining)) },
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
            let first = factory
                .build(())
                .get::<Dynamic<Arc<RefCell<i32>>>>()
                .unwrap();
            assert_eq!(1, first.replace(2));

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
