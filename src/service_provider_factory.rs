use {
    crate::{ServiceCollection, ServiceProvider, Dynamic},
    core::{
        clone::Clone,
        marker::PhantomData,
        any::{Any, TypeId}
    },
    std::sync::Arc
};

///
/// Represents a factory which can efficiently create ServiceProviders from 
/// ServiceCollections which are missing one dependent service T (e.g. Request, StartupConfiguration)
/// The missing service must implement `Any` + `Clone`. Unlike shared services, the reference counter isn't checked
/// to equal zero when the provider is dropped
///
pub struct ServiceProviderFactory<T: Any + Clone> {
    producers: Arc<Vec<(TypeId, *const dyn Fn())>>,
    remaining: PhantomData<T>
}

impl<T: Any + Clone> ServiceProviderFactory<T> {
    pub fn create(mut collection: ServiceCollection) -> Result<Self, super::BuildError> {
        let creator: Box<dyn Fn(&ServiceProvider) -> T> = Box::new(|provider| {
            let pointer: &Arc<T> = unsafe { core::mem::transmute(&provider.root)};
            T::clone(pointer)
        });
        let t_type_id = std::any::TypeId::of::<Dynamic<T>>();
        let function_pointer = Box::into_raw(creator) as *const dyn Fn();

        collection.producers.push((t_type_id, function_pointer));
        let producers = collection.extract_ordered_producers();

        let mut unresolvable_errors = collection.dep_checkers
            .iter()
            .filter_map(|checker| (checker)(&producers));

        match unresolvable_errors.next() {
            Some(err) => {
                // @todo: free
                Err(err)
            },
            None => Ok(ServiceProviderFactory {
                producers: Arc::new(producers),
                remaining: PhantomData
            })
        }        
    }

    pub fn build(&mut self, remaining: T) -> ServiceProvider {  
        ServiceProvider {
            root: unsafe {core::mem::transmute(Arc::new(remaining))},
            producers: self.producers.clone(),
            is_root: true
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ServiceCollection;

    use {super::* };
    
    #[test]
    fn create_provider_with_factory() {
        let mut collection = ServiceCollection::new();
        collection.with::<Dynamic<i32>>().register_transient(|s| s as i64);
        let mut factory = collection.build_factory::<i32>().unwrap();
        let provider = factory.build(42);
        assert_eq!(Some(42i64), provider.get::<Dynamic<i64>>());
    }
}