use alloc::{alloc::alloc, sync::Arc};

use super::{super::AnyPtr, AutoFreePointer};

pub struct ArcAutoFreePointer {
    inner: AutoFreePointer,
    downgrade_ptr: extern "C" fn(AnyPtr) -> WeakInfo,
}

impl ArcAutoFreePointer {
    pub fn new<T: Send + Sync>(i: Arc<T>) -> Self {
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
            inner: AutoFreePointer::new(Arc::into_raw(i) as AnyPtr, dropper::<T>),
            downgrade_ptr: downgrade::<T>,
        }
    }
    pub unsafe fn clone_inner<T>(&self) -> Arc<T> {
        let arc = Arc::from_raw(self.inner.get_pointer() as *const T);
        let r = arc.clone();
        let _ = Arc::into_raw(arc);
        r
    }
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
        let x = ArcAutoFreePointer::new(Arc::new(String::from("Test")));
        let cloned = unsafe { x.clone_inner::<String>() };
        assert_eq!(2, Arc::strong_count(&cloned));
        drop(x);
        assert_eq!(1, Arc::strong_count(&cloned));
    }
}
