use {
    core::{
        any::{Any, TypeId},
    },
    crate::{ServiceProvider, Dynamic}
};

pub type UntypedFnFactory = Box<dyn Fn(&mut usize) -> UntypedFn>;

pub struct UntypedFn {
    result_type_id: TypeId,
    pointer: *mut dyn Fn(),
}

impl UntypedFn {
    pub fn get_result_type_id(&self) -> &TypeId {
        &self.result_type_id
    }
    pub unsafe fn borrow_for<T>(&self) -> &dyn Fn(&ServiceProvider) -> T{
        let func_ptr = self.pointer as *const dyn Fn(&ServiceProvider) -> T;
        &*func_ptr
    }
}

impl<T: Any> From<Box<dyn Fn(&ServiceProvider) -> T>> for UntypedFn
where
    T: Any,
{
    fn from(factory: Box<dyn Fn(&ServiceProvider) -> T>) -> Self {
        UntypedFn {
            result_type_id: core::any::TypeId::of::<Dynamic<T>>(),
            pointer: Box::into_raw(factory) as *mut dyn Fn(),
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
    pointer: usize,
    destroyer: fn(usize),
}

impl UntypedPointer {
    pub fn new<T>(data: T) -> Self {
        Self {
            pointer: Box::into_raw(Box::new(data)) as usize,
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
            pointer: 0,
            destroyer: |_| {},
        }
    }
}

impl Drop for UntypedPointer {
    fn drop(&mut self) {
        if self.pointer != 0 {
            (self.destroyer)(self.pointer)
        }
    }
}