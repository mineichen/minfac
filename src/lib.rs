
//! # IOC framework inspired by .Net's Microsoft.Extensions.DependencyInjection
//! 
//! Complete example:
//! ```
//! use {
//!     ioc_rs::{ServiceCollection, ServiceProvider, Dynamic, DynamicServices},
//!     std::sync::Arc
//! };
//! let mut collection = ioc_rs::ServiceCollection::new();
//! 
//! collection
//!     .with::<Dynamic<i16>>()
//!     .register_transient(|i| i as i32 * 2);
//! collection
//!     .with::<(ServiceProvider, DynamicServices<Arc<i8>>, Dynamic<i32>)>()
//!     .register_transient(|(provider, bytes, int)| {
//!         provider.get::<Dynamic<i16>>().map(|i| i as i64).unwrap_or(1000) // Optional Dependency, fallback not used
//!         + provider.get::<Dynamic<i128>>().map(|i| i as i64).unwrap_or(2000) // Optional Dependency, fallback
//!         + bytes.map(|i| { *i as i64 }).sum::<i64>()
//!         + int as i64 });
//! collection.register_shared(|| Arc::new(1i8));
//! collection.register_shared(|| Arc::new(2i8));
//! collection.register_shared(|| Arc::new(3i8));
//! collection.register_transient(|| 4i16);
//!
//! let provider = collection.build().expect("All dependencies are resolvable");
//! assert_eq!(Some(Arc::new(3)), provider.get::<Dynamic<Arc<i8>>>()); // Last registered i8
//! assert_eq!(Some(4+2000+(1+2+3)+(2*4)), provider.get::<Dynamic<i64>>()); // composed i64
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
    std::sync::Arc,
    resolvable::Resolvable
};

mod resolvable;
mod binary_search;
mod service_provider_factory;


/// Represents instances of a type `T` within a `ServiceProvider`
pub struct ServiceIterator<T> {
    next_pos: Option<usize>,
    provider: ServiceProvider, 
    item_type: PhantomData<T>
}

/// Represents a query for the last registered instance of `T` by value. 
pub struct Dynamic<T: Any>(PhantomData<T>);

/// Represents a Query for all registered instances of Type `T`. Each of those is given by value.
pub struct DynamicServices<T: Any>(PhantomData<T>);

/// Collection of constructors for different types of services. Registered constructors are never called in this state.
/// Instances can only be received by a ServiceProvider, which can be created by calling `build`
pub struct ServiceCollection {
    producers: Vec<(TypeId, *const dyn Fn())>,
    // producers2: Vec<(TypeId, *const dyn Producer<Result=()>)>,
    dep_checkers: Vec<Box<dyn Fn(&Vec<(TypeId, *const dyn Fn())>) -> Option<BuildError>>>
}

impl ServiceCollection {
    /// Creates an empty ServiceCollection
    pub fn new() -> Self {
        Self {
            producers: Vec::new(),
            // producers2: Vec::new(),
            dep_checkers: Vec::new()
        }
    }
} 
impl Drop for ServiceCollection {
    fn drop(&mut self) {
        for p in self.producers.iter_mut() {
            unsafe { core::ptr::drop_in_place(p.1 as *mut dyn Fn()) };
        }
    }
}

impl ServiceCollection {
    /// Generate a ServiceBuilder with `T` as a dependency.
    pub fn with<T: Resolvable>(&mut self) -> ServiceBuilder<'_, T> {
        ServiceBuilder(self, PhantomData)
    }

    /// Registers a transient service without dependencies. 
    /// To add dependencies, use `with` to generate a ServiceBuilder.
    pub fn register_transient<'s, 'a: 's, T: Any>(&'s mut self, creator: fn() -> T) {
        let func : Box<dyn Fn(&'a ServiceProvider) -> T> = Box::new(move |_: &'a ServiceProvider| {
            creator()
        });
        
        self.producers.push((
            TypeId::of::<Dynamic<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }
    /// Registers a shared service without dependencies. 
    /// To add dependencies, use `with` to generate a ServiceBuilder.
    pub fn register_shared<'s, 'a: 's, T: Any + ?Sized>(&'s mut self, creator: fn() -> Arc<T>) {
        let cell = once_cell::sync::OnceCell::new();
       
        let func : Box<dyn Fn(&'a ServiceProvider) -> Arc<T>> = Box::new(move |_container: &'a ServiceProvider| { 
            cell.get_or_init(|| {
                creator()
            }).clone()  
        });
        
        self.producers.push((
            TypeId::of::<Dynamic<Arc<T>>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }

    /// Checks, if all dependencies of registered services are available.
    /// If no errors occured, Ok(ServiceProvider) is returned.
    pub fn build(mut self) -> Result<ServiceProvider, BuildError> {
        let mut producers = self.extract_ordered_producers();

        if let Some(err) = self.dep_checkers.iter().filter_map(|checker| (checker)(&mut producers)).next() {
            return Err(err)
        }

        Ok (ServiceProvider { producers: Arc::new(producers), is_root: true, root: Arc::new(()) })
    }

    pub fn build_factory<T: Clone + Any>(self) -> Result<service_provider_factory::ServiceProviderFactory<T>, BuildError> {
        service_provider_factory::ServiceProviderFactory::create(self)
    }
 
    fn extract_ordered_producers(&mut self) -> Vec<(TypeId, *const dyn Fn())> {
        self.producers.sort_by_key(|(id,_)| *id);
        let mut producers = Vec::new();
        core::mem::swap(&mut self.producers, &mut producers);
        producers
    }
}

/// Possible errors when calling ServiceCollection::build()
#[derive(Debug, PartialEq, Eq)]
pub enum BuildError {
    MissingDependency(MissingDependencyInfos),
    CyclicDependency(String)
}

#[derive(Debug, PartialEq, Eq)]
pub struct MissingDependencyInfos {
    typeid: TypeId,
    typename: &'static str
}

pub struct ServiceBuilder<'col, T: Resolvable>(&'col mut ServiceCollection, PhantomData<T>);
impl<'col, TDep: Resolvable> ServiceBuilder<'col, TDep> {
    pub fn register_transient<'s, 'a: 's, T: Any>(&'s mut self, creator: fn(TDep::ItemPreChecked) -> T) {
        TDep::add_resolvable_checker(&mut self.0);
        
        let func : Box<dyn Fn(&'a ServiceProvider) -> T> = Box::new(move |container: &'a ServiceProvider| {
            let arg = TDep::resolve_prechecked(container);
            creator(arg)
        });
        
        self.0.producers.push((
            TypeId::of::<Dynamic<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }
    pub fn register_shared<'s, 'a: 's, T: Any + ?Sized>(&'s mut self, creator: fn(TDep::ItemPreChecked) -> Arc<T>) {
        let cell = once_cell::sync::OnceCell::new();
        TDep::add_resolvable_checker(&mut self.0);
        let func : Box<dyn Fn(&'a ServiceProvider) -> Arc<T>> = Box::new(move |container: &'a ServiceProvider| { 
            cell.get_or_init(|| {
                let arg = TDep::resolve_prechecked(container);
                creator(arg)
            }).clone()   
        });
        
        self.0.producers.push((
            TypeId::of::<Dynamic<Arc<T>>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }
}

pub struct ServiceProvider {
    
    root: Arc<()>,
    /// Mustn't be changed because `resolve_unchecked` relies on it.
    producers: Arc<Vec<(TypeId, *const dyn Fn())>>,
    is_root: bool
}

impl Drop for ServiceProvider {
    fn drop(&mut self) {
        if self.is_root {
            for p in self.producers.iter() {
                unsafe { drop(Box::from_raw(p.1 as *mut dyn Fn())) };
            }
        }
    }
}

impl ServiceProvider {
    pub fn get<'s, T: Resolvable>(&'s self) -> T::Item {
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
        let nr = provider.get::<Dynamic::<i32>>().unwrap();
        assert_eq!(2, nr);
    }   

    #[test]
    fn resolve_shared() {
        let mut col = ServiceCollection::new();
        col.register_shared(|| Arc::new(std::cell::RefCell::new(1)));
        col.with::<ServiceProvider>().register_shared(|_| Arc::new(std::cell::RefCell::new(2)));

        let prov = col.build().expect("Should have all Dependencies");
        let second = prov.get::<Dynamic<Arc<std::cell::RefCell<i32>>>>().expect("Expecte to get second");
        assert_eq!(2, *second.borrow());
        second.replace(42);

        assert_eq!(
            prov.get::<DynamicServices<Arc<std::cell::RefCell<i32>>>>()
                .map(|c| *c.borrow())
                .sum::<i32>(),
            1 + 42
        );
    }

    #[test]
    fn build_with_missing_transient_dep_fails() {
        build_with_missing_dependency_fails::<Dynamic<String>>(&["Dynamic", "String"]);
    }
    
    #[test]
    fn build_with_missing_tuple2_dep_fails() {
        build_with_missing_dependency_fails::<(Dynamic<String>, Dynamic<i32>)>(&["Dynamic", "String"]);
    }
    #[test]
    fn build_with_missing_tuple3_dep_fails() {
        build_with_missing_dependency_fails::<(Dynamic<String>, Dynamic<i32>, Dynamic<i32>)>(&["Dynamic", "String"]);
    }
    #[test]
    fn build_with_missing_tuple4_dep_fails() {
        build_with_missing_dependency_fails::<(Dynamic<i32>, Dynamic<String>, Dynamic<i32>, Dynamic<i32>)>(&["Dynamic", "String"]);
    }

    fn build_with_missing_dependency_fails<T: Resolvable>(missing_msg_parts: &[&str]) {
        fn check(mut col: ServiceCollection, missing_msg_parts: &[&str]) {
            col.register_transient(|| 1);
            match col.build() {
                Ok(_) => panic!("Build with missing dependency should fail"),
                Err(e) => match e {
                    BuildError::MissingDependency(msg) => {
                        for part in missing_msg_parts {
                            assert!(msg.typename.contains(part), "Expected '{}' to contain '{}'", msg.typename, part);
                        }
                    },
                    _ => panic!("Unexpected Error")
                }
            }
        }
        let mut col = ServiceCollection::new();
        col.with::<T>().register_transient(|_| ());
        check(col, missing_msg_parts);

        let mut col = ServiceCollection::new();
        col.with::<T>().register_shared(|_| Arc::new(()));
        check(col, missing_msg_parts);        
    }

    #[test]
    fn resolve_last_shared() {
        let mut container = ServiceCollection::new();
        container.register_shared(|| Arc::new(0));
        container.register_shared(|| Arc::new(1));
        container.register_shared(|| Arc::new(2));
        let provider = container.build().expect("Expected to have all dependencies");
        let nr_ref = provider.get::<Dynamic::<Arc<i32>>>().unwrap();
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
        let mut count_subset = provider.get::<DynamicServices<i32>>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get::<DynamicServices::<i32>>().count());

        // Last
        assert_eq!(2, provider.get::<DynamicServices<i32>>().last().unwrap());
        
        let mut sub = provider.get::<DynamicServices::<i32>>();
        sub.next();
        assert_eq!(Some(2), sub.last());

        let mut consumed = provider.get::<DynamicServices::<i32>>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());
        
        let mut iter = provider.get::<DynamicServices::<i32>>();
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());     
    }
    #[test]
    fn resolve_shared_services() {
        let mut container = ServiceCollection::new();
        container.register_shared(|| Arc::new(0));
        container.register_shared(|| Arc::new(5));
        container.register_shared(|| Arc::new(2));
        let provider = container.build().expect("Expected to have all dependencies");

        // Count
        let mut count_subset = provider.get::<DynamicServices<Arc<i32>>>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get::<DynamicServices::<Arc<i32>>>().count());

        // Last
        assert_eq!(2, *provider.get::<DynamicServices<Arc<i32>>>().last().unwrap());
        
        let mut sub = provider.get::<DynamicServices::<Arc<i32>>>();
        sub.next();
        assert_eq!(Some(2), sub.last().map(|i| *i));

        let mut consumed = provider.get::<DynamicServices::<Arc<i32>>>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());
        
        let mut iter = provider.get::<DynamicServices::<Arc<i32>>>().map(|i| *i);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());     
    }    

    #[test]
    fn resolve_test() {
        let mut container = ServiceCollection::new();
        container.register_transient(|| 42);
        container.register_shared(|| Arc::new(42));
        let provider = container.build().expect("Expected to have all dependencies");
        assert_eq!(
            provider.get::<Dynamic::<i32>>().unwrap(), 
            provider.get::<Dynamic::<Arc<i32>>>().map(|f| *f).unwrap()
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
                .get::<Dynamic<i32>>()
        );
    }
    #[test]
    fn get_registered_dynamic_ref() {
        let mut container = ServiceCollection::new();
        container.register_shared(|| Arc::new(42));
        assert_eq!(
            Some(42i32), 
            container.build()
                .expect("Expected to have all dependencies")
                .get::<Dynamic<Arc<i32>>>().map(|i| *i)
        );
    }

    #[test]
    fn tuple_dependency_resolves_to_prechecked_type() {
        let mut container = ServiceCollection::new();
        container.register_transient(|| 64i64);
        container.with::<(Dynamic<i64>, Dynamic<i64>)>().register_shared(|(a, b)| {
            assert_eq!(64, a);
            assert_eq!(64, b);
            Arc::new(42)
        });
        assert_eq!(
            Some(42i32), 
            container.build()
                .expect("Expected to have all dependencies")
                .get::<Dynamic<Arc<i32>>>()
                .map(|i| *i)
        );
    }

    #[test]
    fn get_unkown_returns_none() {
        let container = ServiceCollection::new();
        assert_eq!(
            None, 
            container.build()
                .expect("Expected to have all dependencies")
                .get::<Dynamic<i32>>()
        );
    }

    #[test]
    fn resolve_tuple_2() {
        let mut container = ServiceCollection::new();
        container.register_transient(|| 32i32);
        container.register_shared(|| Arc::new(64i64));
        let (a, b) = container.build()
            .expect("Expected to have all dependencies")
            .get::<(Dynamic<i32>, Dynamic<Arc<i64>>)>();
        assert_eq!(Some(32), a);
        assert_eq!(Some(64), b.map(|i| *i));
    }

    #[test]
    fn test_size() {
        fn new_dyn() -> Arc<dyn Service> {
            Arc::new(ServiceImpl(Arc::new(1i32))) as Arc<dyn Service>
        }
        assert_eq!(1, new_dyn().get_value());
    }

    #[test]
    fn register_struct_as_dynamic() {
        let mut container = ServiceCollection::new();   
        container.register_shared(|| Arc::new(42i32));
        container.with::<Dynamic<Arc<i32>>>().register_shared(|i| Arc::new(ServiceImpl(i)) as Arc<dyn Service>);
        let provider = container.build().expect("Expected to have all dependencies");
        let service = provider.get::<Dynamic<Arc<dyn Service>>>()
            .expect("Expected to get a service");
       
        assert_eq!(42, service.get_value());
    }

    trait Service {
        fn get_value(&self) -> i32;
    }

    struct ServiceImpl<T: core::ops::Deref<Target=i32>>(T);
    impl<T: core::ops::Deref<Target=i32>> Service for ServiceImpl<T> {
        fn get_value(&self) -> i32 {
            *self.0
        }
    }

    #[test]
    fn drop_shareds_after_provider_drop() {
        let mut col = ServiceCollection::new();
        col.register_shared(|| Arc::new(Test));
        let prov = col.build().unwrap();
        drop(prov);
        assert_eq!(0, unsafe { DROP_COUNT });
        
        let mut col = ServiceCollection::new();
        col.register_shared(|| Arc::new(Test));
        let prov = col.build().expect("Expected to have all dependencies");
        prov.get::<Dynamic<Arc<Test>>>().expect("Expected to receive the service");
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