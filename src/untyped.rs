use {
    crate::{Registered, ServiceProvider},
    alloc::boxed::Box,
    core::any::{Any, TypeId},
};

#[derive(Clone)]
pub struct UntypedFn {
    result_type_id: TypeId,
    pointer: *mut dyn Fn(),
    wrapper_creator: unsafe fn(*const UntypedFn, *const ServiceProvider) -> UntypedFn,
}

impl UntypedFn {
    pub fn get_result_type_id(&self) -> &TypeId {
        &self.result_type_id
    }

    // Unsafe constraint: Must be called with the same T as it was created
    pub unsafe fn borrow_for<T: Any>(&self) -> &dyn Fn(&ServiceProvider) -> T {
        debug_assert_eq!(TypeId::of::<Registered<T>>(), self.result_type_id);
        &*(self.pointer as *const dyn Fn(&ServiceProvider) -> T)
    }

    /// Creates a UntypedFn which ignores it's passed ServiceProvider and always uses the one it's bound to
    /// Unsafe constraint: `&self` and the value behind `&ServiceProvider` must live longer than the
    /// returned UntypedFn
    pub unsafe fn bind(&self, provider: *const ServiceProvider) -> Self {
        (self.wrapper_creator)(self, provider)
    }
}

impl<T: Any> From<Box<dyn Fn(&ServiceProvider) -> T>> for UntypedFn
where
    T: Any,
{
    fn from(factory: Box<dyn Fn(&ServiceProvider) -> T>) -> Self {
        UntypedFn {
            result_type_id: core::any::TypeId::of::<Registered<T>>(),
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

#[derive(Clone)]
pub struct UntypedPointer {
    #[cfg(debug_assertions)]
    debug_type: TypeId,
    pointer: *mut (),
    destroyer: fn(*mut ()),
}

impl UntypedPointer {
    pub fn new<T: Any>(data: T) -> Self {
        Self {
            #[cfg(debug_assertions)]
            debug_type: TypeId::of::<T>(),
            pointer: Box::into_raw(Box::new(data)) as *mut (),
            destroyer: |x| unsafe { drop(Box::from_raw(x as *mut T)) },
        }
    }

    pub unsafe fn borrow_as<T>(&self) -> &T {
        &*(self.pointer as *mut T)
    }
}

impl Default for UntypedPointer {
    fn default() -> Self {
        Self {
            #[cfg(debug_assertions)]
            debug_type: TypeId::of::<()>(),
            pointer: core::ptr::null_mut(),
            destroyer: |_| {},
        }
    }
}

impl Drop for UntypedPointer {
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            (self.destroyer)(self.pointer)
        }
    }
}
