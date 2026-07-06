mod erased_ptr;

use core::mem::ManuallyDrop;

use alloc::sync::Arc;

use self::erased_ptr::{ErasedPtr, FatAutoFreePointer};

#[repr(C)]
pub(crate) struct ArcAutoFreePointer {
    inner: FatAutoFreePointer,
    downgrade_ptr: extern "C" fn(ErasedPtr) -> WeakInfo,
}

impl<T: Send + Sync + ?Sized> From<Arc<T>> for ArcAutoFreePointer {
    fn from(value: Arc<T>) -> Self {
        extern "C" fn dropper<T: ?Sized>(i: ErasedPtr) {
            drop(unsafe { Arc::from_raw(i.into_ptr::<T>()) });
        }

        extern "C" fn downgrade<T: Send + Sync + ?Sized>(i: ErasedPtr) -> WeakInfo {
            extern "C" fn drop_weak<T: ?Sized>(i: ErasedPtr) {
                drop(unsafe { alloc::sync::Weak::from_raw(i.into_ptr::<T>()) })
            }

            extern "C" fn strong_count_on_weak<T: ?Sized>(i: ErasedPtr) -> usize {
                let weak = unsafe { alloc::sync::Weak::from_raw(i.into_ptr::<T>()) };
                let r = weak.strong_count();
                let _ = weak.into_raw();
                r
            }
            let arc = unsafe { Arc::from_raw(i.into_ptr::<T>()) };
            let weak = Arc::downgrade(&arc);
            let _ = Arc::into_raw(arc);
            WeakInfo {
                inner: FatAutoFreePointer::new(weak.into_raw(), drop_weak::<T>),
                weak_ptr: strong_count_on_weak::<T>,
            }
        }

        Self {
            inner: FatAutoFreePointer::new(Arc::into_raw(value), dropper::<T>),
            downgrade_ptr: downgrade::<T>,
        }
    }
}

#[allow(private_bounds)]
pub trait FromArcAutoFreePointer: Into<ArcAutoFreePointer> {
    #[allow(private_interfaces)]
    unsafe fn from_ref(value: &ArcAutoFreePointer) -> Self;
}

impl<T: Send + Sync + ?Sized> FromArcAutoFreePointer for Arc<T> {
    #[allow(private_interfaces)]
    unsafe fn from_ref(value: &ArcAutoFreePointer) -> Self {
        let arc =
            unsafe { ManuallyDrop::new(Arc::from_raw(value.inner.get_pointer().into_ptr::<T>())) };
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
    weak_ptr: extern "C" fn(ErasedPtr) -> usize,
    inner: FatAutoFreePointer,
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
