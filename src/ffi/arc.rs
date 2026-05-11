use std::{ops::Deref, sync::Arc};

use crate::ffi::FfiResult;

#[repr(C)]
pub(crate) struct FfiArc<T: 'static> {
    ptr: *const T,
    vtable: &'static FfiArcVtable<T>,
}
unsafe impl<T: Send> Send for FfiArc<T> {}
unsafe impl<T: Sync> Sync for FfiArc<T> {}

impl<T: 'static> Clone for FfiArc<T> {
    fn clone(&self) -> Self {
        (self.vtable.increment_strong)(self.ptr);
        Self {
            ptr: self.ptr,
            vtable: self.vtable,
        }
    }
}

impl<T: 'static> FfiArc<T> {
    pub fn new(value: T) -> Self {
        Self::from(Arc::new(value))
    }

    /// Returns inner or the number of remaining references
    pub fn try_unwrap(self) -> Result<T, usize> {
        let r = (self.vtable.try_unwrap)(self);
        r.into()
    }
}

impl<T: 'static> From<Arc<T>> for FfiArc<T> {
    fn from(value: Arc<T>) -> Self {
        let ptr = Arc::into_raw(value);
        let vtable = ArcFactory::VTABLE;
        Self { ptr, vtable }
    }
}

impl<T> Deref for FfiArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

#[repr(C)]
struct FfiArcVtable<T: 'static> {
    drop: extern "C" fn(*const T),
    increment_strong: extern "C" fn(*const T),
    try_unwrap: extern "C" fn(FfiArc<T>) -> FfiResult<T, usize>,
}

extern "C" fn drop_ffi<T>(ptr: *const T) {
    unsafe { Arc::from_raw(ptr) };
}

extern "C" fn increment_strong_ffi<T>(ptr: *const T) {
    unsafe { Arc::increment_strong_count(ptr) };
}

extern "C" fn try_unwrap_ffi<T>(that: FfiArc<T>) -> FfiResult<T, usize> {
    let arc = unsafe { Arc::from_raw(that.ptr) };
    let r = Arc::try_unwrap(arc).map_err(|x| Arc::strong_count(&x));
    std::mem::forget(that);
    r.into()
}

trait Factory<T: 'static> {
    const VTABLE: &'static FfiArcVtable<T>;
}

struct ArcFactory;

impl<T: 'static> Factory<T> for ArcFactory {
    const VTABLE: &'static FfiArcVtable<T> = &FfiArcVtable {
        drop: drop_ffi::<T>,
        increment_strong: increment_strong_ffi::<T>,
        try_unwrap: try_unwrap_ffi::<T>,
    };
}

impl<T> Drop for FfiArc<T> {
    fn drop(&mut self) {
        (self.vtable.drop)(self.ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strong_count_for_cloned_equals_two() {
        let x1 = FfiArc::new(42);
        let x2 = x1.clone();
        assert_eq!(Err(2), x2.try_unwrap());
        assert_eq!(Ok(42), x1.try_unwrap());
    }
}
