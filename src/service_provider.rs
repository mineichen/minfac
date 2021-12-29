use crate::{
    binary_search,
    strategy::{Identifyable, Strategy},
    untyped::UntypedFn,
    AllRegistered, AnyStrategy, Registered, Resolvable, ServiceProducer, TypeNamed,
    UntypedFnFactory,
};
use alloc::{
    boxed::Box,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    any::{type_name, Any},
    fmt,
    fmt::{Debug, Formatter},
    marker::PhantomData,
    mem::swap,
};
use once_cell::sync::OnceCell;

/// ServiceProviders are created directly from ServiceCollections or ServiceProviderFactories and can be used
/// to retrieve services by type. ServiceProviders are final and cannot be modified anßymore. When a ServiceProvider goes
/// out of scope, all related WeakServiceProviders and shared services have to be dropped already. Otherwise
/// dropping the original ServiceProvider results in a call to minfac::ERROR_HANDLER, which panics in std and enabled debug_assertions
pub struct ServiceProvider<TS: Strategy = AnyStrategy> {
    immutable_state: Arc<ServiceProviderImmutableState<TS>>,
    service_states: Arc<ServiceProviderMutableState>,
    #[cfg(debug_assertions)]
    is_root: bool,
}

impl<TS: Strategy> Debug for ServiceProvider<TS> {
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
impl<TS: Strategy> Drop for ServiceProvider<TS> {
    fn drop(&mut self) {
        if !self.is_root {
            return;
        }

        let mut swapped_service_states = Arc::new(ServiceProviderMutableState {
            // Todo: Just swap shared_services?
            base: None,
            shared_services: Vec::new(),
        });
        swap(&mut swapped_service_states, &mut self.service_states);

        match Arc::try_unwrap(swapped_service_states) {
            Ok(service_states) => {
                let checkers: Vec<_> = service_states
                    .shared_services
                    .into_iter()
                    .filter_map(|c| {
                        c.get().and_then(|x| {
                            if Arc::strong_count(&x.inner) > 1 {
                                Some(x.map(|i| Arc::downgrade(i)))
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                let errors: Vec<_> = checkers
                    .into_iter()
                    .filter_map(|c| {
                        (Weak::strong_count(&c.inner) > 0).then(|| DanglingCheckerResult {
                            remaining_references: Weak::strong_count(&c.inner),
                            typename: c.type_name,
                        })
                    })
                    .collect();

                if !errors.is_empty() {
                    unsafe {
                        crate::ERROR_HANDLER(&alloc::format!(
                            "Some instances outlived their ServiceProvider: {:?}",
                            errors
                        ))
                    };
                }
            }
            Err(x) => unsafe {
                crate::ERROR_HANDLER(&alloc::format!(
                    "Original ServiceProvider was dropped while still beeing used {} times",
                    Arc::strong_count(&x) - 1
                ));
            },
        }
    }
}

impl<TS: Strategy> ServiceProvider<TS> {
    pub fn resolve_unchecked<T: Resolvable<TS>>(&self) -> T::ItemPreChecked {
        let precheck_key =
            T::precheck(&self.immutable_state.types).expect("Resolve unkwnown service");
        T::resolve_prechecked(self, &precheck_key)
    }

    pub fn get<T: Identifyable<TS::Id>>(&self) -> Option<T> {
        self.resolve::<Registered<T>>()
    }
    pub fn get_all<T: Identifyable<TS::Id>>(&self) -> ServiceIterator<T, TS> {
        self.resolve::<AllRegistered<T>>()
    }

    pub(crate) fn resolve<T: Resolvable<TS>>(&self) -> T::Item {
        T::resolve(self)
    }

    pub(crate) fn get_producers(&self) -> &Vec<UntypedFn<TS>> {
        &self.immutable_state.producers
    }

    pub(crate) fn new(
        immutable_state: Arc<ServiceProviderImmutableState<TS>>,
        shared_services: Vec<OnceCell<TypeNamed<Arc<dyn Any + Send + Sync>>>>,
        base: Option<Box<dyn Any + Send + Sync>>,
    ) -> Self {
        Self {
            immutable_state,

            service_states: Arc::new(ServiceProviderMutableState {
                shared_services,
                base,
            }),
            #[cfg(debug_assertions)]
            is_root: true,
        }
    }

    pub(crate) fn build_service_producer_for_base<T: Identifyable<TS::Id> + Clone + Send + Sync>(
    ) -> UntypedFnFactory<TS> {
        Box::new(|_service_state_counter| {
            let creator: Box<dyn Fn(&ServiceProvider<TS>) -> T> =
                Box::new(|provider| match &provider.service_states.base {
                    Some(x) => x.downcast_ref::<T>().unwrap().clone(),
                    None => panic!("Expected ServiceProviderFactory to set a value for `base`"),
                });
            Ok(creator.into())
        })
    }

    pub(crate) fn get_or_initialize_pos<T: Any + Send + Sync, TFn: Fn() -> Arc<T>>(
        &self,
        index: usize,
        initializer: TFn,
    ) -> Arc<T> {
        let pointer = self
            .service_states
            .shared_services
            .get(index)
            .unwrap()
            .get_or_init(|| TypeNamed {
                inner: initializer(),
                type_name: type_name::<T>(),
            });

        pointer
            .clone()
            .inner
            .downcast::<T>()
            .expect("This is likely a bug in minfac. Cell should never contain uncastable value")
    }
}

/// Weak ServiceProviders have the same public API as ServiceProviders, but cannot outlive
/// their original ServiceProvider. If they do, the minfac::ERROR_HANDLER is called
pub struct WeakServiceProvider<TS: Strategy = AnyStrategy>(ServiceProvider<TS>);

impl<TS: Strategy> WeakServiceProvider<TS> {
    fn resolve<T: Resolvable<TS>>(&self) -> T::Item {
        T::resolve(&self.0)
    }

    /// Reference for Arc<self> must be kept for the entire lifetime of the new ServiceProvider
    pub(crate) unsafe fn clone_producers(&self) -> impl Iterator<Item = ServiceProducer<TS>> {
        let static_self = &*(self as *const Self);
        static_self
            .0
            .immutable_state
            .producers
            .iter()
            .zip(static_self.0.immutable_state.types.iter())
            .map(move |(parent_producer, parent_type)| {
                // parents are part of ServiceProviderImmutableState to live as long as the inherited UntypedFn
                let factory = parent_producer.bind(&static_self.0);
                ServiceProducer::<TS>::new_with_type(Box::new(move |_| Ok(factory)), *parent_type)
            })
    }

    pub fn resolve_unchecked<T: Resolvable<TS>>(&self) -> T::ItemPreChecked {
        let precheck_key =
            T::precheck(&self.0.immutable_state.types).expect("Resolve unkwnown service");
        T::resolve_prechecked(&self.0, &precheck_key)
    }

    pub fn get<T: Identifyable<TS::Id>>(&self) -> Option<T> {
        self.resolve::<Registered<T>>()
    }

    pub fn get_all<T: Identifyable<TS::Id>>(&self) -> ServiceIterator<T, TS> {
        self.resolve::<AllRegistered<T>>()
    }
}

impl<TS: Strategy> Clone for WeakServiceProvider<TS> {
    fn clone(&self) -> Self {
        Self(ServiceProvider::<TS> {
            immutable_state: self.0.immutable_state.clone(),
            service_states: self.0.service_states.clone(),
            #[cfg(debug_assertions)]
            is_root: false,
        })
    }
}

impl<'a, TS: Strategy> From<&'a ServiceProvider<TS>> for WeakServiceProvider<TS> {
    fn from(provider: &'a ServiceProvider<TS>) -> Self {
        WeakServiceProvider(ServiceProvider {
            immutable_state: provider.immutable_state.clone(),
            service_states: provider.service_states.clone(),
            #[cfg(debug_assertions)]
            is_root: false,
        })
    }
}

pub(crate) struct ServiceProviderImmutableState<TS: Strategy> {
    types: Vec<TS::Id>,
    producers: Vec<UntypedFn<TS>>,
    // Unsafe-Code, which generates UntypedFn from parent, relies on the fact that parent ServiceProvider outlives this state
    _parents: Vec<WeakServiceProvider<TS>>,
}

impl<TS: Strategy> ServiceProviderImmutableState<TS> {
    pub(crate) fn new(
        types: Vec<TS::Id>,
        producers: Vec<UntypedFn<TS>>,
        _parents: Vec<WeakServiceProvider<TS>>,
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
    base: Option<Box<dyn Any + Send + Sync>>,
    shared_services: Vec<OnceCell<TypeNamed<Arc<dyn Any + Send + Sync>>>>,
}

/// Type used to retrieve all instances `T` of a `ServiceProvider`.
/// Services are built just in time when calling `next()`
pub struct ServiceIterator<T, TS: Strategy = AnyStrategy> {
    next_pos: Option<usize>,
    provider: WeakServiceProvider<TS>,
    item_type: PhantomData<T>,
}

impl<T, TS: Strategy> ServiceIterator<T, TS> {
    pub(crate) fn new(provider: WeakServiceProvider<TS>, next_pos: Option<usize>) -> Self {
        Self {
            provider,
            item_type: PhantomData,
            next_pos,
        }
    }
}

impl<'a, TS: Strategy, T: Identifyable<TS::Id>> Iterator for ServiceIterator<T, TS> {
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

struct DanglingCheckerResult {
    pub remaining_references: usize,
    pub typename: &'static str,
}

impl Debug for DanglingCheckerResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Type: {} (remaining {})",
            self.typename, self.remaining_references
        )
    }
}