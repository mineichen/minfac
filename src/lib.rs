#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use abi_stable::std_types::{
    RArc,
    RResult::{RErr, ROk},
    RStr, RString, RVec,
};
use alloc::{
    boxed::Box,
    collections::BTreeMap,
    rc::Rc,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::{any::type_name, cell::RefCell, fmt::Debug, marker::PhantomData};
use once_cell::sync::OnceCell;
use service_provider_factory::ServiceProviderFactoryBuilder;
use strategy::{Identifyable, Strategy};
use untyped::{AutoFreePointer, UntypedFn};

mod binary_search;
mod resolvable;
mod service_provider;
mod service_provider_factory;
#[cfg(feature = "stable_abi")]
pub mod stable_abi;
mod strategy;
mod untyped;

pub use resolvable::Resolvable;
pub use service_provider::ServiceIterator;
pub use service_provider::ServiceProvider;
pub use service_provider::WeakServiceProvider;
pub use service_provider_factory::ServiceProviderFactory;
pub use strategy::AnyStrategy;

use crate::resolvable::SealedResolvable;
pub type ServiceCollection = GenericServiceCollection<AnyStrategy>;

type InternalBuildResult<TS> =
    abi_stable::std_types::RResult<UntypedFn<TS>, InternalBuildError<TS>>;

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
pub static mut ERROR_HANDLER: fn(msg: &dyn Debug) = |_msg| {
    #[cfg(feature = "std")]
    if !std::thread::panicking() {
        panic!("{:?}", _msg)
    }
};

/// Represents a query for the last registered instance of `T`
pub struct Registered<T>(PhantomData<T>);

/// Represents a query for all registered instances of Type `T`.
pub struct AllRegistered<T>(PhantomData<T>);

/// Collection of constructors for different types of services. Registered constructors are never called in this state.
/// Instances can only be received by a ServiceProvider, which can be created by calling `build`
pub struct GenericServiceCollection<TS: Strategy + 'static> {
    strategy: PhantomData<TS>,
    producer_factories: Vec<ServiceProducer<TS>>,
}

/// Alias builder is used to register services, which depend on the previous service.
/// This is especially useful, if the previous service contains an anonymous type like a lambda
pub struct AliasBuilder<'a, T: ?Sized, TS: Strategy + 'static>(
    Rc<RefCell<&'a mut GenericServiceCollection<TS>>>,
    PhantomData<T>,
);

impl<'a, T: Identifyable<TS::Id>, TS: Strategy + 'static> AliasBuilder<'a, T, TS> {
    fn new(col: &'a mut GenericServiceCollection<TS>) -> Self {
        Self(Rc::new(RefCell::new(col)), PhantomData)
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
    pub fn alias<TNew: Identifyable<TS::Id>>(
        &mut self,
        creator: fn(T) -> TNew,
    ) -> AliasBuilder<'a, TNew, TS> {
        self.0
            .borrow_mut()
            .with::<Registered<T>>()
            .register(creator);
        AliasBuilder::<_, TS>(self.0.clone(), PhantomData)
    }
}

struct ServiceProducer<TS: Strategy + 'static> {
    identifier: TS::Id,
    factory: UntypedFnFactory<TS>,
}

impl<TS: Strategy + 'static> ServiceProducer<TS> {
    fn new<T: Identifyable<TS::Id>>(factory: UntypedFnFactory<TS>) -> Self {
        Self::new_with_type(factory, T::get_id())
    }
    fn new_with_type(factory: UntypedFnFactory<TS>, type_id: TS::Id) -> Self {
        Self {
            identifier: type_id,
            factory,
        }
    }
}

type UntypedFnFactoryCreator<TS> = extern "C" fn(
    outer_context: AutoFreePointer,
    inner_context: &mut UntypedFnFactoryContext<TS>,
) -> InternalBuildResult<TS>;

struct UntypedFnFactory<TS: Strategy + 'static> {
    creator: UntypedFnFactoryCreator<TS>,
    context: AutoFreePointer,
}

impl<TS: Strategy + 'static> UntypedFnFactory<TS> {
    fn no_alloc(context: usize, creator: UntypedFnFactoryCreator<TS>) -> Self {
        Self {
            creator,
            context: AutoFreePointer::no_alloc(context),
        }
    }
    fn boxed<T>(input: T, creator: UntypedFnFactoryCreator<TS>) -> Self {
        Self {
            creator,
            context: AutoFreePointer::boxed(input),
        }
    }
    fn call(self, ctx: &mut UntypedFnFactoryContext<TS>) -> InternalBuildResult<TS> {
        (self.creator)(self.context, ctx)
    }
}

struct UntypedFnFactoryContext<'a, TS: Strategy + 'static> {
    service_descriptor_pos: usize,
    state_counter: &'a mut usize,
    final_ordered_types: &'a Vec<TS::Id>,
    cyclic_reference_candidates: &'a mut BTreeMap<usize, CycleCheckerValue>,
}

impl<'a, TS: Strategy + 'static> UntypedFnFactoryContext<'a, TS> {
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

impl<TS: Strategy + 'static> Default for GenericServiceCollection<TS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<TS: Strategy + 'static> GenericServiceCollection<TS> {
    /// Creates an empty ServiceCollection
    pub fn new() -> Self {
        Self {
            strategy: PhantomData,
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
    /// collection.with::<AllRegistered<u8>>().register(|i: ServiceIterator<u8>| i.map(|i| i as u32).sum::<u32>());
    /// // Multiple (max tupple size == 4)
    /// collection.with::<(Registered<u8>, Registered<u16>)>().register(|(byte, short)| (byte as u64));
    /// // Nested tuples for more than 4 Dependencies
    /// collection.with::<((Registered<u8>, Registered<u16>), (Registered<u32>, Registered<u64>))>()
    ///     .register(|((byte, short), (integer, long))| (byte as u128 + short as u128 + integer as u128 + long as u128));
    /// collection.with::<WeakServiceProvider>().register(|s: WeakServiceProvider| s.get::<u16>().expect("<i16> is available as optional parameter ") as u32);
    ///
    /// let provider = collection.build().expect("Dependencies are ok");
    /// assert_eq!(Some(42 * 4), provider.get::<u128>());
    /// ```
    pub fn with<T: Resolvable<TS>>(&mut self) -> ServiceBuilder<'_, T, TS> {
        ServiceBuilder(self, PhantomData)
    }

    /// Register an instance to be resolvable
    /// If a ServiceProviderFactory is used, all ServicesProviders will clone from the same origin
    pub fn register_instance<T: Identifyable<TS::Id> + Clone + 'static + Send + Sync>(
        &mut self,
        instance: T,
    ) {
        extern "C" fn factory<
            T: Identifyable<TS::Id> + Clone + 'static + Send + Sync,
            TS: Strategy + 'static,
        >(
            outer_ctx: AutoFreePointer,
            _ctx: &mut UntypedFnFactoryContext<TS>,
        ) -> InternalBuildResult<TS> {
            extern "C" fn func<
                T: Identifyable<TS::Id> + Clone + 'static + Send + Sync,
                TS: Strategy + 'static,
            >(
                _: &ServiceProvider<TS>,
                _outer_ctx: &AutoFreePointer,
            ) -> T {
                unsafe { &*(_outer_ctx.get_pointer() as *mut T) }.clone()
            }
            ROk(UntypedFn::create(func::<T, TS>, outer_ctx))
        }

        let factory = UntypedFnFactory::boxed(instance, factory::<T, TS>);
        self.producer_factories
            .push(ServiceProducer::<TS>::new::<T>(factory));
    }

    /// Registers a transient service without dependencies.
    /// To add dependencies, use `with` to generate a ServiceBuilder.
    pub fn register<T: Identifyable<TS::Id>>(&mut self, creator: fn() -> T) -> AliasBuilder<T, TS> {
        extern "C" fn factory<T: Identifyable<TS::Id>, TS: Strategy + 'static>(
            stage_1_data: AutoFreePointer,
            _ctx: &mut UntypedFnFactoryContext<TS>,
        ) -> InternalBuildResult<TS> {
            extern "C" fn func<T: Identifyable<TS::Id>, TS: Strategy + 'static>(
                _: &ServiceProvider<TS>,
                stage_2_data: &AutoFreePointer,
            ) -> T {
                let ptr = stage_2_data.get_pointer();
                let creator: fn() -> T = unsafe { std::mem::transmute(ptr) };
                creator()
            }
            ROk(UntypedFn::create(func::<T, TS>, stage_1_data))
        }

        let factory = UntypedFnFactory::no_alloc(creator as usize, factory::<T, TS>);
        self.producer_factories
            .push(ServiceProducer::<TS>::new::<T>(factory));
        AliasBuilder::new(self)
    }

    /// Registers a shared service without dependencies.
    /// To add dependencies, use `with` to generate a ServiceBuilder.
    ///
    /// Shared services must have a reference count == 0 after dropping the ServiceProvider. If an Arc is
    /// cloned and thus kept alive, ServiceProvider::drop will panic to prevent service leaking in std.
    pub fn register_shared<T: Send + Sync>(
        &mut self,
        creator: fn() -> Arc<T>,
    ) -> AliasBuilder<Arc<T>, TS>
    where
        Arc<T>: Identifyable<TS::Id>,
    {
        type InnerContext = (usize, usize);
        extern "C" fn factory<T: Send + Sync, TS: Strategy + 'static>(
            outer_ctx: AutoFreePointer, // No-Alloc
            ctx: &mut UntypedFnFactoryContext<TS>,
        ) -> InternalBuildResult<TS>
        where
            Arc<T>: Identifyable<TS::Id>,
        {
            extern "C" fn func<T: Send + Sync + 'static, TS: Strategy + 'static>(
                provider: &ServiceProvider<TS>,
                outer_ctx: &AutoFreePointer,
            ) -> Arc<T> {
                let (service_state_idx, fnptr) =
                    unsafe { &*(outer_ctx.get_pointer() as *mut InnerContext) };
                let creator: fn() -> Arc<T> = unsafe { std::mem::transmute(*fnptr) };
                provider.get_or_initialize_pos(*service_state_idx, creator)
            }
            let service_state_idx = ctx.reserve_state_space();
            let inner: InnerContext = (service_state_idx, outer_ctx.get_pointer());
            ROk(UntypedFn::create(func, AutoFreePointer::boxed(inner)))
        }

        let factory = UntypedFnFactory::no_alloc(creator as usize, factory::<T, TS>);
        self.producer_factories
            .push(ServiceProducer::<TS>::new::<Arc<T>>(factory));

        AliasBuilder::new(self)
    }

    /// Checks, if all dependencies of registered services are available.
    /// If no errors occured, Ok(ServiceProvider) is returned.
    pub fn build(self) -> Result<ServiceProvider<TS>, BuildError<TS>> {
        let validation = self.validate_producers(Vec::new())?;
        let shared_services = (0..validation.service_states_count)
            .map(|_| OnceCell::default())
            .collect();
        let immutable_state = RArc::new(service_provider::ServiceProviderImmutableState::new(
            validation.types,
            validation.producers,
            RVec::new(),
        ));
        Ok(ServiceProvider::<TS>::new(
            immutable_state,
            shared_services,
            None,
        ))
    }

    ///
    /// Returns a factory which can efficiently create ServiceProviders from
    /// ServiceCollections which are missing one dependent service T (e.g. HttpRequest, StartupConfiguration)
    /// The missing service must implement `Any` + `Clone`.
    ///
    /// Unlike shared services, this service's reference counter isn't checked to equal zero when the provider is dropped
    ///
    pub fn build_factory<T: Clone + Identifyable<TS::Id> + Send + Sync>(
        self,
    ) -> Result<ServiceProviderFactory<T, TS>, BuildError<TS>> {
        ServiceProviderFactory::<_, TS>::create(self, RVec::new())
    }

    pub fn with_parent(
        self,
        provider: impl Into<WeakServiceProvider<TS>>,
    ) -> ServiceProviderFactoryBuilder<TS> {
        ServiceProviderFactoryBuilder::create(self, provider.into())
    }

    fn validate_producers(
        self,
        mut factories: Vec<ServiceProducer<TS>>,
    ) -> Result<ProducerValidationResult<TS>, BuildError<TS>> {
        let mut service_states_count: usize = 0;
        factories.extend(self.producer_factories.into_iter());

        factories.sort_by_key(|a| a.identifier);

        let mut final_ordered_types = factories.iter().map(|f| f.identifier).collect();

        let mut cyclic_reference_candidates = BTreeMap::new();
        let mut producers = RVec::with_capacity(factories.len());
        let mut types = RVec::with_capacity(factories.len());

        for (i, x) in factories.into_iter().enumerate() {
            let mut ctx = UntypedFnFactoryContext {
                state_counter: &mut service_states_count,
                final_ordered_types: &mut final_ordered_types,
                cyclic_reference_candidates: &mut cyclic_reference_candidates,
                service_descriptor_pos: i,
            };

            let producer = match x.factory.call(&mut ctx) {
                abi_stable::std_types::RResult::ROk(x) => x,
                abi_stable::std_types::RResult::RErr(e) => return Err(e.into()),
            };
            debug_assert_eq!(&x.identifier, producer.get_result_type_id());
            producers.push(producer);
            types.push(x.identifier);
        }

        CycleChecker(&mut cyclic_reference_candidates)
            .ok()
            .map_err(|indices| BuildError::CyclicDependency {
                description: indices
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
            })?;

        Ok(ProducerValidationResult {
            producers,
            types,
            service_states_count,
        })
    }
}

pub(crate) struct ProducerValidationResult<TS: Strategy + 'static> {
    producers: RVec<UntypedFn<TS>>,
    types: RVec<TS::Id>,
    service_states_count: usize,
}

struct CycleCheckerValue {
    is_visited: bool,
    type_description: &'static str,
    iter: Box<dyn Iterator<Item = usize>>, // Use RVec
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

/// Possible errors when calling ServiceCollection::build() or ServiceCollection::build_factory().
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
pub enum BuildError<TS: Strategy + Debug> {
    /// `name`-format is subject of change and should only be used for debugging purpose
    #[non_exhaustive]
    MissingDependency { id: TS::Id, name: &'static str },
    /// `description`-format is subject of change and should only be used for debugging purpose
    #[non_exhaustive]
    CyclicDependency { description: String },
}

// Internal, ABI-Safe representation
#[repr(C)]
enum InternalBuildError<TS: Strategy + Debug> {
    MissingDependency { id: TS::Id, name: RStr<'static> },
    CyclicDependency { description: RString },
}

impl<TS: Strategy + Debug> From<InternalBuildError<TS>> for BuildError<TS> {
    fn from(i: InternalBuildError<TS>) -> Self {
        match i {
            InternalBuildError::CyclicDependency { description } => BuildError::CyclicDependency {
                description: description.into(),
            },
            InternalBuildError::MissingDependency { id, name } => BuildError::MissingDependency {
                id,
                name: name.into(),
            },
        }
    }
}

impl<TS: Strategy + Debug> From<BuildError<TS>> for InternalBuildError<TS> {
    fn from(i: BuildError<TS>) -> Self {
        match i {
            BuildError::CyclicDependency { description } => InternalBuildError::CyclicDependency {
                description: description.into(),
            },
            BuildError::MissingDependency { id, name } => InternalBuildError::MissingDependency {
                id,
                name: name.into(),
            },
        }
    }
}

impl<TS: Strategy + 'static> BuildError<TS> {
    fn new_missing_dependency<T: Identifyable<TS::Id>>() -> Self {
        BuildError::MissingDependency {
            name: type_name::<T>(),
            id: T::get_id(),
        }
    }
}

#[doc(hidden)]
pub struct ServiceBuilder<'col, T: Resolvable<TS>, TS: Strategy + 'static = AnyStrategy>(
    pub &'col mut GenericServiceCollection<TS>,
    PhantomData<T>,
);

impl<'col, TDep: Resolvable<TS> + 'static, TS: Strategy + 'static> ServiceBuilder<'col, TDep, TS> {
    pub fn register<T: Identifyable<TS::Id>>(
        &mut self,
        creator: fn(TDep::ItemPreChecked) -> T,
    ) -> AliasBuilder<T, TS> {
        type InnerContext<TDep, TS> = (<TDep as SealedResolvable<TS>>::PrecheckResult, usize);
        extern "C" fn factory<
            T: Identifyable<TS::Id>,
            TDep: Resolvable<TS> + 'static,
            TS: Strategy + 'static,
        >(
            outer_ctx: AutoFreePointer, // No-Alloc
            ctx: &mut UntypedFnFactoryContext<TS>,
        ) -> InternalBuildResult<TS> {
            let key = match TDep::precheck(ctx.final_ordered_types) {
                Ok(x) => x,
                Err(x) => return RErr(x.into()),
            };

            ctx.register_cyclic_reference_candidate(
                type_name::<TDep::ItemPreChecked>(),
                Box::new(TDep::iter_positions(ctx.final_ordered_types)),
            );
            extern "C" fn func<
                T: Identifyable<TS::Id>,
                TDep: Resolvable<TS> + 'static,
                TS: Strategy + 'static,
            >(
                provider: &ServiceProvider<TS>,
                _outer_ctx: &AutoFreePointer,
            ) -> T {
                let (key, c): &InnerContext<TDep, TS> =
                    unsafe { &*(_outer_ctx.get_pointer() as *mut InnerContext<TDep, TS>) };
                let creator: fn(TDep::ItemPreChecked) -> T = unsafe { std::mem::transmute(*c) };
                let arg = TDep::resolve_prechecked(provider, key);
                creator(arg)
            }
            let inner: InnerContext<TDep, TS> = (key, outer_ctx.get_pointer());
            ROk(UntypedFn::create(
                func::<T, TDep, TS>,
                AutoFreePointer::boxed(inner),
            ))
        }
        let factory = UntypedFnFactory::no_alloc(creator as usize, factory::<T, TDep, TS>);
        self.0
            .producer_factories
            .push(ServiceProducer::<TS>::new::<T>(factory));

        AliasBuilder::new(self.0)
    }
    pub fn register_shared<T: Send + Sync>(
        &mut self,
        creator: fn(TDep::ItemPreChecked) -> Arc<T>,
    ) -> AliasBuilder<Arc<T>, TS>
    where
        Arc<T>: Identifyable<TS::Id>,
    {
        type InnerContext<TDep, TS> =
            (<TDep as SealedResolvable<TS>>::PrecheckResult, usize, usize);
        extern "C" fn factory<
            T: Send + Sync,
            TDep: Resolvable<TS> + 'static,
            TS: Strategy + 'static,
        >(
            outer_ctx: AutoFreePointer,
            ctx: &mut UntypedFnFactoryContext<TS>,
        ) -> InternalBuildResult<TS>
        where
            Arc<T>: Identifyable<TS::Id>,
        {
            let service_state_idx = ctx.reserve_state_space();
            let key = match TDep::precheck(ctx.final_ordered_types) {
                Ok(x) => x,
                Err(x) => return RErr(x.into()),
            };
            ctx.register_cyclic_reference_candidate(
                type_name::<TDep::ItemPreChecked>(),
                Box::new(TDep::iter_positions(ctx.final_ordered_types)),
            );
            extern "C" fn func<
                T: Send + Sync + 'static,
                TDep: Resolvable<TS> + 'static,
                TS: Strategy + 'static,
            >(
                provider: &ServiceProvider<TS>,
                _outer_ctx: &AutoFreePointer,
            ) -> Arc<T> {
                let (key, c, service_state_idx): &InnerContext<TDep, TS> =
                    unsafe { &*(_outer_ctx.get_pointer() as *mut InnerContext<TDep, TS>) };
                provider.get_or_initialize_pos(*service_state_idx, || {
                    let creator: fn(TDep::ItemPreChecked) -> Arc<T> =
                        unsafe { std::mem::transmute(*c) };
                    creator(TDep::resolve_prechecked(provider, key))
                })
            }
            let inner: InnerContext<TDep, TS> = (key, outer_ctx.get_pointer(), service_state_idx);
            ROk(UntypedFn::create(
                func::<T, TDep, TS>,
                AutoFreePointer::boxed(inner),
            ))
        }
        let factory = UntypedFnFactory::no_alloc(creator as usize, factory::<T, TDep, TS>);
        self.0
            .producer_factories
            .push(ServiceProducer::<TS>::new::<Arc<T>>(factory));

        AliasBuilder::new(self.0)
    }
}

// At the time of writing, core::any::type_name_of_val was behind a nightly feature flag
struct TypeNamed<T> {
    inner: T,
    type_name: &'static str,
}
