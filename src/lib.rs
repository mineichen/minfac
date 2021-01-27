
//! # IOC framework inspired by .Net's Microsoft.Extensions.DependencyInjection
//! 
//! Complete example:
//! ```
//! use ioc_rs::{ServiceCollection, Singleton, SingletonServices, Transient};
//! let mut collection = ioc_rs::ServiceCollection::new();
//! 
//! collection
//!     .with::<Transient<i16>>()
//!     .register_transient(|i| i as i32 * 2);
//! collection
//!     .with::<(SingletonServices<i8>, Transient<i16>, Transient<i32>)>()
//!     .register_transient(|(bytes, short, int)| bytes.map(|i| *i as i64).sum::<i64>() + short as i64 + int as i64);
//! collection.register_singleton(|| 1i8);
//! collection.register_singleton(|| 2i8);
//! collection.register_singleton(|| 3i8);
//! collection.register_transient(|| 4i16);
//!
//! let provider = collection.build().expect("All dependencies are resolvable");
//! assert_eq!(Some(&3), provider.get::<Singleton<i8>>()); // Last registered i8
//! assert_eq!(Some(1+2+3+4+(2*4)), provider.get::<Transient<i64>>()); // composed i64
//! ```
//! # Notes
//! - Registration is order independent
//! - Registration can occur in separately compiled dynamic lib (see /examples)
//! - Types requested as dependencies (.with<>()) are, in contrast to ServiceProvider.get(), not Options, because their existance is asserted at ServiceCollection.build()
//! 
//! Visit the documentation for more details

use {
    core::{
        marker::PhantomData,
        any::{Any, TypeId}
    },
    family_lifetime::FamilyLt,
    resolvable::Resolvable
};

mod resolvable;
mod binary_search;
mod family_lifetime;

pub struct ServiceIterator<'a, T> {
    next_pos: Option<usize>,
    provider: &'a ServiceProvider, 
    item_type: PhantomData<T>
}

pub struct Singleton<T: Any>(PhantomData<T>);
pub struct Transient<T: Any>(PhantomData<T>);
pub struct TransientServices<T: Any>(PhantomData<T>);
pub struct SingletonServices<T: Any>(PhantomData<T>);

pub struct ServiceCollection {
    producers: Vec<(TypeId, *const dyn Fn())>,
    dep_checkers: Vec<Box<dyn Fn(&ServiceCollection) -> bool>>
}

impl ServiceCollection {
    pub fn new() -> Self {
        Self {
            producers: Vec::new(),
            dep_checkers: Vec::new()
        }
    }
} 
impl Drop for ServiceCollection {
    fn drop(&mut self) {
        for p in self.producers.iter_mut() {
            unsafe { drop(Box::from_raw(p.1 as *mut dyn Fn())) };
        }
    }
}

impl ServiceCollection {
    pub fn with<T: Resolvable>(&mut self) -> ServiceCollectionWithDependency<'_, T> {
        ServiceCollectionWithDependency(self, PhantomData)
    }

    pub fn register_transient<'s, 'a: 's, T: Any>(&'s mut self, creator: fn() -> T) {
        let func : Box<dyn Fn(&'a ServiceProvider) -> T> = Box::new(move |_: &'a ServiceProvider| {
            creator()
        });
        
        self.producers.push((
            TypeId::of::<Transient<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }

    pub fn register_singleton<'s, 'a: 's, T: Any + Sync>(&'s mut self, creator: fn() -> T) {
        let cell = once_cell::sync::OnceCell::new();
        
        let func : Box<dyn Fn(&'a ServiceProvider) -> &'a T> = Box::new(move |_: &'a ServiceProvider| { 
            unsafe { 
                // Deref is valid because provider never alters any producers
                // Unless destroying itself
                &*(cell.get_or_init(|| {
                    creator()
                }) as *const T) as &'a T
            }
        });
        
        self.producers.push((
            TypeId::of::<Singleton<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }
    pub fn build(mut self) -> Result<ServiceProvider, BuildError> {
        self.producers.sort_by_key(|(id,_)| *id);
        let mut producers = Vec::new();
        let mut dep_checkers = Vec::new();

        core::mem::swap(&mut self.dep_checkers, &mut dep_checkers);
        for checker in dep_checkers.iter() {
            if !(checker)(&mut self) {
                return Err(BuildError::MissingDependency);
            }
        }

        core::mem::swap(&mut self.producers, &mut producers);
        Ok (ServiceProvider { producers })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BuildError {
    MissingDependency
}

pub struct ServiceCollectionWithDependency<'col, T: Resolvable>(&'col mut ServiceCollection, PhantomData<T>);
impl<'col, TDep: Resolvable> ServiceCollectionWithDependency<'col, TDep> {
    pub fn register_transient<'s, 'a: 's, T: Any>(&'s mut self, creator: fn(<TDep::ItemPreChecked as FamilyLt<'a>>::Out) -> T) {
        TDep::add_resolvable_checker(&mut self.0);
        
        let func : Box<dyn Fn(&'a ServiceProvider) -> T> = Box::new(move |container: &'a ServiceProvider| {
            let arg = TDep::resolve_prechecked(container);
            creator(arg)
        });
        
        self.0.producers.push((
            TypeId::of::<Transient<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }
    pub fn register_singleton<'s, 'a: 's, T: Any + Sync>(&'s mut self, creator: fn(<TDep::ItemPreChecked as FamilyLt<'a>>::Out) -> T) {
        let cell = once_cell::sync::OnceCell::new();
        TDep::add_resolvable_checker(&mut self.0);
        
        let func : Box<dyn Fn(&'a ServiceProvider) -> &T> = Box::new(move |container: &'a ServiceProvider| { 
            unsafe { 
                // Deref is valid because provider never alters any producers
                // Unless destroying itself
                &*(cell.get_or_init(|| {
                    let arg = TDep::resolve_prechecked(container);
                    creator(arg)
                }) as *const T)
            }
        });
        
        self.0.producers.push((
            TypeId::of::<Singleton<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }
}
pub struct ServiceProvider {
    /// Mustn't be changed because `resolve_unchecked` relies on it.
    producers: Vec<(TypeId, *const dyn Fn())>
}

impl Drop for ServiceProvider {
    fn drop(&mut self) {
        for p in self.producers.iter_mut() {
            unsafe { drop(Box::from_raw(p.1 as *mut dyn Fn())) };
        }
    }
}

impl ServiceProvider {
    pub fn get<'s, T: Resolvable>(&'s self) -> <T::Item as FamilyLt<'s>>::Out {
        T::resolve(self)
    }
}

#[cfg(test)]
mod tests {
    use {super::* };
    
    #[test]
    fn resolve_last_transient() {
        let mut col = ServiceCollection::new();
        col.register_transient(|| 0);
        col.register_transient(|| 5);
        col.register_transient(|| 1);
        col.register_transient(|| 2);
        let provider = col.build().expect("Expected to have all dependencies");
        let nr = provider.get::<Transient::<i32>>().unwrap();
        assert_eq!(2, nr);
    }    

    #[test]
    fn build_with_missing_transient_dep_fails() {
        build_with_missing_dependency_fails::<Transient<String>>();
    }
    #[test]
    fn build_with_missing_singleton_dep_fails() {
        build_with_missing_dependency_fails::<Singleton<String>>();
    }
    #[test]
    fn build_with_missing_tuple2_dep_fails() {
        build_with_missing_dependency_fails::<(Transient<String>, Transient<i32>)>();
    }
    #[test]
    fn build_with_missing_tuple3_dep_fails() {
        build_with_missing_dependency_fails::<(Transient<String>, Transient<i32>, Transient<i32>)>();
    }
    #[test]
    fn build_with_missing_tuple4_dep_fails() {
        build_with_missing_dependency_fails::<(Transient<i32>, Transient<String>, Transient<i32>, Transient<i32>)>();
    }

    fn build_with_missing_dependency_fails<T: Resolvable>() {
        fn check(mut col: ServiceCollection) {
            col.register_transient(|| 1);
            match col.build() {
                Ok(_) => panic!("Build with missing dependency should fail"),
                Err(e) => assert!(e == BuildError::MissingDependency)
            }
        }
        let mut col = ServiceCollection::new();
        col.with::<T>().register_transient(|_| ());
        check(col);

        let mut col = ServiceCollection::new();
        col.with::<T>().register_singleton(|_| ());
        check(col);        
    }

    #[test]
    fn resolve_last_singleton() {
        let mut container = ServiceCollection::new();
        container.register_singleton(|| 0);
        container.register_singleton(|| 1);
        container.register_singleton(|| 2);
        let provider = container.build().expect("Expected to have all dependencies");
        let nr_ref = provider.get::<Singleton::<i32>>().unwrap();
        assert_eq!(2, *nr_ref);
    }

    #[test]
    fn resolve_transient_services() {
        let mut container = ServiceCollection::new();
        container.register_transient(|| 0);
        container.register_transient(|| 5);
        container.register_transient(|| 2);
        let provider = container.build().expect("Expected to have all dependencies");

        // Count
        let mut count_subset = provider.get::<TransientServices<i32>>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get::<TransientServices::<i32>>().count());

        // Last
        assert_eq!(2, provider.get::<TransientServices<i32>>().last().unwrap());
        
        let mut sub = provider.get::<TransientServices::<i32>>();
        sub.next();
        assert_eq!(Some(2), sub.last());

        let mut consumed = provider.get::<TransientServices::<i32>>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());
        
        let mut iter = provider.get::<TransientServices::<i32>>();
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());     
    }
    #[test]
    fn resolve_singleton_services() {
        let mut container = ServiceCollection::new();
        container.register_singleton(|| 0);
        container.register_singleton(|| 5);
        container.register_singleton(|| 2);
        let provider = container.build().expect("Expected to have all dependencies");

        // Count
        let mut count_subset = provider.get::<SingletonServices<i32>>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get::<SingletonServices::<i32>>().count());

        // Last
        assert_eq!(&2, provider.get::<SingletonServices<i32>>().last().unwrap());
        
        let mut sub = provider.get::<SingletonServices::<i32>>();
        sub.next();
        assert_eq!(Some(&2), sub.last());

        let mut consumed = provider.get::<SingletonServices::<i32>>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());
        
        let mut iter = provider.get::<SingletonServices::<i32>>();
        assert_eq!(Some(&0), iter.next());
        assert_eq!(Some(&5), iter.next());
        assert_eq!(Some(&2), iter.next());
        assert_eq!(None, iter.next());     
    }
    /*
    fn resolve_services<TInner, T>(m: for<'a> fn(<TInner::ItemPreChecked as FamilyLt<'a>>::Out) -> i32) 
        where TInner: Resolvable, 
            T: Resolvable<Item=ServiceIteratorFamily<TInner>> 
    { */
    

    #[test]
    fn resolve_test() {
        let mut container = ServiceCollection::new();
        container.register_transient(|| 42);
        container.register_singleton(|| 42);
        let provider = container.build().expect("Expected to have all dependencies");
        assert_eq!(
            provider.get::<Transient::<i32>>().unwrap(), 
            provider.get::<Singleton::<i32>>().map(|f| *f).unwrap()
        );
    }

    #[test]
    fn get_registered_dynamic_id() {
        let mut container = ServiceCollection::new();
        container.register_transient(|| 42);
        assert_eq!(
            Some(42i32), 
            container.build()
                .expect("Expected to have all dependencies")
                .get::<Transient<i32>>()
        );
    }
    #[test]
    fn get_registered_dynamic_ref() {
        let mut container = ServiceCollection::new();
        container.register_singleton(|| 42);
        assert_eq!(
            Some(&42i32), 
            container.build()
                .expect("Expected to have all dependencies")
                .get::<Singleton<i32>>());
    }

    #[test]
    fn tuple_dependency_resolves_to_prechecked_type() {
        let mut container = ServiceCollection::new();
        container.register_transient(|| 64i64);
        container.with::<(Transient<i64>, Transient<i64>)>().register_singleton(|(a, b)| {
            assert_eq!(64, a);
            assert_eq!(64, b);
            42
        });
        assert_eq!(Some(&42i32), container.build().expect("Expected to have all dependencies").get::<Singleton<i32>>());
    }

    #[test]
    fn get_unkown_returns_none() {
        let container = ServiceCollection::new();
        assert_eq!(
            None, 
            container.build()
                .expect("Expected to have all dependencies")
                .get::<Transient<i32>>()
        );
    }

    #[test]
    fn resolve_tuple_2() {
        let mut container = ServiceCollection::new();
        container.register_transient(|| 32i32);
        container.register_singleton(|| 64i64);
        assert_eq!(
            (Some(32), Some(&64)), 
            container.build()
                .expect("Expected to have all dependencies")
                .get::<(Transient<i32>, Singleton<i64>)>()
        );
    }

    #[test]
    fn register_struct_as_dynamic() {
        let mut container = ServiceCollection::new();   
        container.register_singleton(|| 42i32);
        container.with::<Singleton<i32>>().register_singleton(|i| ServiceImpl(i));
        container.with::<Singleton<ServiceImpl>>().register_transient(|c| c as &dyn Service);
        let provider = container.build().expect("Expected to have all dependencies");
        let service = provider.get::<Transient<&dyn Service>>()
            .expect("Expected to get a service");
       
        assert_eq!(42, service.get_value());
    }

    trait Service {
        fn get_value(&self) -> i32;
    }

    struct ServiceImpl<'a>(&'a i32);
    impl<'a> Service for ServiceImpl<'a> {
        fn get_value(&self) -> i32 {
            println!("Before getting");
            *self.0
        }
    }

    #[test]
    fn drop_singletons_after_provider_drop() {
        let mut col = ServiceCollection::new();
        col.register_singleton(|| Test);
        let prov = col.build().unwrap();
        drop(prov);
        assert_eq!(0, unsafe { DROP_COUNT });
        
        let mut col = ServiceCollection::new();
        col.register_singleton(|| Test);
        let prov = col.build().expect("Expected to have all dependencies");
        prov.get::<Singleton<Test>>().expect("Expected to receive the service");
        drop(prov);
        assert_eq!(1, unsafe { DROP_COUNT });
    }

    static mut DROP_COUNT: u8 = 0;
    struct Test;
    impl Drop for Test {
        fn drop(&mut self) {
            unsafe { DROP_COUNT += 1 };
        }
    }
}