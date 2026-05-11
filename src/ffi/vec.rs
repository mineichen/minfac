use std::{
    mem::{ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut},
};

#[repr(C)]
pub(crate) struct FfiVec<T: 'static> {
    ptr: *mut T,
    length: usize,
    capacity: usize,
    vtable: &'static FfiVecVtable<T>,
}

#[repr(C)]
struct FfiVecVtable<T: 'static> {
    push: extern "C" fn(&mut FfiVec<T>, T),
    drop: extern "C" fn(&mut FfiVec<T>),
    drop_uninit: extern "C" fn(&mut FfiVec<T>),
}

unsafe impl<T: Send> Send for FfiVec<T> {}
unsafe impl<T: Sync> Sync for FfiVec<T> {}

impl<T: 'static> IntoIterator for FfiVec<T> {
    type Item = T;
    type IntoIter = FfiVecIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        let ptr = self.ptr;
        let last = unsafe { self.ptr.add(self.length) };

        Self::IntoIter {
            last,
            ptr,
            original: self,
        }
    }
}

impl<T> FfiVec<T> {
    pub fn new() -> Self {
        Self::from(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::from(Vec::with_capacity(capacity))
    }

    pub fn push(&mut self, value: T) {
        (self.vtable.push)(self, value)
    }
}

#[repr(C)]
pub(crate) struct FfiVecIntoIter<T: 'static> {
    original: FfiVec<T>,
    last: *mut T,
    ptr: *mut T,
}

impl<T> Drop for FfiVecIntoIter<T> {
    fn drop(&mut self) {
        while let Some(_) = self.next() {}
        (self.original.vtable.drop_uninit)(&mut self.original);
        self.original.ptr = std::ptr::null_mut();
    }
}

impl<T: 'static> Iterator for FfiVecIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        (self.ptr != self.last).then(|| unsafe {
            let r = self.ptr.read();
            self.ptr = self.ptr.add(1);
            r
        })
    }
}

impl<T> Drop for FfiVec<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            (self.vtable.drop)(self)
        }
    }
}

impl<T> From<Vec<T>> for FfiVec<T> {
    fn from(mut value: Vec<T>) -> Self {
        let (ptr, length, capacity) = (value.as_mut_ptr(), value.len(), value.capacity());
        std::mem::forget(value); // Will be cleaned up by FfiVec::drop or FfiVecIter::drop
        let vtable = VecFactory::VTABLE;
        Self {
            ptr,
            length,
            capacity,
            vtable,
        }
    }
}

extern "C" fn push_ffi<T>(that: &mut FfiVec<T>, value: T) {
    let mut vec =
        ManuallyDrop::new(unsafe { Vec::from_raw_parts(that.ptr, that.length, that.capacity) });
    vec.push(value);
    that.ptr = vec.as_mut_ptr();
    that.capacity = vec.capacity();
    that.length = vec.len();
}

extern "C" fn drop_ffi<T>(that: &mut FfiVec<T>) {
    unsafe { Vec::from_raw_parts(that.ptr, that.length, that.capacity) };
}
extern "C" fn drop_uninit_ffi<T>(that: &mut FfiVec<T>) {
    unsafe {
        Vec::from_raw_parts(
            that.ptr.cast::<MaybeUninit<T>>(),
            that.length,
            that.capacity,
        )
    };
}

trait Factory<T: 'static> {
    const VTABLE: &'static FfiVecVtable<T>;
}

struct VecFactory;

impl<T: 'static> Factory<T> for VecFactory {
    const VTABLE: &'static FfiVecVtable<T> = &FfiVecVtable {
        push: push_ffi::<T>,
        drop: drop_ffi::<T>,
        drop_uninit: drop_uninit_ffi::<T>,
    };
}

impl<TItem> FromIterator<TItem> for FfiVec<TItem> {
    fn from_iter<T: IntoIterator<Item = TItem>>(iter: T) -> Self {
        iter.into_iter().collect::<Vec<_>>().into()
    }
}

impl<T> Deref for FfiVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { alloc::slice::from_raw_parts(self.ptr, self.length) }
    }
}

impl<T> DerefMut for FfiVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { alloc::slice::from_raw_parts_mut(self.ptr, self.length) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_iter() {
        let mut value = FfiVec::new();
        value.push("first".to_string());
        value.push("second".to_string());

        let value_target = value.into_iter().collect::<FfiVec<_>>();

        assert_eq!(
            vec!("first", "second"),
            value_target.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn into_iter_and_from_iter_roundtrip() {
        let original: Vec<String> = vec!["first".into(), "second".into()];
        let value = original.iter().cloned().collect::<FfiVec<String>>();
        let result = value.into_iter().collect::<Vec<String>>();
        assert_eq!(original, result);
    }

    #[test]
    fn partially_consume_into_iter() {
        let original: Vec<String> = vec!["first".into(), "second".into()];
        let value = original.iter().cloned().collect::<FfiVec<String>>();
        assert_eq!(Some("first".to_string()), value.into_iter().next());
    }
}
