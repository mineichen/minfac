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
//!     .register(|i| i as i32 * 2);
//! collection
//!     .with::<(ServiceProvider, DynamicServices<Arc<i8>>, Dynamic<i32>)>()
//!     .register(|(provider, bytes, int)| {
//!         provider.get::<Dynamic<i16>>().map(|i| i as i64).unwrap_or(1000) // Optional Dependency, fallback not used
//!         + provider.get::<Dynamic<i128>>().map(|i| i as i64).unwrap_or(2000) // Optional Dependency, fallback
//!         + bytes.map(|i| { *i as i64 }).sum::<i64>()
//!         + int as i64 });
//! collection.register_arc(|| Arc::new(1i8));
//! collection.register_arc(|| Arc::new(2i8));
//! collection.register_arc(|| Arc::new(3i8));
//! collection.register(|| 4i16);
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
        any::{Any, TypeId},
        marker::PhantomData,
        fmt::Debug,
    },
    once_cell::sync::OnceCell,
    resolvable::Resolvable,
    service_provider_factory::{ServiceProviderFactory, ServiceProviderFactoryBuilder},
    untyped::{UntypedFn, UntypedPointer},
    std::sync::Arc,
};

mod binary_search;
mod resolvable;
mod service_provider_factory;
mod untyped;

/// Represents instances of a type `T` within a `ServiceProvider`
pub struct ServiceIterator<T> {
    ///
    next_pos: Option<usize>,
    provider: ServiceProvider,
    item_type: PhantomData<T>,
}

/// Represents a query for the last registered instance of `T` by value.
pub struct Dynamic<T: Any>(PhantomData<T>);

/// Represents a Query for all registered instances of Type `T`. Each of those is given by value.
pub struct DynamicServices<T: Any>(PhantomData<T>);

/// Collection of constructors for different types of services. Registered constructors are never called in this state.
/// Instances can only be received by a ServiceProvider, which can be created by calling `build`
pub struct ServiceCollection {
    producer_factories: Vec<ServiceProducer>
}

struct ServiceProducer {
    type_id: TypeId,
    factory: UntypedFnFactory,
}

impl ServiceProducer {
    fn new<T: Any>(factory: UntypedFnFactory) -> Self {
        Self::new_with_type(factory, TypeId::of::<Dynamic<T>>())
    }
    fn new_with_type(factory: UntypedFnFactory, type_id: TypeId) -> Self {
        Self {
            factory,
            type_id
        }
    }
}
// type CycleChecker = fn() -> Option<BuildError>;
type DependencyChecker = fn(&Vec<ServiceProducer>) -> Option<BuildError>;
type UntypedFnFactory = Box<dyn for<'a> FnOnce(&mut UntypedFnFactoryContext<'a>) -> Result<(UntypedFn, Option<DependencyChecker>),BuildError>>;

struct UntypedFnFactoryContext<'a> {
    service_state_counter: usize,
    service_types: &'a Vec<TypeId>
}

impl<'a> UntypedFnFactoryContext<'a> {
    fn increment(&mut self) -> usize {
        let result: usize = self.service_state_counter;
        self.service_state_counter += 1;
        result
    }
}

impl ServiceCollection {
    /// Creates an empty ServiceCollection
    pub fn new() -> Self {
        Self {
            producer_factories: Vec::new()
        }
    }
}

impl Default for ServiceCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceCollection {
    /// Generate a ServiceBuilder with `T` as a dependency.
    pub fn with<T: Resolvable>(&mut self) -> ServiceBuilder<'_, T> {
        ServiceBuilder(self, PhantomData)
    }

    /// Registers a transient service without dependencies.
    /// To add dependencies, use `with` to generate a ServiceBuilder.
    pub fn register<T: Any>(&mut self, creator: fn() -> T) {
        let factory: UntypedFnFactory = Box::new(move |_service_state_counter| {
            let func: Box<dyn Fn(&ServiceProvider) -> T> =
                Box::new(move |_: &ServiceProvider| creator());
            Ok((func.into(), None))
        });
        self.producer_factories.push(ServiceProducer::new::<T>(factory));
    }

    /// Registers a shared service without dependencies.
    /// To add dependencies, use `with` to generate a ServiceBuilder.
    ///
    /// Shared services must have a reference count == 0 after dropping the ServiceProvider. If an Arc is
    /// cloned and thus kept alive, ServiceProvider::drop will panic to prevent memory leaks.
    pub fn register_arc<T: Any + ?Sized>(&mut self, creator: fn() -> Arc<T>) {
        let factory: UntypedFnFactory = Box::new(move |ctx| {
            let service_state_idx = ctx.increment();

            let func: Box<dyn Fn(&ServiceProvider) -> Arc<T>> =
                Box::new(move |provider: &ServiceProvider| {
                    provider.get_or_initialize_pos(service_state_idx, creator)
                });
            Ok((func.into(), None))
        });
        self.producer_factories.push(ServiceProducer::new::<Arc<T>>(factory));
    }

    /// Checks, if all dependencies of registered services are available.
    /// If no errors occured, Ok(ServiceProvider) is returned.
    pub fn build(self) -> Result<ServiceProvider, BuildError> {
        let (producers, service_states_count) = self.validate_producers(Vec::new(), 0)?;
        let service_states = vec![OnceCell::new(); service_states_count];
        let immutable_state = Arc::new(ServiceProviderImmutableState {
            producers,
            _parents: Vec::new()
        });
        Ok(ServiceProvider {
            immutable_state,
            service_states: Arc::new(service_states),
        })
    }

    ///
    /// Returns a factory which can efficiently create ServiceProviders from
    /// ServiceCollections which are missing one dependent service T (e.g. HttpRequest, StartupConfiguration)
    /// The missing service must implement `Any` + `Clone`.
    ///
    /// Unlike shared services, this service's reference counter isn't checked to equal zero when the provider is dropped
    ///
    pub fn build_factory<T: Clone + Any>(
        self,
    ) -> Result<ServiceProviderFactory<T>, BuildError> {
        ServiceProviderFactory::create(self, Vec::new())
    }

    pub fn with_parent(self, provider: ServiceProvider) -> ServiceProviderFactoryBuilder {
        ServiceProviderFactoryBuilder::create(self, provider)
    }

    fn validate_producers(
        self, 
        mut factories: Vec<ServiceProducer>,
        service_state_counter: usize
    ) -> Result<(Vec<UntypedFn>, usize), BuildError> {
        
        factories.extend(self
            .producer_factories
            .into_iter());
            
        factories.sort_by_key(|a| a.type_id);
       
        // Todo: Avoid additional iteration here
        let mut types : Vec<TypeId> = factories.iter()
            .map(|f| f.type_id)
            .collect();
            
        let mut ctx = UntypedFnFactoryContext { service_state_counter, service_types: &mut types };
        let mut producers = Vec::with_capacity(factories.len());
        for x in  factories.into_iter() {            
            let (producer, _) = (x.factory)(&mut ctx)?;
            debug_assert_eq!(&x.type_id, producer.get_result_type_id());
            producers.push(producer);
        }
        Ok((producers, ctx.service_state_counter))
    }
}

/// Possible errors when calling ServiceCollection::build() or ServiceCollection::build_factory()
#[derive(Debug, PartialEq, Eq)]
pub enum BuildError {
    MissingDependency(MissingDependencyType),
    CyclicDependency(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct MissingDependencyType {
    id: TypeId,
    name: &'static str,
}

impl MissingDependencyType {
    fn new<T: Any>() -> Self {
        Self {
            name: std::any::type_name::<T>(),
            id: std::any::TypeId::of::<T>(),
        }
    }
}

pub struct ServiceBuilder<'col, T: Resolvable>(&'col mut ServiceCollection, PhantomData<T>);
impl<'col, TDep: Resolvable> ServiceBuilder<'col, TDep> {
    pub fn register<T: Any>(&mut self, creator: fn(TDep::ItemPreChecked) -> T) {
        let factory: UntypedFnFactory = Box::new(move |ctx| {
            let key = TDep::precheck(ctx.service_types)?;
            let func: Box<dyn Fn(&ServiceProvider) -> T> =
                Box::new(move |container: &ServiceProvider| {
                    let arg = TDep::resolve_prechecked(container, &key);
                    creator(arg)
                });
            Ok((func.into(), Some(|_| { None })))
        });
        self.0.producer_factories.push(ServiceProducer::new::<T>(factory));
    }
    pub fn register_arc<T: Any + ?Sized>(&mut self, creator: fn(TDep::ItemPreChecked) -> Arc<T>) {
        let factory: UntypedFnFactory = Box::new(move |ctx| {
            let service_state_idx = ctx.increment();
            let key = TDep::precheck(ctx.service_types)?;
            let func: Box<dyn Fn(&ServiceProvider) -> Arc<T>> =
                Box::new(move |provider: &ServiceProvider| {
                    let moved_key = &key;
                    provider.get_or_initialize_pos(service_state_idx, move || {
                        creator(TDep::resolve_prechecked(provider, &moved_key))
                    })
                });
            Ok((func.into(), None))
        });
        self.0.producer_factories.push(ServiceProducer::new::<Arc<T>>(factory));
    }
}

pub struct ServiceProvider {
    immutable_state: Arc<ServiceProviderImmutableState>,
    service_states: Arc<Vec<OnceCell<UntypedPointer>>>,
}

impl Debug for ServiceProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "ServiceProvider (services: {}, with_state: {})", 
            self.immutable_state.producers.len(),
            self.service_states.len()
        ))
    }
}

impl ServiceProvider {
    pub fn get<T: Resolvable>(&self) -> T::Item {
        T::resolve(self)
    }
    fn get_or_initialize_pos<T: Clone, TFn: Fn() -> T>(&self, index: usize, initializer: TFn) -> T {
        let pointer = self
            .service_states
            .get(index)
            .unwrap()
            .get_or_init(|| UntypedPointer::new(initializer()));

        unsafe { pointer.borrow_as::<T>() }.clone()
    }
}

impl Clone for ServiceProvider {
    fn clone(&self) -> Self {
        Self {
            immutable_state: self.immutable_state.clone(),
            service_states: self.service_states.clone(),
        }
    }
}

struct ServiceProviderImmutableState {
    producers: Vec<UntypedFn>,
    // Unsafe-Code, which generates UntypedFn from parent, relies on the fact that parent ServiceProvider outlives this state
    _parents: Vec<ServiceProvider>
}


#[cfg(test)]
mod tests {

    use super::*;

    
    #[test]
    fn resolve_last_transient() {
        let mut col = ServiceCollection::new();
        col.register(|| 0);
        col.register(|| 5);
        col.register(|| 1);
        col.register(|| 2);
        let provider = col.build().expect("Expected to have all dependencies");
        let nr = provider.get::<Dynamic<i32>>().unwrap();
        assert_eq!(2, nr);
    }

    #[test]
    fn resolve_shared() {
        let mut col = ServiceCollection::new();
        col.register_arc(|| Arc::new(std::cell::RefCell::new(1)));
        col.with::<ServiceProvider>()
            .register_arc(|_| Arc::new(std::cell::RefCell::new(2)));

        let prov = col.build().expect("Should have all Dependencies");
        let second = prov
            .get::<Dynamic<Arc<std::cell::RefCell<i32>>>>()
            .expect("Expecte to get second");
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
        build_with_missing_dependency_fails::<(Dynamic<String>, Dynamic<i32>)>(&[
            "Dynamic", "String",
        ]);
    }
    #[test]
    fn build_with_missing_tuple3_dep_fails() {
        build_with_missing_dependency_fails::<(Dynamic<String>, Dynamic<i32>, Dynamic<i32>)>(&[
            "Dynamic", "String",
        ]);
    }
    #[test]
    fn build_with_missing_tuple4_dep_fails() {
        build_with_missing_dependency_fails::<(
            Dynamic<i32>,
            Dynamic<String>,
            Dynamic<i32>,
            Dynamic<i32>,
        )>(&["Dynamic", "String"]);
    }

    fn build_with_missing_dependency_fails<T: Resolvable>(missing_msg_parts: &[&str]) {
        fn check(mut col: ServiceCollection, missing_msg_parts: &[&str]) {
            col.register(|| 1);
            match col.build() {
                Ok(_) => panic!("Build with missing dependency should fail"),
                Err(e) => match e {
                    BuildError::MissingDependency(msg) => {
                        for part in missing_msg_parts {
                            assert!(
                                msg.name.contains(part),
                                "Expected '{}' to contain '{}'",
                                msg.name,
                                part
                            );
                        }
                    }
                    _ => panic!("Unexpected Error"),
                },
            }
        }
        let mut col = ServiceCollection::new();
        col.with::<T>().register(|_| ());
        check(col, missing_msg_parts);

        let mut col = ServiceCollection::new();
        col.with::<T>().register_arc(|_| Arc::new(()));
        check(col, missing_msg_parts);
    }

    #[test]
    fn resolve_shared_returns_last_registered() {
        let mut container = ServiceCollection::new();
        container.register_arc(|| Arc::new(0));
        container.register_arc(|| Arc::new(1));
        container.register_arc(|| Arc::new(2));
        let provider = container
            .build()
            .expect("Expected to have all dependencies");
        let nr_ref = provider.get::<Dynamic<Arc<i32>>>().unwrap();
        assert_eq!(2, *nr_ref);
    }

    #[test]
    fn resolve_transient_services() {
        let mut container = ServiceCollection::new();
        container.register(|| 0);
        container.register(|| 5);
        container.register(|| 2);
        let provider = container
            .build()
            .expect("Expected to have all dependencies");

        // Count
        let mut count_subset = provider.get::<DynamicServices<i32>>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get::<DynamicServices::<i32>>().count());

        // Last
        assert_eq!(2, provider.get::<DynamicServices<i32>>().last().unwrap());

        let mut sub = provider.get::<DynamicServices<i32>>();
        sub.next();
        assert_eq!(Some(2), sub.last());

        let mut consumed = provider.get::<DynamicServices<i32>>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());

        let mut iter = provider.get::<DynamicServices<i32>>();
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn no_dependency_needed_if_service_depends_on_services_which_are_not_present() {
        let mut container = ServiceCollection::new();
        container.with::<DynamicServices<String>>().register(|_| 0);

        assert!(container.build().is_ok())
    }

    #[test]
    fn resolve_shared_services() {
        let mut container = ServiceCollection::new();
        container.register_arc(|| Arc::new(0));
        container.register_arc(|| Arc::new(5));
        container.register_arc(|| Arc::new(2));
        let provider = container
            .build()
            .expect("Expected to have all dependencies");

        // Count
        let mut count_subset = provider.get::<DynamicServices<Arc<i32>>>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get::<DynamicServices::<Arc<i32>>>().count());

        // Last
        assert_eq!(
            2,
            *provider.get::<DynamicServices<Arc<i32>>>().last().unwrap()
        );

        let mut sub = provider.get::<DynamicServices<Arc<i32>>>();
        sub.next();
        assert_eq!(Some(2), sub.last().map(|i| *i));

        let mut consumed = provider.get::<DynamicServices<Arc<i32>>>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());

        let mut iter = provider.get::<DynamicServices<Arc<i32>>>().map(|i| *i);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn resolve_test() {
        let mut container = ServiceCollection::new();
        container.register(|| 42);
        container.register_arc(|| Arc::new(42));
        let provider = container
            .build()
            .expect("Expected to have all dependencies");
        assert_eq!(
            provider.get::<Dynamic::<i32>>().unwrap(),
            provider.get::<Dynamic::<Arc<i32>>>().map(|f| *f).unwrap()
        );
    }

    #[test]
    fn get_registered_dynamic_id() {
        let mut container = ServiceCollection::new();
        container.register(|| 42);
        assert_eq!(
            Some(42i32),
            container
                .build()
                .expect("Expected to have all dependencies")
                .get::<Dynamic<i32>>()
        );
    }
    #[test]
    fn get_registered_dynamic_ref() {
        let mut container = ServiceCollection::new();
        container.register_arc(|| Arc::new(42));
        assert_eq!(
            Some(42i32),
            container
                .build()
                .expect("Expected to have all dependencies")
                .get::<Dynamic<Arc<i32>>>()
                .map(|i| *i)
        );
    }

    #[test]
    fn tuple_dependency_resolves_to_prechecked_type() {
        let mut container = ServiceCollection::new();
        container.register(|| 64i64);
        container
            .with::<(Dynamic<i64>, Dynamic<i64>)>()
            .register_arc(|(a, b)| {
                assert_eq!(64, a);
                assert_eq!(64, b);
                Arc::new(42)
            });
        assert_eq!(
            Some(42i32),
            container
                .build()
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
            container
                .build()
                .expect("Expected to have all dependencies")
                .get::<Dynamic<i32>>()
        );
    }

    #[test]
    fn resolve_tuple_2() {
        let mut container = ServiceCollection::new();
        container.register(|| 32i32);
        container.register_arc(|| Arc::new(64i64));
        let (a, b) = container
            .build()
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
        container.register_arc(|| Arc::new(42i32));
        container
            .with::<Dynamic<Arc<i32>>>()
            .register_arc(|i| Arc::new(ServiceImpl(i)) as Arc<dyn Service>);
        let provider = container
            .build()
            .expect("Expected to have all dependencies");
        let service = provider
            .get::<Dynamic<Arc<dyn Service>>>()
            .expect("Expected to get a service");

        assert_eq!(42, service.get_value());
    }

    trait Service {
        fn get_value(&self) -> i32;
    }

    struct ServiceImpl<T: core::ops::Deref<Target = i32>>(T);
    impl<T: core::ops::Deref<Target = i32>> Service for ServiceImpl<T> {
        fn get_value(&self) -> i32 {
            *self.0
        }
    }

    #[test]
    fn drop_collection_doesnt_call_any_factories() {
        let mut col = ServiceCollection::new();
        col.register_arc::<std::rc::Rc<()>>(|| { panic!("Should never be called"); });
        let prov = col.build().unwrap();
        drop(prov);
    }
    #[test]
    fn drop_shareds_after_provider_drop() {
        let mut col = ServiceCollection::new();
        col.register_arc(|| Arc::new(std::rc::Rc::new(())));
        let prov = col.build().expect("Expected to have all dependencies");
        let inner = prov.get::<Dynamic<Arc<std::rc::Rc<()>>>>()
            .expect("Expected to receive the service")
            .as_ref()
            .clone();

        assert_eq!(2, std::rc::Rc::strong_count(&inner));
        drop(prov);
        assert_eq!(1, std::rc::Rc::strong_count(&inner));
    }
}
