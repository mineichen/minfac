use core::mem::ManuallyDrop;

use alloc::sync::Arc;

use super::{super::AnyPtr, AutoFreePointer};

#[repr(C)]
pub(crate) struct ArcAutoFreePointer {
    inner: AutoFreePointer,
    downgrade_ptr: extern "C" fn(AnyPtr) -> WeakInfo,
}

impl<T: Send + Sync> From<Arc<T>> for ArcAutoFreePointer {
    fn from(value: Arc<T>) -> Self {
        extern "C" fn dropper<T>(i: AnyPtr) {
            drop(unsafe { Arc::from_raw(i as *const T) });
        }

        extern "C" fn downgrade<T: Send + Sync>(i: AnyPtr) -> WeakInfo {
            extern "C" fn drop_weak<T>(i: AnyPtr) {
                drop(unsafe { alloc::sync::Weak::from_raw(i as *const T) })
            }

            extern "C" fn strong_count_on_weak<T>(i: AnyPtr) -> usize {
                let weak = unsafe { alloc::sync::Weak::from_raw(i as *const T) };
                let r = weak.strong_count();
                let _ = weak.into_raw();
                r
            }
            let arc = unsafe { Arc::from_raw(i as *const T) };
            let weak = Arc::downgrade(&arc);
            let _ = Arc::into_raw(arc);
            WeakInfo {
                inner: AutoFreePointer::new(weak.into_raw(), drop_weak::<T>),
                weak_ptr: strong_count_on_weak::<T>,
            }
        }

        Self {
            inner: AutoFreePointer::new(Arc::into_raw(value) as AnyPtr, dropper::<T>),
            downgrade_ptr: downgrade::<T>,
        }
    }
}

#[allow(private_bounds)]
pub trait FromArcAutoFreePointer: Into<ArcAutoFreePointer> {
    #[allow(private_interfaces)]
    unsafe fn from_ref(value: &ArcAutoFreePointer) -> Self;
}

impl<T: Send + Sync> FromArcAutoFreePointer for Arc<T> {
    #[allow(private_interfaces)]
    unsafe fn from_ref(value: &ArcAutoFreePointer) -> Self {
        let arc =
            unsafe { ManuallyDrop::new(Arc::from_raw(value.inner.get_pointer() as *const T)) };
        (*arc).clone()
    }
}

impl ArcAutoFreePointer {
    pub fn downgrade(&self) -> WeakInfo {
        (self.downgrade_ptr)(self.inner.get_pointer())
    }
}

#[repr(C)]
pub struct WeakInfo {
    weak_ptr: extern "C" fn(AnyPtr) -> usize,
    inner: AutoFreePointer,
}

impl WeakInfo {
    pub fn strong_count(&self) -> usize {
        (self.weak_ptr)(self.inner.get_pointer())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_drop() {
        let x = ArcAutoFreePointer::from(Arc::new(String::from("Test")));
        let cloned = unsafe { Arc::<String>::from_ref(&x) };
        assert_eq!(2, Arc::strong_count(&cloned));
        drop(x);
        assert_eq!(1, Arc::strong_count(&cloned));
    }
}
