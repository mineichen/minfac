//! # IOC framework inspired by .Net's Microsoft.Extensions.DependencyInjection
//!
//! Simple example with two services, one of which depends on the other.
//! ```
//! use {ioc_rs::{Registered, ServiceCollection}};
//!
//! let mut collection = ServiceCollection::new();
//! collection
//!     .with::<Registered<u8>>()
//!     .register(|byte| byte as i16 * 2);
//! collection.register(|| 1u8);
//! let provider = collection.build().expect("Configuration is valid");
//!
//! assert_eq!(Some(2), provider.get::<Registered<i16>>());
//! ```
//! # Features
//! - Register Types/Traits which are not part of your crate (e.g. std::*). No macros needed.
//! - Service registration from separately compiled dynamic libraries. see `examples/distributed_simple` for more details
//! - No redundant reference counting. Transient Services are retrieved as `T`, SharedServices as `Arc<T>`
//! - Service discovery, e.g. `service_provider.get::<DynamicServices<i32>>()` returns an iterator over all registered i32
//! - Fail fast. When building a `ServiceProvider` all registered services are checked to
//!   - have all dependencies
//!   - contain no dependency-cycles
//! - Common pitfalls of traditional IOC are prevented by design
//!   - Singleton services cannot reference scoped services (see examples/complete.rs)
//!   - Shared services cannot outlive their `ServiceProvider`
//! - `#[no_std]`
//!
//! Visit the examples/documentation for more details
//!
//! This library requires some amounts of unsafe code. All tests are executed with `cargo miri test`
//! to reduce the chance of having undefined behavior or memory leaks. Audits from experienced developers
//! would be appreciated.

#![no_std]

extern crate alloc;

use {
    alloc::{
        boxed::Box,
        collections::BTreeMap,
        string::{String, ToString},
        sync::Arc,
        vec,
        vec::Vec,
    },
    core::{
        any::{type_name, Any, TypeId},
        fmt::Debug,
        marker::PhantomData,
    },
    once_cell::sync::OnceCell,
    resolvable::Resolvable,
    service_provider_factory::{ServiceProviderFactory, ServiceProviderFactoryBuilder},
    untyped::{UntypedFn, UntypedPointer},
};

mod binary_search;
mod resolvable;
mod service_provider_factory;
mod untyped;

/// Type used to retrieve all instances of `T` of a `ServiceProvider`
pub struct ServiceIterator<T> {
    next_pos: Option<usize>,
    provider: ServiceProvider,
    item_type: PhantomData<T>,
}

/// Represents a query for the last registered instance of `T`
pub struct Registered<T: Any>(PhantomData<T>);

/// Represents a Query for all registered instances of Type `T`.
pub struct AllRegistered<T: Any>(PhantomData<T>);

/// Collection of constructors for different types of services. Registered constructors are never called in this state.
/// Instances can only be received by a ServiceProvider, which can be created by calling `build`
pub struct ServiceCollection {
    producer_factories: Vec<ServiceProducer>,
}

struct ServiceProducer {
    type_id: TypeId,
    factory: UntypedFnFactory,
}

impl ServiceProducer {
    fn new<T: Any>(factory: UntypedFnFactory) -> Self {
        Self::new_with_type(factory, TypeId::of::<Registered<T>>())
    }
    fn new_with_type(factory: UntypedFnFactory, type_id: TypeId) -> Self {
        Self { type_id, factory }
    }
}
// type CycleChecker = fn() -> Option<BuildError>;
type UntypedFnFactory =
    Box<dyn for<'a> FnOnce(&mut UntypedFnFactoryContext<'a>) -> Result<UntypedFn, BuildError>>;

struct UntypedFnFactoryContext<'a> {
    service_descriptor_pos: usize,
    state_counter: &'a mut usize,
    final_ordered_types: &'a Vec<TypeId>,
    cyclic_reference_candidates:
        &'a mut BTreeMap<usize, CycleCheckerValue>,
}

impl<'a> UntypedFnFactoryContext<'a> {
    fn reserve_state_space(&mut self) -> usize {
        let result: usize = *self.state_counter;
        *self.state_counter += 1;
        result
    }
    fn register_cyclic_reference_candidate(
        &mut self,
        type_name: &'static str,
        dependencies: Box<dyn Iterator<Item = usize>>,
    ) {
        self.cyclic_reference_candidates.insert(
            self.service_descriptor_pos,
            CycleCheckerValue { is_visited: false, type_description: type_name, iter: dependencies},
        );
    }
}

impl ServiceCollection {
    /// Creates an empty ServiceCollection
    pub fn new() -> Self {
        Self {
            producer_factories: Vec::new(),
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
            Ok(func.into())
        });
        self.producer_factories
            .push(ServiceProducer::new::<T>(factory));
    }

    /// Registers a shared service without dependencies.
    /// To add dependencies, use `with` to generate a ServiceBuilder.
    ///
    /// Shared services must have a reference count == 0 after dropping the ServiceProvider. If an Arc is
    /// cloned and thus kept alive, ServiceProvider::drop will panic to prevent memory leaks.
    pub fn register_shared<T: Any + ?Sized + Send + Sync>(&mut self, creator: fn() -> Arc<T>) {
        let factory: UntypedFnFactory = Box::new(move |ctx| {
            let service_state_idx = ctx.reserve_state_space();

            let func: Box<dyn Fn(&ServiceProvider) -> Arc<T>> =
                Box::new(move |provider: &ServiceProvider| {
                    provider.get_or_initialize_pos(service_state_idx, creator)
                });
            Ok(func.into())
        });
        self.producer_factories
            .push(ServiceProducer::new::<Arc<T>>(factory));
    }

    /// Checks, if all dependencies of registered services are available.
    /// If no errors occured, Ok(ServiceProvider) is returned.
    pub fn build(self) -> Result<ServiceProvider, BuildError> {
        let mut service_states_count = 0;
        let producers = self.validate_producers(Vec::new(), &mut service_states_count)?;
        let service_states = vec![OnceCell::new(); service_states_count];
        let immutable_state = Arc::new(ServiceProviderImmutableState {
            producers,
            _parents: Vec::new(),
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
    pub fn build_factory<T: Clone + Any>(self) -> Result<ServiceProviderFactory<T>, BuildError> {
        ServiceProviderFactory::create(self, Vec::new())
    }

    pub fn with_parent(self, provider: ServiceProvider) -> ServiceProviderFactoryBuilder {
        ServiceProviderFactoryBuilder::create(self, provider)
    }

    fn validate_producers(
        self,
        mut factories: Vec<ServiceProducer>,
        state_counter: &mut usize,
    ) -> Result<Vec<UntypedFn>, BuildError> {
        factories.extend(self.producer_factories.into_iter());

        factories.sort_by_key(|a| a.type_id);

        let mut final_ordered_types: Vec<TypeId> = factories.iter().map(|f| f.type_id).collect();

        let mut cyclic_reference_candidates = BTreeMap::new();
        let mut producers = Vec::with_capacity(factories.len());

        for (i, x) in factories.into_iter().enumerate() {
            let mut ctx = UntypedFnFactoryContext {
                state_counter,
                final_ordered_types: &mut final_ordered_types,
                cyclic_reference_candidates: &mut cyclic_reference_candidates,
                service_descriptor_pos: i,
            };
            let producer = (x.factory)(&mut ctx)?;
            debug_assert_eq!(&x.type_id, producer.get_result_type_id());
            producers.push(producer);
        }

        CycleChecker(&mut cyclic_reference_candidates)
            .ok()
            .map_err(|indices| {
                BuildError::CyclicDependency(
                    indices
                        .into_iter()
                        .skip(1)
                        .map(|i| cyclic_reference_candidates.get(&i).unwrap().type_description)
                        .fold(
                            cyclic_reference_candidates
                                .values()
                                .next()
                                .unwrap()
                                .type_description
                                .to_string(),
                            |acc, n| acc + " -> " + n,
                        ),
                )
            })?;

        Ok(producers)
    }
}

struct CycleCheckerValue {
    is_visited: bool,
    type_description: &'static str,
    iter: Box<dyn Iterator<Item = usize>>
}
struct CycleChecker<'a>(
    &'a mut BTreeMap<usize, CycleCheckerValue>,
);

impl<'a> CycleChecker<'a> {
    fn ok(self) -> Result<(), Vec<usize>> {
        let mut stack = Vec::new();
        while let Some((pos, _)) = self.0.iter().next() {
            stack.push(*pos);
            while let Some(current) = stack.last() {
                if let Some(value) = self.0.get_mut(current) {
                    if value.is_visited {
                        return Err(stack);
                    }
                    value.is_visited = true;
                    match value.iter.next() {
                        Some(x) => {
                            stack.push(x);
                            continue;
                        }
                        None => {
                            self.0.remove(current);
                        }
                    };
                }
                stack.pop();
                if let Some(parent) = stack.last() {
                    let state = self.0.get_mut(parent).unwrap();
                    state.is_visited = false;
                }
            }
        }
        Ok(())
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
            name: type_name::<T>(),
            id: TypeId::of::<T>(),
        }
    }
}

pub struct ServiceBuilder<'col, T: Resolvable>(&'col mut ServiceCollection, PhantomData<T>);
impl<'col, TDep: Resolvable> ServiceBuilder<'col, TDep> {
    pub fn register<T: Any>(&mut self, creator: fn(TDep::ItemPreChecked) -> T) {
        let factory: UntypedFnFactory = Box::new(move |ctx| {
            let key = TDep::precheck(ctx.final_ordered_types)?;
            ctx.register_cyclic_reference_candidate(
                core::any::type_name::<TDep::ItemPreChecked>(),
                Box::new(TDep::iter_positions(ctx.final_ordered_types)),
            );
            let func: Box<dyn Fn(&ServiceProvider) -> T> =
                Box::new(move |container: &ServiceProvider| {
                    let arg = TDep::resolve_prechecked(container, &key);
                    creator(arg)
                });
            Ok(func.into())
        });
        self.0
            .producer_factories
            .push(ServiceProducer::new::<T>(factory));
    }
    pub fn register_shared<T: Any + ?Sized + Send + Sync>(
        &mut self,
        creator: fn(TDep::ItemPreChecked) -> Arc<T>,
    ) {
        let factory: UntypedFnFactory = Box::new(move |ctx| {
            let service_state_idx = ctx.reserve_state_space();
            let key = TDep::precheck(ctx.final_ordered_types)?;
            ctx.register_cyclic_reference_candidate(
                core::any::type_name::<TDep::ItemPreChecked>(),
                Box::new(TDep::iter_positions(ctx.final_ordered_types)),
            );
            let func: Box<dyn Fn(&ServiceProvider) -> Arc<T>> =
                Box::new(move |provider: &ServiceProvider| {
                    let moved_key = &key;
                    provider.get_or_initialize_pos(service_state_idx, move || {
                        creator(TDep::resolve_prechecked(provider, &moved_key))
                    })
                });
            Ok(func.into())
        });
        self.0
            .producer_factories
            .push(ServiceProducer::new::<Arc<T>>(factory));
    }
}

/// ServiceProviders are created directly from ServiceCollections or ServiceProviderFactories and can be used
/// to retrieve services by type. ServiceProviders are final and cannot be modified anymore. When a ServiceProvider goes
/// out of scope, all of its clones and retrieve shared services have to be dropped too. Otherwise
/// dropping the original ServiceProvider panics
pub struct ServiceProvider {
    immutable_state: Arc<ServiceProviderImmutableState>,
    service_states: Arc<Vec<OnceCell<UntypedPointer>>>,
}

impl Debug for ServiceProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
    fn get_or_initialize_pos<T: Clone + Any, TFn: Fn() -> T>(
        &self,
        index: usize,
        initializer: TFn,
    ) -> T {
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
    _parents: Vec<ServiceProvider>,
}

#[cfg(test)]
mod tests {

    use {
        super::*,
        core::sync::atomic::{AtomicI32, Ordering},
    };

    #[test]
    fn resolve_last_transient() {
        let mut col = ServiceCollection::new();
        col.register(|| 0);
        col.register(|| 5);
        col.register(|| 1);
        col.register(|| 2);
        let provider = col.build().expect("Expected to have all dependencies");
        let nr = provider.get::<Registered<i32>>().unwrap();
        assert_eq!(2, nr);
    }

    #[test]
    fn resolve_shared() {
        let mut col = ServiceCollection::new();
        col.register_shared(|| Arc::new(AtomicI32::new(1)));
        col.with::<ServiceProvider>()
            .register_shared(|_| Arc::new(AtomicI32::new(2)));

        let provider = col.build().expect("Should have all Dependencies");
        let service = provider
            .get::<Registered<Arc<AtomicI32>>>()
            .expect("Expecte to get second");
        assert_eq!(2, service.load(Ordering::Relaxed));
        service.fetch_add(40, Ordering::Relaxed);

        assert_eq!(
            provider
                .get::<AllRegistered<Arc<AtomicI32>>>()
                .map(|c| c.load(Ordering::Relaxed))
                .sum::<i32>(),
            1 + 42
        );
    }

    #[test]
    fn build_with_missing_transient_dep_fails() {
        build_with_missing_dependency_fails::<Registered<String>>(&["Registered", "String"]);
    }

    #[test]
    fn build_with_missing_tuple2_dep_fails() {
        build_with_missing_dependency_fails::<(Registered<String>, Registered<i32>)>(&[
            "Registered",
            "String",
        ]);
    }

    #[test]
    fn build_with_missing_tuple3_dep_fails() {
        build_with_missing_dependency_fails::<(Registered<String>, Registered<i32>, Registered<i32>)>(
            &["Registered", "String"],
        );
    }
    #[test]
    fn build_with_missing_tuple4_dep_fails() {
        build_with_missing_dependency_fails::<(
            Registered<i32>,
            Registered<String>,
            Registered<i32>,
            Registered<i32>,
        )>(&["Registered", "String"]);
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
        col.with::<T>().register_shared(|_| Arc::new(()));
        check(col, missing_msg_parts);
    }

    #[test]
    fn resolve_shared_returns_last_registered() {
        let mut container = ServiceCollection::new();
        container.register_shared(|| Arc::new(0));
        container.register_shared(|| Arc::new(1));
        container.register_shared(|| Arc::new(2));
        let provider = container
            .build()
            .expect("Expected to have all dependencies");
        let nr_ref = provider.get::<Registered<Arc<i32>>>().unwrap();
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
        let mut count_subset = provider.get::<AllRegistered<i32>>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get::<AllRegistered::<i32>>().count());

        // Last
        assert_eq!(2, provider.get::<AllRegistered<i32>>().last().unwrap());

        let mut sub = provider.get::<AllRegistered<i32>>();
        sub.next();
        assert_eq!(Some(2), sub.last());

        let mut consumed = provider.get::<AllRegistered<i32>>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());

        let mut iter = provider.get::<AllRegistered<i32>>();
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn no_dependency_needed_if_service_depends_on_services_which_are_not_present() {
        let mut container = ServiceCollection::new();
        container.with::<AllRegistered<String>>().register(|_| 0);

        assert!(container.build().is_ok())
    }

    #[test]
    fn resolve_shared_services() {
        let mut container = ServiceCollection::new();
        container.register_shared(|| Arc::new(0));
        container.register_shared(|| Arc::new(5));
        container.register_shared(|| Arc::new(2));
        let provider = container
            .build()
            .expect("Expected to have all dependencies");

        // Count
        let mut count_subset = provider.get::<AllRegistered<Arc<i32>>>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get::<AllRegistered::<Arc<i32>>>().count());

        // Last
        assert_eq!(
            2,
            *provider.get::<AllRegistered<Arc<i32>>>().last().unwrap()
        );

        let mut sub = provider.get::<AllRegistered<Arc<i32>>>();
        sub.next();
        assert_eq!(Some(2), sub.last().map(|i| *i));

        let mut consumed = provider.get::<AllRegistered<Arc<i32>>>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());

        let mut iter = provider.get::<AllRegistered<Arc<i32>>>().map(|i| *i);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn resolve_test() {
        let mut container = ServiceCollection::new();
        container.register(|| 42);
        container.register_shared(|| Arc::new(42));
        let provider = container
            .build()
            .expect("Expected to have all dependencies");
        assert_eq!(
            provider.get::<Registered::<i32>>().unwrap(),
            provider
                .get::<Registered::<Arc<i32>>>()
                .map(|f| *f)
                .unwrap()
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
                .get::<Registered<i32>>()
        );
    }
    #[test]
    fn get_registered_dynamic_ref() {
        let mut container = ServiceCollection::new();
        container.register_shared(|| Arc::new(42));
        assert_eq!(
            Some(42i32),
            container
                .build()
                .expect("Expected to have all dependencies")
                .get::<Registered<Arc<i32>>>()
                .map(|i| *i)
        );
    }

    #[test]
    fn tuple_dependency_resolves_to_prechecked_type() {
        let mut container = ServiceCollection::new();
        container.register(|| 64i64);
        container
            .with::<(Registered<i64>, Registered<i64>)>()
            .register_shared(|(a, b)| {
                assert_eq!(64, a);
                assert_eq!(64, b);
                Arc::new(42)
            });
        assert_eq!(
            Some(42i32),
            container
                .build()
                .expect("Expected to have all dependencies")
                .get::<Registered<Arc<i32>>>()
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
                .get::<Registered<i32>>()
        );
    }

    #[test]
    fn resolve_tuple_2() {
        let mut container = ServiceCollection::new();
        container.register(|| 32i32);
        container.register_shared(|| Arc::new(64i64));
        let (a, b) = container
            .build()
            .expect("Expected to have all dependencies")
            .get::<(Registered<i32>, Registered<Arc<i64>>)>();
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
        container
            .with::<Registered<Arc<i32>>>()
            .register_shared(|i| Arc::new(ServiceImpl(i)) as Arc<dyn Service + Send + Sync>);
        let provider = container
            .build()
            .expect("Expected to have all dependencies");
        let service = provider
            .get::<Registered<Arc<dyn Service + Send + Sync>>>()
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
        col.register_shared::<Arc<()>>(|| {
            panic!("Should never be called");
        });
        let prov = col.build().unwrap();
        drop(prov);
    }
    #[test]
    fn drop_shareds_after_provider_drop() {
        let mut col = ServiceCollection::new();
        col.register_shared(|| Arc::new(Arc::new(())));
        let prov = col.build().expect("Expected to have all dependencies");
        let inner = prov
            .get::<Registered<Arc<Arc<()>>>>()
            .expect("Expected to receive the service")
            .as_ref()
            .clone();

        assert_eq!(2, Arc::strong_count(&inner));
        drop(prov);
        assert_eq!(1, Arc::strong_count(&inner));
    }
}
