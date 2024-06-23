use super::super::AnyPtr;

#[repr(C)]
#[cfg_attr(feature = "stable_abi", derive(abi_stable::StableAbi))]
pub struct AutoFreePointer {
    dropper: extern "C" fn(outer_context: AnyPtr),
    context: AnyPtr,
}
unsafe impl Send for AutoFreePointer {}
unsafe impl Sync for AutoFreePointer {}

impl AutoFreePointer {
    pub fn new(context: AnyPtr, dropper: extern "C" fn(outer_context: AnyPtr)) -> Self {
        Self { context, dropper }
    }

    pub fn no_alloc(context: AnyPtr) -> Self {
        extern "C" fn dropper(_: AnyPtr) {}
        Self { dropper, context }
    }
    pub fn boxed<T>(input: T) -> Self {
        extern "C" fn dropper<T>(u: AnyPtr) {
            if !u.is_null() {
                drop(unsafe { Box::from_raw(u as *mut T) })
            }
        }
        Self {
            dropper: dropper::<T>,
            context: Box::into_raw(Box::new(input)) as AnyPtr,
        }
    }
    pub fn get_pointer(&self) -> AnyPtr {
        self.context
    }
}

impl Drop for AutoFreePointer {
    fn drop(&mut self) {
        (self.dropper)(self.context)
    }
}
