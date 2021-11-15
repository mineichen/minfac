use {
    crate::ServiceProvider,
    alloc::boxed::Box,
    core::any::{Any, TypeId},
};

#[derive(Clone)]
pub struct UntypedFn {
    result_type_id: TypeId, // Todo: Debug only
    pointer: *mut dyn Fn(),
    wrapper_creator: unsafe fn(*const UntypedFn, *const ServiceProvider) -> UntypedFn,
}

unsafe impl Send for UntypedFn {}
unsafe impl Sync for UntypedFn {}

impl UntypedFn {
    // Todo: Debug only
    pub fn get_result_type_id(&self) -> &TypeId {
        &self.result_type_id
    }

    // Unsafe constraint: Must be called with the same T as it was created
    pub unsafe fn borrow_for<T: Any>(&self) -> &dyn Fn(&ServiceProvider) -> T {
        debug_assert_eq!(TypeId::of::<T>(), self.result_type_id, "This is likely a bug in minfac itself. Please file a bug report to overcome this issue");
        &*(self.pointer as *const dyn Fn(&ServiceProvider) -> T)
    }

    /// Creates a UntypedFn which ignores it's passed ServiceProvider and always uses the one it's bound to
    /// Unsafe constraint: `&self` and the value behind `&ServiceProvider` must live longer than the
    /// returned UntypedFn
    pub unsafe fn bind(&self, provider: *const ServiceProvider) -> Self {
        (self.wrapper_creator)(self, provider)
    }
}

impl<T> From<Box<dyn Fn(&ServiceProvider) -> T>> for UntypedFn
where
    T: Any,
{
    fn from(factory: Box<dyn Fn(&ServiceProvider) -> T>) -> Self {
        UntypedFn {
            result_type_id: core::any::TypeId::of::<T>(),
            pointer: Box::into_raw(factory) as *mut dyn Fn(),
            wrapper_creator: |inner, provider| {
                let factory: Box<dyn Fn(&ServiceProvider) -> T> =
                    Box::new(move |_| unsafe { ((&*inner).borrow_for::<T>())(&*provider) });
                factory.into()
            },
        }
    }
}

impl Drop for UntypedFn {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.pointer as *mut dyn Fn(&ServiceProvider)) });
    }
}
