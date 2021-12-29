use crate::{
    service_provider::ServiceProvider,
    strategy::{Identifyable, Strategy},
};
use alloc::boxed::Box;

#[derive(Clone)]
pub struct UntypedFn<TS: Strategy> {
    result_type_id: TS::Id, // Todo: Debug only
    pointer: *mut dyn Fn(),
    wrapper_creator: unsafe fn(*const UntypedFn<TS>, *const ServiceProvider<TS>) -> UntypedFn<TS>,
}

unsafe impl<TS: Strategy> Send for UntypedFn<TS> {}
unsafe impl<TS: Strategy> Sync for UntypedFn<TS> {}

impl<TS: Strategy> UntypedFn<TS> {
    // Todo: Debug only
    pub fn get_result_type_id(&self) -> &TS::Id {
        &self.result_type_id
    }

    // Unsafe constraint: Must be called with the same T as it was created
    pub unsafe fn borrow_for<T>(&self) -> &dyn Fn(&ServiceProvider<TS>) -> T {
        // debug_assert_eq!(TypeId::of::<T>(), self.result_type_id, "This is likely a bug in minfac itself. Please file a bug report to overcome this issue");
        &*(self.pointer as *const dyn Fn(&ServiceProvider<TS>) -> T)
    }

    /// Creates a UntypedFn which ignores it's passed ServiceProvider and always uses the one it's bound to
    /// Unsafe constraint: `&self` and the value behind `&ServiceProvider` must live longer than the
    /// returned UntypedFn
    pub unsafe fn bind(&self, provider: *const ServiceProvider<TS>) -> Self {
        (self.wrapper_creator)(self, provider)
    }
}

impl<TS: Strategy, T> From<Box<dyn Fn(&ServiceProvider<TS>) -> T>> for UntypedFn<TS>
where
    T: Identifyable<TS::Id>,
{
    fn from(factory: Box<dyn Fn(&ServiceProvider<TS>) -> T>) -> Self {
        UntypedFn {
            result_type_id: T::get_id(),
            pointer: Box::into_raw(factory) as *mut dyn Fn(),
            wrapper_creator: |inner, provider| {
                let factory: Box<dyn Fn(&ServiceProvider<TS>) -> T> =
                    Box::new(move |_| unsafe { ((&*inner).borrow_for::<T>())(&*provider) });
                factory.into()
            },
        }
    }
}

impl<TS: Strategy> Drop for UntypedFn<TS> {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.pointer) });
    }
}
