use core::{marker::PhantomData, ops::Deref};

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct FfiStr<'a> {
    ptr: *const u8,
    len: usize,
    phantom: PhantomData<&'a ()>,
}

unsafe impl<'a> Send for FfiStr<'a> {}
unsafe impl<'a> Sync for FfiStr<'a> {}

impl<'a> core::fmt::Debug for FfiStr<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.deref(), f)
    }
}
impl<'a> core::fmt::Display for FfiStr<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.deref(), f)
    }
}

impl<'a> From<&'a str> for FfiStr<'a> {
    fn from(value: &'a str) -> Self {
        let bytes = value.as_bytes();
        Self {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
            phantom: PhantomData,
        }
    }
}

impl<'a> From<FfiStr<'a>> for &'a str {
    fn from(value: FfiStr<'a>) -> Self {
        unsafe {
            let bytes = core::slice::from_raw_parts(value.ptr, value.len);
            core::str::from_utf8_unchecked(bytes)
        }
    }
}

impl<'a> Deref for FfiStr<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        (*self).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deref_through_str() {
        let x = "foobar";
        let ffi = FfiStr::from(x);
        assert_eq!(*ffi, *x);
    }
}
