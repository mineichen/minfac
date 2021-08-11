#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use {
    alloc::{
        boxed::Box,
        collections::BTreeMap,
        rc::Rc,
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
    service_provider_factory::ServiceProviderFactoryBuilder,
    untyped::{UntypedFn, UntypedPointer},
};

mod binary_search;
mod resolvable;
mod service_provider_factory;
mod untyped;

use core::cell::RefCell;

pub use resolvable::Resolvable;
pub use service_provider_factory::ServiceProviderFactory;

/// Handles lifetime errors, which cannot be enforced using the type system. This is the case when:
/// - WeakServiceProvider outlives the ServiceProvider its created from
/// - ServiceIterator<T>, which owns a WeakServiceProvider internally, outlives its ServiceProvider
/// - Any shared service outlives its ServiceProvider
///
/// Ignoring errors is strongly discouraged, but doesn't cause any undefined behavior or memory leaks by the framework.
/// However, leaking context specific services often lead to memory leaks in user code which are difficult to find:
/// All shared references of a ServiceProvider are kept alive if the result of a single provider::get::<AllRegistered<i32>>() call
/// is leaking it's provider. This can easily happen, if you forget to collect the results into a vector.
/// To prevent these sneaky errors, ServiceProvider::drop() ensures that none of it's internals are kept alive when debug_assertions are enabled.
///
/// The default implementation panics, if the std-feature is enabled (on by default). Otherwise this is a no_op
/// For custom implementations, be aware that this function could be called while panicking already.
/// In std, panic!(), when the thread is panicking already, terminates the entire program immediately.
///
/// This variable only exists, if debug_assertions are enabled
#[cfg(debug_assertions)]
pub static mut ERROR_HANDLER: fn(msg: &dyn core::fmt::Debug) = |msg| {
    #[cfg(feature = "std")]
    if !std::thread::panicking() {
        panic!("{:?}", msg)
    }
};

/// Type used to retrieve all instances `T` of a `ServiceProvider`.
/// Services are built just in time when calling `next()`
pub struct ServiceIterator<T> {
    next_pos: Option<usize>,
    provider: WeakServiceProvider,
    item_type: PhantomData<T>,
}

/// Represents a query for the last registered instance of `T`
pub struct Registered<T: Any>(PhantomData<T>);

/// Represents a query for all registered instances of Type `T`.
pub struct AllRegistered<T: Any>(PhantomData<T>);

/// Collection of constructors for different types of services. Registered constructors are never called in this state.
/// Instances can only be received by a ServiceProvider, which can be created by calling `build`
pub struct ServiceCollection {
    producer_factories: Vec<ServiceProducer>,
}

/// Alias builder is used to register services, which depend on the previous service. 
/// This is especially useful, if the previous service contains an anonymous type like a lambda
pub struct AliasBuilder<'a, T: ?Sized>(Rc<RefCell<&'a mut ServiceCollection>>, PhantomData<T>);

impl<'a, T: Any> AliasBuilder<'a, T> {
    fn new(col: &'a mut ServiceCollection) -> Self {
        AliasBuilder(Rc::new(RefCell::new(col)), PhantomData)
    }

    /// Registers an aliased service. The returned AliasBuilder refers to the new type
    /// ``` rust
    /// let mut col = minfac::ServiceCollection::new();
    /// let mut i8alias = col.register(|| 1i8)
    ///     .alias(|a| a as i16 * 2)
    ///     .alias(|a| a as i32 * 2);
    /// let prov = col.build().unwrap();
    /// assert_eq!(Some(2i16), prov.get());
    /// assert_eq!(Some(4i32), prov.get());
    /// ```
    pub fn alias<TNew: Any>(&mut self, creator: fn(T) -> TNew) -> AliasBuilder<'a, TNew> {
        self.0
            .borrow_mut()
            .with::<Registered<T>>()
            .register(creator);
        AliasBuilder(self.0.clone(), PhantomData)
    }
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
    cyclic_reference_candidates: &'a mut BTreeMap<usize, CycleCheckerValue>,
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
            CycleCheckerValue {
                is_visited: false,
                type_description: type_name,
                iter: dependencies,
            },
        );
    }
}

impl Default for ServiceCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceCollection {
    /// Creates an empty ServiceCollection
    pub fn new() -> Self {
        Self {
            producer_factories: Vec::new(),
        }
    }

    /// Generate a ServiceBuilder with `T` as a dependency.
    /// An instance of T is provided as an argument to the factory fn:
    /// ``` rust
    /// use {minfac::{AllRegistered, Registered, ServiceCollection, ServiceIterator, WeakServiceProvider}};
    ///
    /// let mut collection = ServiceCollection::new();
    ///
    /// // No dependency
    /// collection.register(|| 42u8);
    /// // Single Dependency
    /// collection.with::<Registered<u8>>().register(|i: u8| i as u16);
    /// // All of a type
    /// collection.with::<AllRegistered<u8>>().register(|i: ServiceIterator<Registered<u8>>| i.map(|i| i as u32).sum::<u32>());
    /// // Multiple (max tupple size == 4)
    /// collection.with::<(Registered<u8>, Registered<u16>)>().register(|(byte, short)| (byte as u64));
    /// // Nested tuples for more than 4 Dependencies
    /// collection.with::<((Registered<u8>, Registered<u16>), (Registered<u32>, Registered<u64>))>()
    ///     .register(|((byte, short), (integer, long))| (byte as u128 + short as u128 + integer as u128 + long as u128));
    /// // Inject WeakServiceProvider for optional dependencies or to pass it to a factory
    /// collection.with::<WeakServiceProvider>().register(|s: WeakServiceProvider| s.get::<u16>().unwrap() as u32);
    ///
    /// let provider = collection.build().expect("Dependencies are ok");
    /// assert_eq!(Some(42 * 4), provider.get::<u128>());
    /// ```
    pub fn with<T: Resolvable>(&mut self) -> ServiceBuilder<'_, T> {
        ServiceBuilder(self, PhantomData)
    }

    /// Register an instance to be resolvable
    /// If a ServiceProviderFactory is used, all ServicesProviders will clone from the same origin
    pub fn register_instance<T: Clone + 'static + Send + Sync>(&mut self, instance: T) {
        let factory: UntypedFnFactory = Box::new(move |_service_state_counter| {
            let func: Box<dyn Fn(&ServiceProvider) -> T> =
                Box::new(move |_: &ServiceProvider| instance.clone());
            Ok(func.into())
        });
        self.producer_factories
            .push(ServiceProducer::new::<T>(factory));
    }

    /// Registers a transient service without dependencies.
    /// To add dependencies, use `with` to generate a ServiceBuilder.
    pub fn register<'a, T: Any>(&'a mut self, creator: fn() -> T) -> AliasBuilder<'a, T> {
        let factory: UntypedFnFactory = Box::new(move |_service_state_counter| {
            let func: Box<dyn Fn(&ServiceProvider) -> T> =
                Box::new(move |_: &ServiceProvider| creator());
            Ok(func.into())
        });
        self.producer_factories
            .push(ServiceProducer::new::<T>(factory));
        AliasBuilder::new(self)
    }

    /// Registers a shared service without dependencies.
    /// To add dependencies, use `with` to generate a ServiceBuilder.
    ///
    /// Shared services must have a reference count == 0 after dropping the ServiceProvider. If an Arc is
    /// cloned and thus kept alive, ServiceProvider::drop will panic to prevent service leaking in std.
    pub fn register_shared<'a, T: Any + Send + Sync>(
        &'a mut self,
        creator: fn() -> Arc<T>,
    ) -> AliasBuilder<'a, Arc<T>> {
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

        AliasBuilder::new(self)
    }

    /// Checks, if all dependencies of registered services are available.
    /// If no errors occured, Ok(ServiceProvider) is returned.
    pub fn build(self) -> Result<ServiceProvider, BuildError> {
        let (producers, types, service_states_count) = self.validate_producers(Vec::new())?;
        let shared_services = vec![OnceCell::new(); service_states_count];
        let immutable_state = Arc::new(ServiceProviderImmutableState {
            producers,
            types,
            _parents: Vec::new(),
        });
        Ok(ServiceProvider {
            immutable_state,
            service_states: Arc::new(ServiceProviderMutableState {
                shared_services,
                base: None,
            }),
            #[cfg(debug_assertions)]
            is_root: true,
        })
    }

    ///
    /// Returns a factory which can efficiently create ServiceProviders from
    /// ServiceCollections which are missing one dependent service T (e.g. HttpRequest, StartupConfiguration)
    /// The missing service must implement `Any` + `Clone`.
    ///
    /// Unlike shared services, this service's reference counter isn't checked to equal zero when the provider is dropped
    ///
    pub fn build_factory<T: Clone + Any + Send + Sync>(
        self,
    ) -> Result<ServiceProviderFactory<T>, BuildError> {
        ServiceProviderFactory::create(self, Vec::new())
    }

    pub fn with_parent(
        self,
        provider: impl Into<WeakServiceProvider>,
    ) -> ServiceProviderFactoryBuilder {
        ServiceProviderFactoryBuilder::create(self, provider.into())
    }

    fn validate_producers(
        self,
        mut factories: Vec<ServiceProducer>,
    ) -> Result<(Vec<UntypedFn>, Vec<TypeId>, usize), BuildError> {
        let mut state_counter: usize = 0;
        factories.extend(self.producer_factories.into_iter());

        factories.sort_by_key(|a| a.type_id);

        let mut final_ordered_types: Vec<TypeId> = factories.iter().map(|f| f.type_id).collect();

        let mut cyclic_reference_candidates = BTreeMap::new();
        let mut producers = Vec::with_capacity(factories.len());
        let mut types = Vec::with_capacity(factories.len());

        for (i, x) in factories.into_iter().enumerate() {
            let mut ctx = UntypedFnFactoryContext {
                state_counter: &mut state_counter,
                final_ordered_types: &mut final_ordered_types,
                cyclic_reference_candidates: &mut cyclic_reference_candidates,
                service_descriptor_pos: i,
            };
            let producer = (x.factory)(&mut ctx)?;
            debug_assert_eq!(&x.type_id, producer.get_result_type_id());
            producers.push(producer);
            types.push(x.type_id);
        }

        CycleChecker(&mut cyclic_reference_candidates)
            .ok()
            .map_err(|indices| {
                BuildError::CyclicDependency(
                    indices
                        .into_iter()
                        .skip(1)
                        .map(|i| {
                            cyclic_reference_candidates
                                .get(&i)
                                .unwrap()
                                .type_description
                        })
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

        Ok((producers, types, state_counter))
    }
}

struct CycleCheckerValue {
    is_visited: bool,
    type_description: &'static str,
    iter: Box<dyn Iterator<Item = usize>>,
}
struct CycleChecker<'a>(&'a mut BTreeMap<usize, CycleCheckerValue>);

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
#[non_exhaustive]
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

pub struct ServiceBuilder<'col, T: Resolvable>(pub &'col mut ServiceCollection, PhantomData<T>);

impl<'col, TDep: Resolvable> ServiceBuilder<'col, TDep> {
    pub fn register<'a, T: core::any::Any>(
        &'a mut self,
        creator: fn(TDep::ItemPreChecked) -> T,
    ) -> AliasBuilder<'a, T> {
        let factory: UntypedFnFactory = Box::new(move |ctx| {
            let key = TDep::precheck(ctx.final_ordered_types)?;
            ctx.register_cyclic_reference_candidate(
                core::any::type_name::<TDep::ItemPreChecked>(),
                Box::new(TDep::iter_positions(ctx.final_ordered_types)),
            );
            let func: Box<dyn Fn(&ServiceProvider) -> T> =
                Box::new(move |provider: &ServiceProvider| {
                    let arg = TDep::resolve_prechecked(provider, &key);
                    creator(arg)
                });
            Ok(func.into())
        });
        self.0
            .producer_factories
            .push(ServiceProducer::new::<T>(factory));

        AliasBuilder::new(&mut self.0)
    }
    pub fn register_shared<'a, T: core::any::Any + Send + Sync>(
        &'a mut self,
        creator: fn(TDep::ItemPreChecked) -> alloc::sync::Arc<T>,
    ) -> AliasBuilder<Arc<T>> {
        let factory: UntypedFnFactory = Box::new(move |ctx| {
            let service_state_idx = ctx.reserve_state_space();
            let key = TDep::precheck(ctx.final_ordered_types)?;
            ctx.register_cyclic_reference_candidate(
                core::any::type_name::<TDep::ItemPreChecked>(),
                Box::new(TDep::iter_positions(ctx.final_ordered_types)),
            );
            let func: Box<dyn Fn(&ServiceProvider) -> alloc::sync::Arc<T>> =
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
            .push(ServiceProducer::new::<alloc::sync::Arc<T>>(factory));

        AliasBuilder::new(&mut self.0)
    }
}

/// ServiceProviders are created directly from ServiceCollections or ServiceProviderFactories and can be used
/// to retrieve services by type. ServiceProviders are final and cannot be modified anymore. When a ServiceProvider goes
/// out of scope, all related WeakServiceProviders and shared services have to be dropped already. Otherwise
/// dropping the original ServiceProvider results in a call to minfac::ERROR_HANDLER, which panics in std and enabled debug_assertions
pub struct ServiceProvider {
    immutable_state: Arc<ServiceProviderImmutableState>,
    service_states: Arc<ServiceProviderMutableState>,
    #[cfg(debug_assertions)]
    is_root: bool,
}

impl Debug for ServiceProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "ServiceProvider (services: {}, with_state: {})",
            self.immutable_state.producers.len(),
            self.service_states.shared_services.len()
        ))
    }
}

/// Dropping ServiceProviders created by ServiceCollection::build() or ServiceProviderFactory::build()
/// directly are expected to have no remaining clones when they are dropped. Clones could be used in services
/// which have a dependency to ServiceProvider or ServiceIterators<T>, which are using ServiceProvider internally)
#[cfg(debug_assertions)]
#[allow(clippy::needless_collect)]
impl Drop for ServiceProvider {
    fn drop(&mut self) {
        if !self.is_root {
            return;
        }

        let mut swapped_service_states = Arc::new(ServiceProviderMutableState {
            base: None,
            shared_services: Vec::new(),
        });
        core::mem::swap(&mut swapped_service_states, &mut self.service_states);

        match Arc::try_unwrap(swapped_service_states) {
            Ok(service_states) => {
                let checkers: Vec<_> = service_states
                    .shared_services
                    .into_iter()
                    .filter_map(|c| c.get().and_then(|x| x.get_weak_checker_if_dangling()))
                    .collect();
                let errors: Vec<_> = checkers
                    .into_iter()
                    .filter_map(|c| {
                        let v = (c)();
                        (v.remaining_references > 0).then(|| v)
                    })
                    .collect();

                if !errors.is_empty() {
                    unsafe {
                        ERROR_HANDLER(&alloc::format!(
                            "Some instances outlived their ServiceProvider: {:?}",
                            errors
                        ))
                    };
                }
            }
            Err(x) => unsafe {
                ERROR_HANDLER(&alloc::format!(
                    "Original ServiceProvider was dropped while still beeing used {} times",
                    Arc::strong_count(&x) - 1
                ));
            },
        }
    }
}

impl ServiceProvider {
    fn resolve<T: Resolvable>(&self) -> T::Item {
        T::resolve(self)
    }

    pub fn resolve_unchecked<T: Resolvable>(&self) -> T::ItemPreChecked {
        let precheck_key =
            T::precheck(&self.immutable_state.types).expect("Resolve unkwnown service");
        T::resolve_prechecked(self, &precheck_key)
    }

    pub fn get<T: Any>(&self) -> Option<T> {
        self.resolve::<Registered<T>>()
    }
    pub fn get_all<T: Any>(&self) -> ServiceIterator<Registered<T>> {
        self.resolve::<AllRegistered<T>>()
    }

    fn get_or_initialize_pos<T: Any + Send + Sync, TFn: Fn() -> Arc<T>>(
        &self,
        index: usize,
        initializer: TFn,
    ) -> Arc<T> {
        let pointer = self
            .service_states
            .shared_services
            .get(index)
            .unwrap()
            .get_or_init(|| UntypedPointer::new(initializer()));
        unsafe { pointer.clone_as::<Arc<T>>() }
    }
}

/// Weak ServiceProviders have the same public API as ServiceProviders, but cannot outlive
/// their original ServiceProvider. If they do, the minfac::ERROR_HANDLER is called
pub struct WeakServiceProvider(ServiceProvider);

impl WeakServiceProvider {
    fn resolve<T: Resolvable>(&self) -> T::Item {
        T::resolve(&self.0)
    }

    pub fn resolve_unchecked<T: Resolvable>(&self) -> T::ItemPreChecked {
        let precheck_key =
            T::precheck(&self.0.immutable_state.types).expect("Resolve unkwnown service");
        T::resolve_prechecked(&self.0, &precheck_key)
    }

    pub fn get<T: Any>(&self) -> Option<T> {
        self.resolve::<Registered<T>>()
    }

    pub fn get_all<T: Any>(&self) -> ServiceIterator<Registered<T>> {
        self.resolve::<AllRegistered<T>>()
    }
}

impl Clone for WeakServiceProvider {
    fn clone(&self) -> Self {
        Self(ServiceProvider {
            immutable_state: self.0.immutable_state.clone(),
            service_states: self.0.service_states.clone(),
            #[cfg(debug_assertions)]
            is_root: false,
        })
    }
}

impl<'a> From<&'a ServiceProvider> for WeakServiceProvider {
    fn from(provider: &'a ServiceProvider) -> Self {
        WeakServiceProvider(ServiceProvider {
            immutable_state: provider.immutable_state.clone(),
            service_states: provider.service_states.clone(),
            #[cfg(debug_assertions)]
            is_root: false,
        })
    }
}

struct ServiceProviderImmutableState {
    types: Vec<TypeId>,
    producers: Vec<UntypedFn>,
    // Unsafe-Code, which generates UntypedFn from parent, relies on the fact that parent ServiceProvider outlives this state
    _parents: Vec<WeakServiceProvider>,
}

struct ServiceProviderMutableState {
    base: Option<Box<dyn Any + Send + Sync>>,
    shared_services: Vec<OnceCell<UntypedPointer>>,
}

#[cfg(test)]
mod tests {

    use {
        super::*,
        core::sync::atomic::{AtomicI32, Ordering},
    };

    #[test]
    #[should_panic(expected = "Panicking while copy exists")]
    fn drop_service_provider_with_existing_clone_on_panic_is_recoverable_with_default_error_handler(
    ) {
        let mut _outer = None;
        {
            let mut collection = ServiceCollection::new();
            collection.with::<WeakServiceProvider>().register(|p| p);
            let provider = collection.build().unwrap();
            _outer = Some(provider.get::<WeakServiceProvider>().unwrap());
            panic!("Panicking while copy exists");
        }
    }

    #[test]
    #[should_panic(expected = "Panicking while shared exists")]
    fn drop_service_provider_with_existing_shared_registered_on_panic_is_recoverable_with_default_error_handler(
    ) {
        let mut _outer = None;
        {
            let mut collection = ServiceCollection::new();
            collection.register_shared(|| Arc::new(1i32));
            let provider = collection.build().unwrap();
            _outer = provider.get::<Arc<i32>>();
            panic!("Panicking while shared exists");
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(
        expected = "Some instances outlived their ServiceProvider: [Type: i32 (remaining 1)]"
    )]
    fn drop_service_provider_with_existing_shared_registered_is_panicking() {
        let mut _outer = None;
        {
            let mut collection = ServiceCollection::new();
            collection.register_shared(|| Arc::new(1i32));
            let provider = collection.build().unwrap();
            _outer = provider.get::<Arc<i32>>();
        }
    }

    #[test]
    fn resolve_last() {
        let mut col = ServiceCollection::new();
        col.register(|| 0);
        col.register(|| 5);
        col.register(|| 1);
        col.register(|| 2);
        let provider = col.build().expect("Expected to have all dependencies");
        assert_eq!(Some(2), provider.get());
    }

    #[test]
    fn resolve_shared() {
        let mut col = ServiceCollection::new();
        col.register_shared(|| Arc::new(AtomicI32::new(1)));
        col.with::<WeakServiceProvider>()
            .register_shared(|_| Arc::new(AtomicI32::new(2)));

        let provider = col.build().expect("Should have all Dependencies");
        let service = provider
            .get::<Arc<AtomicI32>>()
            .expect("Expecte to get second");
        assert_eq!(2, service.load(Ordering::Relaxed));
        service.fetch_add(40, Ordering::Relaxed);

        assert_eq!(
            provider
                .get_all::<Arc<AtomicI32>>()
                .map(|c| c.load(Ordering::Relaxed))
                .sum::<i32>(),
            1 + 42
        );
    }

    #[test]
    fn build_with_missing_dep_fails() {
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
        let mut collection = ServiceCollection::new();
        collection.register_shared(|| Arc::new(0));
        collection.register_shared(|| Arc::new(1));
        collection.register_shared(|| Arc::new(2));
        let provider = collection
            .build()
            .expect("Expected to have all dependencies");
        let nr_ref = provider.get::<Arc<i32>>().unwrap();
        assert_eq!(2, *nr_ref);
    }

    #[test]
    fn resolve_all_services() {
        let mut collection = ServiceCollection::new();
        collection.register(|| 0);
        collection.register(|| 5);
        collection.register(|| 2);
        let provider = collection
            .build()
            .expect("Expected to have all dependencies");

        // Count
        let mut count_subset = provider.get_all::<i32>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get_all::<i32>().count());

        // Last
        assert_eq!(2, provider.get_all::<i32>().last().unwrap());

        let mut sub = provider.get_all::<i32>();
        sub.next();
        assert_eq!(Some(2), sub.last());

        let mut consumed = provider.get_all::<i32>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());

        let mut iter = provider.get_all::<i32>();
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn no_dependency_needed_if_service_depends_on_services_which_are_not_present() {
        let mut collection = ServiceCollection::new();
        collection.with::<AllRegistered<String>>().register(|_| 0);

        assert!(collection.build().is_ok())
    }

    #[test]
    fn resolve_shared_services() {
        let mut collection = ServiceCollection::new();
        collection.register_shared(|| Arc::new(0));
        collection.register_shared(|| Arc::new(5));
        collection.register_shared(|| Arc::new(2));
        let provider = collection
            .build()
            .expect("Expected to have all dependencies");

        // Count
        let mut count_subset = provider.get_all::<Arc<i32>>();
        count_subset.next();
        assert_eq!(2, count_subset.count());
        assert_eq!(3, provider.get_all::<Arc<i32>>().count());

        // Last
        assert_eq!(2, *provider.get_all::<Arc<i32>>().last().unwrap());

        let mut sub = provider.get_all::<Arc<i32>>();
        sub.next();
        assert_eq!(Some(2), sub.last().map(|i| *i));

        let mut consumed = provider.get_all::<Arc<i32>>();
        consumed.by_ref().for_each(|_| {});
        assert_eq!(None, consumed.last());

        let mut iter = provider.get_all::<Arc<i32>>().map(|i| *i);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn resolve_test() {
        let mut collection = ServiceCollection::new();
        collection.register(|| 42);
        collection.register_shared(|| Arc::new(42));
        let provider = collection
            .build()
            .expect("Expected to have all dependencies");
        assert_eq!(
            provider.get::<i32>(),
            provider.get::<Arc<i32>>().map(|f| *f)
        );
    }

    #[test]
    fn get_registered_dynamic_id() {
        let mut collection = ServiceCollection::new();
        collection.register(|| 42);
        assert_eq!(
            Some(42i32),
            collection
                .build()
                .expect("Expected to have all dependencies")
                .get()
        );
    }
    #[test]
    fn get_registered_dynamic_ref() {
        let mut collection = ServiceCollection::new();
        collection.register_shared(|| Arc::new(42));
        assert_eq!(
            Some(42i32),
            collection
                .build()
                .expect("Expected to have all dependencies")
                .get::<Arc<i32>>()
                .map(|i| *i)
        );
    }

    #[test]
    fn tuple_dependency_resolves_to_prechecked_type() {
        let mut collection = ServiceCollection::new();
        collection.register(|| 64i64);
        collection
            .with::<(Registered<i64>, Registered<i64>)>()
            .register_shared(|(a, b)| {
                assert_eq!(64, a);
                assert_eq!(64, b);
                Arc::new(42)
            });
        assert_eq!(
            Some(42i32),
            collection
                .build()
                .expect("Expected to have all dependencies")
                .get::<Arc<i32>>()
                .map(|i| *i)
        );
    }

    #[test]
    fn get_unkown_returns_none() {
        let collection = ServiceCollection::new();
        assert_eq!(
            None,
            collection
                .build()
                .expect("Expected to have all dependencies")
                .get::<i32>()
        );
    }

    #[test]
    fn resolve_tuple_2() {
        let mut collection = ServiceCollection::new();
        collection.register(|| 32i32);
        collection.register_shared(|| Arc::new(64i64));
        let provider = collection
            .build()
            .expect("Expected to have all dependencies");
        let (a, b) = provider.resolve::<(Registered<i32>, Registered<Arc<i64>>)>();
        assert_eq!(Some(32), a);
        assert_eq!(Some(64), b.map(|i| *i));
    }

    #[test]
    fn register_struct_as_dynamic() {
        let mut collection = ServiceCollection::new();
        collection.register_shared(|| Arc::new(42i32));
        collection
            .with::<Registered<Arc<i32>>>()
            .register_shared(|i| Arc::new(ServiceImpl(i)))
            .alias(|a| a as Arc<dyn Service + Send + Sync>);
        let provider = collection
            .build()
            .expect("Expected to have all dependencies");
        let service = provider
            .get::<Arc<dyn Service + Send + Sync>>()
            .expect("Expected to get a service");

        assert_eq!(42, service.get_value());
        drop(service);
        drop(provider);
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
            .get::<Arc<Arc<()>>>()
            .expect("Expected to receive the service")
            .as_ref()
            .clone();

        assert_eq!(2, Arc::strong_count(&inner));
        drop(prov);
        assert_eq!(1, Arc::strong_count(&inner));
    }

    #[test]
    fn register_instance() {
        let mut col = ServiceCollection::new();
        col.register_instance(42);
        let prov = col.build().unwrap();
        assert_eq!(Some(42), prov.get());
    }

    #[test]
    fn register_multiple_alias_per_type() {
        let mut col = ServiceCollection::new();
        let mut i8alias = col.register(|| 1i8);
        let mut i16alias = i8alias.alias(|a| a as i16 * 2);
        i8alias.alias(|a| a as i32 * 2);
        i16alias.alias(|a| a as i64 * 2);

        let prov = col.build().unwrap();
        assert_eq!(Some(2i32), prov.get());
        assert_eq!(Some(4i64), prov.get());
    }
}
