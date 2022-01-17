use crate::{
    service_provider::ServiceProvider,
    strategy::{Identifyable, Strategy},
};
use alloc::{boxed::Box, sync::Arc};

#[repr(C)]
pub struct UntypedFn<TS: Strategy + 'static> {
    result_type_id: TS::Id,
    factory_pointer: usize,
    context: AutoFreePointer,
    wrapper_creator:
        unsafe extern fn(*const UntypedFn<TS>, *const ServiceProvider<TS>) -> UntypedFn<TS>,
}

unsafe impl<TS: Strategy + 'static> Send for UntypedFn<TS> {}
unsafe impl<TS: Strategy + 'static> Sync for UntypedFn<TS> {}

impl<TS: Strategy + 'static> UntypedFn<TS> {
    pub fn create<T: Identifyable<TS::Id>>(
        creator: extern fn(&ServiceProvider<TS>, &AutoFreePointer) -> T,
        context: AutoFreePointer,
    ) -> Self {
        type InnerContext<TS> = (*const UntypedFn<TS>, *const ServiceProvider<TS>);
        unsafe extern fn wrapper_creator<T: Identifyable<TS::Id>, TS: Strategy + 'static>(
            inner: *const UntypedFn<TS>,
            provider: *const ServiceProvider<TS>,
        ) -> UntypedFn<TS> {
            extern fn new_factory<T: Identifyable<TS::Id>, TS: Strategy + 'static>(
                _ignored_provider: &ServiceProvider<TS>,
                context: &AutoFreePointer,
            ) -> T {
                unsafe {
                    let (inner, provider) = &*(context.get_pointer() as *mut InnerContext<TS>);
                    (&**inner).execute::<T>(&**provider)
                }
            }
            let inner: InnerContext<TS> = (inner, provider);
            UntypedFn::<TS>::create::<T>(new_factory, AutoFreePointer::boxed(inner))
        }
        UntypedFn {
            result_type_id: T::get_id(),
            context,
            factory_pointer: creator as usize,
            wrapper_creator: wrapper_creator::<T, TS>,
        }
    }
    pub fn get_result_type_id(&self) -> &TS::Id {
        &self.result_type_id
    }

    // Unsafe constraint: Must be called with the same T as it was created
    pub unsafe fn execute<T>(&self, provider: &ServiceProvider<TS>) -> T {
        let lambda: extern fn(&ServiceProvider<TS>, &AutoFreePointer) -> T =
            std::mem::transmute(self.factory_pointer);
        (lambda)(provider, &self.context)
    }

    /// Creates a UntypedFn which ignores it's passed ServiceProvider and always uses the one it's bound to
    /// Unsafe constraint: `&self` and the value behind `&ServiceProvider` must live longer than the
    /// returned UntypedFn
    pub unsafe fn bind(&self, provider: *const ServiceProvider<TS>) -> Self {
        (self.wrapper_creator)(self, provider)
    }
}

#[repr(C)]
#[cfg_attr(feature = "stable_abi", derive(abi_stable::StableAbi))]
pub struct AutoFreePointer {
    dropper: extern fn(outer_context: usize),
    context: usize,
}

impl AutoFreePointer {
    pub fn no_alloc(context: usize) -> Self {
        extern fn dropper(_: usize) {}
        Self { dropper, context }
    }
    pub fn boxed<T>(input: T) -> Self {
        extern fn dropper<T>(u: usize) {
            if u != 0 {
                drop(unsafe { Box::from_raw(u as *mut T) })
            }
        }
        Self {
            dropper: dropper::<T>,
            context: Box::into_raw(Box::new(input)) as usize,
        }
    }
    pub fn get_pointer(&self) -> usize {
        self.context
    }
}

impl Drop for AutoFreePointer {
    fn drop(&mut self) {
        (self.dropper)(self.context)
    }
}

pub struct ArcAutoFreePointer {
    inner: AutoFreePointer,
    downgrade_ptr: extern fn(usize) -> WeakInfo,
}

impl ArcAutoFreePointer {
    pub fn new<T>(i: Arc<T>) -> Self {
        extern fn dropper<T>(i: usize) {
            drop(unsafe { Arc::from_raw(i as *const T) });
        }

        extern fn downgrade<T>(i: usize) -> WeakInfo {
            extern fn drop_weak<T>(i: usize) {
                drop(unsafe { alloc::sync::Weak::from_raw(i as *const T) })
            }

            extern fn strong_count_on_weak<T>(i: usize) -> usize {
                let weak = unsafe { std::sync::Weak::from_raw(i as *const T) };
                let r = weak.strong_count();
                let _ = weak.into_raw();
                r
            }
            let arc = unsafe { Arc::from_raw(i as *const T) };
            let weak = Arc::downgrade(&arc);
            let _ = Arc::into_raw(arc);
            WeakInfo {
                inner: AutoFreePointer {
                    context: weak.into_raw() as usize,
                    dropper: drop_weak::<T>,
                },
                weak_ptr: strong_count_on_weak::<T>,
            }
        }

        Self {
            inner: AutoFreePointer {
                context: Arc::into_raw(i) as usize,
                dropper: dropper::<T>,
            },
            downgrade_ptr: downgrade::<T>,
        }
    }
    pub unsafe fn clone_inner<T>(&self) -> Arc<T> {
        let arc = Arc::from_raw(self.inner.context as *const T);
        let r = arc.clone();
        let _ = Arc::into_raw(arc);
        r
    }
    pub fn downgrade(&self) -> WeakInfo {
        (self.downgrade_ptr)(self.inner.context)
    }
}

#[repr(C)]
pub struct WeakInfo {
    weak_ptr: extern fn(usize) -> usize,
    inner: AutoFreePointer,
}

impl WeakInfo {
    pub fn strong_count(&self) -> usize {
        (self.weak_ptr)(self.inner.context)
    }
}
