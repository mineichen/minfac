use super::*;

/// Represents anything resolvable by a ServiceProvider. This 
pub trait Resolvable: Any {
    /// Used if it's uncertain, wether a type is initializable, e.g.
    /// - Option<i32> for provider.get<Singleton<i32>>() 
    type Item: for<'a> FamilyLt<'a>;
    /// If a resolvable is used as a dependency for another service, ServiceCollection::build() ensures
    /// that the dependency can be initialized. So it's currently used:
    /// - provider.get<SingletonServices<i32>>() -> EmptyIterator if nothing is registered
    /// - collection.with::<Singleton<i32>>().register_singleton(|_prechecked_i32: i32| {})
    type ItemPreChecked: for<'a> FamilyLt<'a>;

    /// Resolves a type with the specified provider. There might be multiple calls to this method with
    /// parent ServiceProviders. It will therefore not necessarily be an alias for provider.get() in the future.
    fn resolve<'s>(provider: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out;

    /// Called internally when resolving dependencies.
    fn resolve_prechecked<'s>(provider: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out;

    fn add_resolvable_checker(_: &mut ServiceCollection) {
    }
}

impl Resolvable for () {
    type Item = IdFamily<()>;
    type ItemPreChecked = IdFamily<()>;

    fn resolve<'s>(_: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        ()
    }
    fn resolve_prechecked<'s>(_: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        ()
    }
}

impl<T0: Resolvable, T1: Resolvable> Resolvable for (T0, T1) {
    type Item = (T0::Item, T1::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked);
  
    fn resolve<'s>(provider: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        (provider.get::<T0>(), provider.get::<T1>())
    }
  
    fn resolve_prechecked<'s>(provider: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
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
  
    fn resolve<'s>(provider: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        (provider.get::<T0>(), provider.get::<T1>(), provider.get::<T2>())
    }
    fn resolve_prechecked<'s>(provider: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
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

    fn resolve<'s>(provider: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        (
            provider.get::<T0>(), 
            provider.get::<T1>(),
            provider.get::<T2>(),
            provider.get::<T3>()
        )
    }
    fn resolve_prechecked<'s>(provider: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
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
    type Item = RefFamily<ServiceProvider>;
    type ItemPreChecked  = RefFamily<ServiceProvider>;

    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        container
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        Self::resolve(container)
    }
}

impl<T: Any> resolvable::Resolvable for Singleton<T> {
    type Item = Option<RefFamily<T>>;
    type ItemPreChecked = RefFamily<T>;

    fn resolve<'s>(provider: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        binary_search::binary_search_by_last_key(&provider.producers, &TypeId::of::<Self>(), |(id, _)| id)
            .map(|f| {  
                unsafe { resolve_unchecked::<Self>(provider, f) }
            })
    }

    fn resolve_prechecked<'s>(provider: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        Self::resolve(provider).unwrap()
    }
    fn add_resolvable_checker(col: &mut ServiceCollection) {
        add_dynamic_checker::<Self>(col)
    }
}

/// pos must be a valid index in provider.producers
unsafe fn resolve_unchecked<'a, T: resolvable::Resolvable>(provider: &'a ServiceProvider, pos: usize) -> <T::ItemPreChecked as FamilyLt<'a>>::Out {
    ({
        let entry = provider.producers.get_unchecked(pos);
        debug_assert_eq!(entry.0, TypeId::of::<T>());
        let func_ptr = entry.1 as *const dyn Fn(&'a ServiceProvider) -> <T::ItemPreChecked as FamilyLt<'a>>::Out;
        &* func_ptr
    })(&provider)
}

impl<'a, T: resolvable::Resolvable> std::iter::Iterator for ServiceIterator<'a, T> {
    type Item = <T::ItemPreChecked as FamilyLt<'a>>::Out;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_pos.map(|i| {
            self.next_pos = if let Some(next) = self.provider.producers.get(i + 1) {
                if next.0 == TypeId::of::<T>() { 
                    Some(i + 1) 
                } else {
                    None
                }
            } else {
                None
            };
            
            unsafe { resolve_unchecked::<T>(self.provider, i) }
        })
    }

    fn last(self) -> Option<Self::Item> where Self: Sized {
        self.next_pos.map(|i| {
            // If has_next, last must exist
            let pos = binary_search::binary_search_by_last_key(&self.provider.producers[i..], &TypeId::of::<T>(), |(id, _)| id).unwrap();
            unsafe { resolve_unchecked::<T>(self.provider, i+pos)}            
        }) 
    }
    fn count(self) -> usize where Self: Sized {
        self.next_pos.map(|i| {
            let pos = binary_search::binary_search_by_last_key(&self.provider.producers[i..], &TypeId::of::<T>(), |(id, _)| id).unwrap();
            pos + 1       
        }).unwrap_or(0)
    }
}

pub trait GenericServices {
    type Resolvable: resolvable::Resolvable;
}

impl<TServices: GenericServices + 'static> resolvable::Resolvable for TServices {
    type Item = ServiceIteratorFamily<TServices::Resolvable>;
    type ItemPreChecked = ServiceIteratorFamily<TServices::Resolvable>;

    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        let next_pos = binary_search::binary_search_by_first_key(&container.producers, &TypeId::of::<TServices::Resolvable>(), |(id, _)| id);
        ServiceIterator { provider: &container, item_type: PhantomData, next_pos }
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        Self::resolve(container)
    }
}
impl<T: Any> GenericServices for TransientServices<T> {
    type Resolvable = Transient<T>;
}
impl<T: Any> GenericServices for SingletonServices<T> {
    type Resolvable = Singleton<T>;
}

impl<T: Any> resolvable::Resolvable for Transient<T> {
    type Item = Option<IdFamily<T>>;
    type ItemPreChecked = IdFamily<T>;

    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        binary_search::binary_search_by_last_key(&container.producers, &TypeId::of::<Self>(), |(id, _)| id)
            .map(|f| {    
                unsafe { resolve_unchecked::<Self>(container, f) }
            })
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        Self::resolve(container).unwrap()
    }
    fn add_resolvable_checker(col: &mut ServiceCollection) {
        add_dynamic_checker::<Self>(col)
    }
}

fn add_dynamic_checker<T: resolvable::Resolvable>(col: &mut ServiceCollection) {
    col.dep_checkers.push(Box::new(|col| { 
        col.producers[..].binary_search_by_key(&TypeId::of::<T>(), |(id, _)| { *id }).is_ok()
    }));
}