use crate::{
    binary_search,
    lifetime::{
        DanglingCheckerResult, DanglingCheckerResults, LifetimeError, OutlivedLifetimeErrorVariants,
    },
    strategy::{Identifyable, Strategy},
    untyped::{ArcAutoFreePointer, AutoFreePointer, FromArcAutoFreePointer, UntypedFn},
    AllRegistered, AnyStrategy, InternalBuildResult, Registered, Resolvable, ServiceProducer,
    TypeNamed, UntypedFnFactory, UntypedFnFactoryContext,
};
use abi_stable::std_types::{RArc, RVec};
use alloc::vec::Vec;
use core::{
    any::{type_name, Any},
    fmt,
    fmt::{Debug, Formatter},
    marker::PhantomData,
    mem::swap,
};
use std::sync::OnceLock;

/// ServiceProviders are created directly from ServiceCollections or ServiceProviderFactories and can be used
/// to retrieve services by type. ServiceProviders are final and cannot be modified anßymore. When a ServiceProvider goes
/// out of scope, all related WeakServiceProviders and shared services have to be dropped already. Otherwise
/// dropping the original ServiceProvider results in a call to minfac::ERROR_HANDLER, which panics in std and enabled debug_assertions
pub struct ServiceProvider<TS: Strategy + 'static = AnyStrategy> {
    immutable_state: RArc<ServiceProviderImmutableState<TS>>,
    service_states: RArc<ServiceProviderMutableState>,
    #[cfg(debug_assertions)]
    is_root: bool,
}

impl<TS: Strategy + 'static> Debug for ServiceProvider<TS> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
impl<TS: Strategy + 'static> Drop for ServiceProvider<TS> {
    fn drop(&mut self) {
        if !self.is_root {
            return;
        }

        let mut swapped_service_states = RArc::new(ServiceProviderMutableState {
            base: None,
            shared_services: RVec::new(),
        });
        swap(&mut swapped_service_states, &mut self.service_states);

        match RArc::try_unwrap(swapped_service_states) {
            Ok(service_states) => {
                let checkers: Vec<_> = service_states
                    .shared_services
                    .into_iter()
                    .filter_map(|c| {
                        c.get().and_then(|x| {
                            let weak = x.inner.downgrade();
                            if weak.strong_count() > 0 {
                                Some(TypeNamed {
                                    inner: weak,
                                    type_name: x.type_name,
                                })
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                let errors = checkers
                    .into_iter()
                    .filter_map(|x| {
                        (x.inner.strong_count() > 0).then(|| {
                            DanglingCheckerResult::new(x.inner.strong_count(), x.type_name)
                        })
                    })
                    .collect::<DanglingCheckerResults>();

                if errors.len > 0 {
                    unsafe {
                        (crate::MINFAC_ERROR_HANDLER)(&LifetimeError::new(
                            OutlivedLifetimeErrorVariants::SharedServices(errors),
                        ))
                    };
                }
            }
            Err(x) => unsafe {
                let remaining_references = RArc::strong_count(&x) - 1;
                crate::MINFAC_ERROR_HANDLER(&LifetimeError::new(
                    OutlivedLifetimeErrorVariants::WeakServiceProvider {
                        remaining_references,
                    },
                ));
            },
        }
    }
}

impl<TS: Strategy + 'static> ServiceProvider<TS> {
    pub fn resolve_unchecked<T: Resolvable<TS>>(&self) -> T::ItemPreChecked {
        let precheck_key =
            T::precheck(&self.immutable_state.types).expect("Resolve unkwnown service");
        T::resolve_prechecked(self, &precheck_key)
    }

    pub fn get<T: Identifyable<TS::Id>>(&self) -> Option<T> {
        self.resolve_item::<Registered<T>>()
    }
    pub fn get_all<T: Identifyable<TS::Id>>(&self) -> ServiceIterator<T, TS> {
        self.resolve_item::<AllRegistered<T>>()
    }

    /// Returns the Resolvable itself. For less verbose api, consider `get` or `get_all`
    /// This is useful for code, where a Lambda defines it's own dependencies
    ///
    /// ```
    /// use minfac::{ServiceCollection, Registered};
    /// let mut collection = ServiceCollection::new();
    /// collection.register(|| 42);
    /// let provider = collection
    ///     .build()
    ///     .expect("Expected to have all dependencies");
    /// let Registered(x) = provider.resolve::<Registered<i32>>().unwrap();
    /// assert_eq!(42, x);
    /// ```
    pub fn resolve<T: Resolvable<TS>>(&self) -> Option<T> {
        let precheck_key = T::precheck(&self.immutable_state.types).ok()?;
        Some(T::resolve_prechecked_self(self, &precheck_key))
    }

    pub(crate) fn resolve_item<T: Resolvable<TS>>(&self) -> T::Item {
        T::resolve(self)
    }

    pub(crate) fn get_producers(&self) -> &RVec<UntypedFn<TS>> {
        &self.immutable_state.producers
    }

    pub(crate) fn new(
        immutable_state: RArc<ServiceProviderImmutableState<TS>>,
        shared_services: RVec<OnceLock<TypeNamed<ArcAutoFreePointer>>>,
        base: Option<AutoFreePointer>,
    ) -> Self {
        Self {
            immutable_state,

            service_states: RArc::new(ServiceProviderMutableState {
                shared_services,
                base,
            }),
            #[cfg(debug_assertions)]
            is_root: true,
        }
    }

    pub(crate) fn build_service_producer_for_base<T: Identifyable<TS::Id> + Clone + Send + Sync>(
    ) -> UntypedFnFactory<TS> {
        extern "C" fn factory<
            T: Identifyable<TS::Id> + Clone + 'static + Send + Sync,
            TS: Strategy + 'static,
        >(
            stage_1_data: AutoFreePointer,
            _ctx: &mut UntypedFnFactoryContext<TS>,
        ) -> InternalBuildResult<TS> {
            extern "C" fn creator<
                T: Identifyable<TS::Id> + Clone + 'static + Send + Sync,
                TS: Strategy + 'static,
            >(
                provider: *const ServiceProvider<TS>,
                _stage_2_data: *const AutoFreePointer,
            ) -> T {
                let provider = unsafe { &*provider as &ServiceProvider<TS> };
                match &provider.service_states.base {
                    Some(x) => unsafe { &*(x.get_pointer() as *const T) }.clone(),
                    None => panic!("Expected ServiceProviderFactory to set a value for `base`"),
                }
            }
            Ok(UntypedFn::create(creator::<T, TS>, stage_1_data)).into()
        }

        UntypedFnFactory::no_alloc(std::ptr::null(), factory::<T, TS>)
    }

    pub(crate) fn get_or_initialize_pos<
        T: Any + Send + Sync + Into<ArcAutoFreePointer> + FromArcAutoFreePointer,
        TFn: Fn() -> T,
    >(
        &self,
        index: usize,
        initializer: TFn,
    ) -> T {
        let pointer = self.service_states.shared_services[index].get_or_init(|| TypeNamed {
            inner: initializer().into(),
            type_name: type_name::<T>(),
        });

        unsafe { T::from_ref(&pointer.inner) }
    }
}

/// Weak ServiceProviders have the same public API as ServiceProviders, but cannot outlive
/// their original ServiceProvider. If they do, the minfac::ERROR_HANDLER is called.
///
/// In contrast to std::sync::Arc<T> / std::sync::Weak<T>, WeakServiceProviders prevent
/// their parent from being vanished, if minfac::ERROR_HANDLER doesn't panic
pub struct WeakServiceProvider<TS: Strategy + 'static = AnyStrategy>(ServiceProvider<TS>);

impl<TS: Strategy + 'static> WeakServiceProvider<TS> {
    /// Reference for Arc<self> must be kept for the entire lifetime of the new ServiceProvider
    pub(crate) unsafe fn clone_producers(&self) -> impl Iterator<Item = ServiceProducer<TS>> {
        type OuterContextType<TS> = (&'static UntypedFn<TS>, &'static WeakServiceProvider<TS>);
        let static_self = &*(self as *const Self);
        static_self
            .0
            .immutable_state
            .producers
            .iter()
            .zip(static_self.0.immutable_state.types.iter())
            .map(move |(parent_producer, parent_type)| {
                // parents are part of ServiceProviderImmutableState to live as long as the inherited UntypedFn
                extern "C" fn factory<TS: Strategy + 'static>(
                    outer_ctx: AutoFreePointer,
                    _: &mut UntypedFnFactoryContext<TS>,
                ) -> InternalBuildResult<TS> {
                    let ptr = outer_ctx.get_pointer() as *const OuterContextType<TS>;
                    unsafe {
                        let (parent_producer, static_self) = &*ptr;
                        Ok(parent_producer.bind(&static_self.0)).into()
                    }
                }
                let factory =
                    UntypedFnFactory::boxed((parent_producer, static_self), factory::<TS>);
                ServiceProducer::<TS>::new_with_type(factory, *parent_type)
            })
    }

    pub fn resolve_unchecked<T: Resolvable<TS>>(&self) -> T::ItemPreChecked {
        self.0.resolve_unchecked::<T>()
    }

    pub fn resolve<T: Resolvable<TS>>(&self) -> Option<T> {
        self.0.resolve::<T>()
    }

    pub fn get<T: Identifyable<TS::Id>>(&self) -> Option<T> {
        self.0.get::<T>()
    }

    pub fn get_all<T: Identifyable<TS::Id>>(&self) -> ServiceIterator<T, TS> {
        self.0.get_all::<T>()
    }
}

impl<TS: Strategy + 'static> Clone for WeakServiceProvider<TS> {
    fn clone(&self) -> Self {
        Self(ServiceProvider::<TS> {
            immutable_state: self.0.immutable_state.clone(),
            service_states: self.0.service_states.clone(),
            #[cfg(debug_assertions)]
            is_root: false,
        })
    }
}

impl<'a, TS: Strategy + 'static> From<&'a ServiceProvider<TS>> for WeakServiceProvider<TS> {
    fn from(provider: &'a ServiceProvider<TS>) -> Self {
        WeakServiceProvider(ServiceProvider {
            immutable_state: provider.immutable_state.clone(),
            service_states: provider.service_states.clone(),
            #[cfg(debug_assertions)]
            is_root: false,
        })
    }
}

pub(crate) struct ServiceProviderImmutableState<TS: Strategy + 'static> {
    types: RVec<TS::Id>,
    producers: RVec<UntypedFn<TS>>,
    // Unsafe-Code, which generates UntypedFn from parent, relies on the fact that parent ServiceProvider outlives this state
    _parents: RVec<WeakServiceProvider<TS>>,
}

impl<TS: Strategy + 'static> ServiceProviderImmutableState<TS> {
    pub(crate) fn new(
        types: RVec<TS::Id>,
        producers: RVec<UntypedFn<TS>>,
        _parents: RVec<WeakServiceProvider<TS>>,
    ) -> Self {
        Self {
            types,
            producers,
            _parents,
        }
    }
}

pub(crate) struct ServiceProviderMutableState {
    // Placeholder for the type which is provided when serviceProvider is built from ServiceFactory
    base: Option<AutoFreePointer>,
    shared_services: RVec<OnceLock<TypeNamed<ArcAutoFreePointer>>>,
}

/// Type used to retrieve all instances `T` of a `ServiceProvider`.
/// Services are built just in time when calling `next()`
pub struct ServiceIterator<T, TS: Strategy + 'static = AnyStrategy> {
    next_pos: Option<usize>,
    provider: WeakServiceProvider<TS>,
    item_type: PhantomData<T>,
}

impl<T, TS: Strategy + 'static> ServiceIterator<T, TS> {
    pub(crate) fn new(provider: WeakServiceProvider<TS>, next_pos: Option<usize>) -> Self {
        Self {
            provider,
            item_type: PhantomData,
            next_pos,
        }
    }
}

impl<TS: Strategy + 'static, T: Identifyable<TS::Id>> Iterator for ServiceIterator<T, TS> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_pos.map(|i| {
            self.next_pos = self
                .provider
                .0
                .immutable_state
                .producers
                .get(i + 1)
                .and_then(|next| (next.get_result_type_id() == &T::get_id()).then(|| i + 1));

            unsafe { crate::resolvable::resolve_unchecked::<TS, T>(&self.provider.0, i) }
        })
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.next_pos.map(|i| {
            let pos = binary_search::binary_search_last_by_key(
                &self.provider.0.immutable_state.producers[i..],
                &T::get_id(),
                UntypedFn::<TS>::get_result_type_id,
            );
            let pos = pos.expect("to be present if next_pos has value");
            unsafe { crate::resolvable::resolve_unchecked::<TS, T>(&self.provider.0, i + pos) }
        })
    }
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.next_pos
            .map(|i| {
                let pos = binary_search::binary_search_last_by_key(
                    &self.provider.0.immutable_state.producers[i..],
                    &T::get_id(),
                    UntypedFn::get_result_type_id,
                )
                .expect("having at least one item because has next_pos");
                pos + 1
            })
            .unwrap_or(0)
    }
}
