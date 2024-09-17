use crate::{
    service_provider::ServiceProvider,
    strategy::{Identifyable, Strategy},
    AnyPtr,
};

use super::AutoFreePointer;

#[repr(C)]
pub struct UntypedFn<TS: Strategy + 'static> {
    result_type_id: TS::Id,
    factory_pointer: AnyPtr,
    context: AutoFreePointer,
    wrapper_creator:
        unsafe extern "C" fn(*const UntypedFn<TS>, *const ServiceProvider<TS>) -> UntypedFn<TS>,
}

unsafe impl<TS: Strategy + 'static> Send for UntypedFn<TS> {}
unsafe impl<TS: Strategy + 'static> Sync for UntypedFn<TS> {}

impl<TS: Strategy + 'static> UntypedFn<TS> {
    pub fn create<T: Identifyable<TS::Id>>(
        creator: extern "C" fn(*const ServiceProvider<TS>, *const AutoFreePointer) -> T,
        context: AutoFreePointer,
    ) -> Self {
        type InnerContext<TS> = (*const UntypedFn<TS>, *const ServiceProvider<TS>);
        extern "C" fn wrapper_creator<T: Identifyable<TS::Id>, TS: Strategy + 'static>(
            inner: *const UntypedFn<TS>,
            provider: *const ServiceProvider<TS>,
        ) -> UntypedFn<TS> {
            extern "C" fn new_factory<T: Identifyable<TS::Id>, TS: Strategy + 'static>(
                _ignored_provider: *const ServiceProvider<TS>,
                context: *const AutoFreePointer,
            ) -> T {
                unsafe {
                    let (inner, provider) = &*((&*context as &AutoFreePointer).get_pointer()
                        as *const InnerContext<TS>);
                    (&**inner).execute::<T>(&**provider)
                }
            }
            let inner: InnerContext<TS> = (inner, provider);
            UntypedFn::<TS>::create::<T>(new_factory, AutoFreePointer::boxed(inner))
        }
        UntypedFn {
            result_type_id: T::get_id(),
            context,
            factory_pointer: creator as AnyPtr,
            wrapper_creator: wrapper_creator::<T, TS>,
        }
    }
    pub fn get_result_type_id(&self) -> &TS::Id {
        &self.result_type_id
    }

    // Unsafe constraint: Must be called with the same T as it was created
    pub unsafe fn execute<T>(&self, provider: &ServiceProvider<TS>) -> T {
        let lambda: extern "C" fn(&ServiceProvider<TS>, &AutoFreePointer) -> T =
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

#[cfg(test)]
mod tests {
    use crate::{AnyStrategy, ServiceCollection};

    use super::*;

    #[test]
    fn create_execute_and_drop() {
        extern "C" fn test<T: Identifyable<TS::Id> + Copy, TS: Strategy>(
            _provider: *const ServiceProvider<TS>,
            ctx: *const AutoFreePointer,
        ) -> T {
            unsafe {
                let dat = (&*ctx as &AutoFreePointer).get_pointer();
                *(dat as *const T)
            }
        }
        let ptr = AutoFreePointer::boxed(2i64);
        let x = UntypedFn::create(test::<i64, AnyStrategy>, ptr);
        let num = unsafe { x.execute::<i64>(&ServiceCollection::new().build().unwrap()) };
        assert_eq!(num, 2);
    }
}
