use core::mem::ManuallyDrop;
use std::sync::Arc;

trait Factory<T: 'static> {
    const VTABLE: &'static ServiceBuilderVtable<T>;
}

struct ArcFactory;
impl<T: 'static> Factory<Arc<T>> for ArcFactory {
    const VTABLE: &'static ServiceBuilderVtable<Arc<T>> = {
        #[allow(
            improper_ctypes_definitions,
            reason = "Arc is not safe over FFI-Boundaries, but for single binary projects it's ok"
        )]
        unsafe extern "C" fn clone<T>(ptr: *const ()) -> Arc<T> {
            let x = ManuallyDrop::new(Arc::from_raw(ptr as *const T));
            (*x).clone()
        }

        unsafe extern "C" fn drop<T>(ptr: *const ()) {
            Arc::from_raw(ptr);
        }

        &ServiceBuilderVtable {
            clone: clone::<T>,
            drop: drop::<T>,
        }
    };
}

#[repr(C)]
struct ServiceBuilderVtable<T> {
    clone: unsafe extern "C" fn(this: *const ()) -> T,
    drop: unsafe extern "C" fn(this: *const ()),
}

#[repr(C)]
struct SharedServiceBuilder<T: 'static> {
    vtable: &'static ServiceBuilderVtable<T>,
    ptr: *const (),
}

impl<T> SharedServiceBuilder<T> {
    fn clone_inner(&self) -> T {
        unsafe { (self.vtable.clone)(self.ptr) }
    }
}

impl<T> From<Arc<T>> for SharedServiceBuilder<Arc<T>> {
    fn from(value: Arc<T>) -> Self {
        Self {
            vtable: <ArcFactory as Factory<Arc<T>>>::VTABLE,
            ptr: Arc::into_raw(value) as *const Arc<T> as *const (),
        }
    }
}

impl<T> Drop for SharedServiceBuilder<T> {
    fn drop(&mut self) {
        unsafe { (self.vtable.drop)(self.ptr) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_arc_builder() {
        let orig = Arc::new(vec![42i32]);
        let builder = SharedServiceBuilder::<Arc<Vec<i32>>>::from(orig.clone());
        assert_eq!(builder.clone_inner(), orig);
    }
}
