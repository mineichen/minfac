use { super::* };

/// Represents anything resolvable by a ServiceProvider. This 
pub trait Resolvable: Any {
    /// Used if it's uncertain, wether a type is initializable, e.g.
    /// - Option<i32> for provider.get<Singleton<i32>>() 
    type Item;
    /// If a resolvable is used as a dependency for another service, ServiceCollection::build() ensures
    /// that the dependency can be initialized. So it's currently used:
    /// - provider.get<SingletonServices<i32>>() -> EmptyIterator if nothing is registered
    /// - collection.with::<Singleton<i32>>().register_singleton(|_prechecked_i32: i32| {})
    type ItemPreChecked;

    /// Resolves a type with the specified provider. There might be multiple calls to this method with
    /// parent ServiceProviders. It will therefore not necessarily be an alias for provider.get() in the future.
    fn resolve<'s>(provider: &'s ServiceProvider) -> Self::Item;

    /// Called internally when resolving dependencies.
    fn resolve_prechecked<'s>(provider: &'s ServiceProvider) -> Self::ItemPreChecked;

    fn add_resolvable_checker(_: &mut ServiceCollection) {
    }
}

impl Resolvable for () {
    type Item = ();
    type ItemPreChecked = ();

    fn resolve<'s>(_: &'s ServiceProvider) -> Self::Item {
        ()
    }
    fn resolve_prechecked<'s>(_: &'s ServiceProvider) -> Self::ItemPreChecked {
        ()
    }
}

impl<T0: Resolvable, T1: Resolvable> Resolvable for (T0, T1) {
    type Item = (T0::Item, T1::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked);
  
    fn resolve<'s>(provider: &'s ServiceProvider) -> Self::Item {
        (provider.get::<T0>(), provider.get::<T1>())
    }
  
    fn resolve_prechecked<'s>(provider: &'s ServiceProvider) -> Self::ItemPreChecked {
        (T0::resolve_prechecked(provider), T1::resolve_prechecked(provider))
    }

    fn add_resolvable_checker(col: &mut ServiceCollection) {
        T0::add_resolvable_checker(col);
        T1::add_resolvable_checker(col);
    }
}

impl<T0: Resolvable, T1: Resolvable, T2: Resolvable> Resolvable for (T0, T1, T2) {
    type Item = (T0::Item, T1::Item, T2::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked, T2::ItemPreChecked);
  
    fn resolve<'s>(provider: &'s ServiceProvider) -> Self::Item {
        (provider.get::<T0>(), provider.get::<T1>(), provider.get::<T2>())
    }
    fn resolve_prechecked<'s>(provider: &'s ServiceProvider) -> Self::ItemPreChecked {
        (T0::resolve_prechecked(provider), T1::resolve_prechecked(provider), T2::resolve_prechecked(provider))
    }
    fn add_resolvable_checker(collection: &mut ServiceCollection) {
        T0::add_resolvable_checker(collection);
        T1::add_resolvable_checker(collection);
        T2::add_resolvable_checker(collection);
    }
}
impl<T0: Resolvable, T1: Resolvable, T2: Resolvable, T3: Resolvable> Resolvable for (T0, T1, T2, T3) {
    type Item = (T0::Item, T1::Item, T2::Item, T3::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked, T2::ItemPreChecked, T3::ItemPreChecked);

    fn resolve<'s>(provider: &'s ServiceProvider) -> Self::Item {
        (
            provider.get::<T0>(), 
            provider.get::<T1>(),
            provider.get::<T2>(),
            provider.get::<T3>()
        )
    }
    fn resolve_prechecked<'s>(provider: &'s ServiceProvider) -> Self::ItemPreChecked {
        (
            T0::resolve_prechecked(provider), 
            T1::resolve_prechecked(provider),
            T2::resolve_prechecked(provider),
            T3::resolve_prechecked(provider)
        )
    }
    fn add_resolvable_checker(collection: &mut ServiceCollection) {
        T0::add_resolvable_checker(collection);
        T1::add_resolvable_checker(collection);
        T2::add_resolvable_checker(collection);
        T3::add_resolvable_checker(collection);
    }
}

impl Resolvable for ServiceProvider {
    // Doesn't make sense to call from the outside
    type Item = ();
    type ItemPreChecked = ServiceProvider;

    fn resolve<'s>(_container: &'s ServiceProvider) -> Self::Item {
        ()
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> Self::ItemPreChecked {
        ServiceProvider { 
            producers: container.producers.clone(),
            initial_state: container.initial_state.clone()
        }
    }
}

/// pos must be a valid index in provider.producers
unsafe fn resolve_unchecked<'a, T: resolvable::Resolvable>(provider: &'a ServiceProvider, pos: usize) -> T::ItemPreChecked {
    ({
        let entry = provider.producers.get_unchecked(pos);
        debug_assert_eq!(entry.result_type_id, TypeId::of::<T>());
        let func_ptr = entry.pointer as *const dyn Fn(&'a ServiceProvider) -> T::ItemPreChecked;
        &* func_ptr
    })(&provider)
}

impl<'a, T: resolvable::Resolvable> std::iter::Iterator for ServiceIterator<T> {
    type Item = T::ItemPreChecked;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_pos.map(|i| {
            self.next_pos = if let Some(next) = self.provider.producers.get(i + 1) {
                if next.result_type_id == TypeId::of::<T>() { 
                    Some(i + 1) 
                } else {
                    None
                }
            } else {
                None
            };
            
            unsafe { resolve_unchecked::<T>(&self.provider, i) }
        })
    }

    fn last(self) -> Option<Self::Item> where Self: Sized {
        self.next_pos.map(|i| {
            // If has_next, last must exist
            let pos = binary_search::binary_search_by_last_key(&self.provider.producers[i..], &TypeId::of::<T>(), |f| &f.result_type_id).unwrap();
            unsafe { resolve_unchecked::<T>(&self.provider, i+pos)}            
        }) 
    }
    fn count(self) -> usize where Self: Sized {
        self.next_pos.map(|i| {
            let pos = binary_search::binary_search_by_last_key(&self.provider.producers[i..], &TypeId::of::<T>(), |f| &f.result_type_id).unwrap();
            pos + 1       
        }).unwrap_or(0)
    }
}

impl<T: Any> resolvable::Resolvable for DynamicServices<T> {
    type Item = ServiceIterator<Dynamic<T>>;
    type ItemPreChecked = ServiceIterator<Dynamic<T>>;

    fn resolve<'s>(container: &'s ServiceProvider) -> Self::Item {
        let next_pos = binary_search::binary_search_by_first_key(&container.producers, &TypeId::of::<Dynamic<T>>(), |f| &f.result_type_id);
        ServiceIterator { 
            provider: ServiceProvider {
                initial_state: container.initial_state.clone(),
                producers: container.producers.clone()
            }, 
            item_type: PhantomData, 
            next_pos
         }
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> Self::ItemPreChecked {
        Self::resolve(container)
    }
}

impl<T: Any> resolvable::Resolvable for Dynamic<T> {
    type Item = Option<T>;
    type ItemPreChecked = T;

    fn resolve<'s>(container: &'s ServiceProvider) -> Self::Item {
        binary_search::binary_search_by_last_key(&container.producers, &TypeId::of::<Self>(), |f| &f.result_type_id)
            .map(|f| {    
                unsafe { resolve_unchecked::<Self>(container, f) }
            })
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> Self::ItemPreChecked {
        Self::resolve(container).unwrap()
    }
    fn add_resolvable_checker(col: &mut ServiceCollection) {
        col.dep_checkers.push(Box::new(|producers| { 
            match producers[..].binary_search_by_key(&TypeId::of::<Self>(), |f| f.result_type_id) {
                Ok(_) => None,
                Err(_) => Some(BuildError::MissingDependency(
                    MissingDependencyType { 
                        name: std::any::type_name::<Self>(), 
                        id: std::any::TypeId::of::<Self>()
                    } ))
            }
        }))
    }
}