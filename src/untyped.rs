use {
    crate::{Registered, ServiceProvider},
    alloc::{boxed::Box, sync::Arc},
    core::any::{type_name, Any, TypeId},
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

type UntypedPointerChecker = Option<Box<dyn Fn() -> DanglingCheckerResult>>;
#[derive(Clone)]
pub struct UntypedPointer {
    #[cfg(debug_assertions)]
    debug_type: TypeId,
    pointer: *mut (),
    destroyer: fn(*mut ()),
    checker: fn(*mut ()) -> UntypedPointerChecker,
}

pub struct DanglingCheckerResult {
    pub remaining_references: usize,
    pub typename: &'static str,
}

impl core::fmt::Debug for DanglingCheckerResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Type: {} (remaining {})",
            self.typename, self.remaining_references
        )
    }
}

/// We need this structure, because at the time of writing (13.05.21),
/// Arc<dyn Any>::downcast<T> doesn't support T: ?Sized
impl UntypedPointer {
    pub fn new<T: Any + ?Sized>(data: Arc<T>) -> Self {
        Self {
            #[cfg(debug_assertions)]
            debug_type: TypeId::of::<Arc<T>>(),
            pointer: Box::into_raw(Box::new(data)) as *mut (),
            destroyer: |x| unsafe { drop(Box::from_raw(x as *mut Arc<T>)) },
            checker: |x| {
                let arc_ref: &Arc<T> = unsafe { &*(x as *mut Arc<T>) };
                let count = Arc::strong_count(arc_ref);
                if count > 1 {
                    let weak = Arc::downgrade(arc_ref);
                    Some(Box::new(move || DanglingCheckerResult {
                        remaining_references: weak.strong_count(),
                        typename: type_name::<T>(),
                    }))
                } else {
                    None
                }
            },
        }
    }

    pub unsafe fn clone_as<T: Clone>(&self) -> T {
        T::clone(&*(self.pointer as *mut T))
    }
    /// Returns a lambda which can be called even after the UntypedPointer is destroyed
    /// The checker is just created, if the strong_count > 1. Because this method is used in the desturctor of ServiceProvider,
    /// the pointer which is causing > 1 is held by the ServiceProvider itself.
    pub fn get_weak_checker_if_dangling(&self) -> Option<Box<dyn Fn() -> DanglingCheckerResult>> {
        (self.checker)(self.pointer)
    }
}

impl Default for UntypedPointer {
    fn default() -> Self {
        Self {
            #[cfg(debug_assertions)]
            debug_type: TypeId::of::<()>(),
            pointer: core::ptr::null_mut(),
            destroyer: |_| {},
            checker: |_| None,
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
