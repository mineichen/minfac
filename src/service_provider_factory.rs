use {
    crate::{ServiceCollection, ServiceProvider, UntypedFn},
    core::{any::Any, clone::Clone, marker::PhantomData},
    std::sync::Arc,
};

/// Does all checks to build a ServiceProvider on premise that an instance of T will be available.
/// Therefore multiple ServiceProvider with different scoped information like HttpRequest can be created very efficiently
pub struct ServiceProviderFactory<T: Any + Clone> {
    producers: Arc<Vec<UntypedFn>>,
    anticipated: PhantomData<T>,
}

impl<T: Any + Clone> ServiceProviderFactory<T> {
    pub fn create(mut collection: ServiceCollection) -> Result<Self, super::BuildError> {
        let creator: Box<dyn Fn(&ServiceProvider) -> T> = Box::new(|provider| {
            let pointer: &Arc<T> = unsafe { core::mem::transmute(&provider.initial_state) };
            T::clone(pointer)
        });

        collection.producers.push(creator.into());
        let producers = collection.validate_producers()?;

        Ok(ServiceProviderFactory {
            producers: Arc::new(producers),
            anticipated: PhantomData,
        })
    }

    pub fn build(&mut self, remaining: T) -> ServiceProvider {
        ServiceProvider {
            initial_state_destroyer: |state| { unsafe { drop(core::mem::transmute::<_, Arc<T>>(state)) }},
            initial_state: unsafe { core::mem::transmute(Arc::new(remaining)) },
            producers: self.producers.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ServiceCollection;

    use crate::{BuildError, Dynamic};

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
