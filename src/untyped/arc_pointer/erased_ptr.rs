// taken from https://users.rust-lang.org/t/type-erasing-pointers-to-t-sized/96984/3
use core::mem::{size_of, MaybeUninit};

#[derive(Copy, Clone)]
#[repr(C)]
#[cfg_attr(feature = "stable_abi", derive(abi_stable::StableAbi))]
pub struct ErasedPtr {
    value: MaybeUninit<[usize; 2]>,
}

impl ErasedPtr {
    pub fn new<T: ?Sized>(ptr: *const T) -> Self {
        let mut res = ErasedPtr {
            value: MaybeUninit::zeroed(),
        };

        let len = size_of::<*const T>();
        assert!(len <= size_of::<[usize; 2]>());

        let ptr_val = (&ptr) as *const *const T as *const u8;
        let target = res.value.as_mut_ptr() as *mut u8;
        // SAFETY: The target is valid for at least `len` bytes, and has no
        // requirements on the value.
        unsafe {
            core::ptr::copy_nonoverlapping(ptr_val, target, len);
        }

        res
    }

    /// Convert the type erased pointer back into a pointer.
    ///
    /// # Safety
    ///
    /// The type `T` must be the same type as the one used with `new`.
    pub unsafe fn into_ptr<T: ?Sized>(&self) -> *const T {
        // SAFETY: The constructor ensures that the first `size_of::<T>()`
        // bytes of `&self.value` are a valid `*const T` pointer.
        unsafe { core::mem::transmute_copy(&self.value) }
    }
}

#[repr(C)]
#[cfg_attr(feature = "stable_abi", derive(abi_stable::StableAbi))]
pub struct FatAutoFreePointer {
    dropper: extern "C" fn(outer_context: ErasedPtr),
    context: ErasedPtr,
}
unsafe impl Send for FatAutoFreePointer {}
unsafe impl Sync for FatAutoFreePointer {}

impl FatAutoFreePointer {
    pub fn new<T: Send + Sync + ?Sized>(
        context: *const T,
        dropper: extern "C" fn(outer_context: ErasedPtr),
    ) -> Self {
        Self {
            context: ErasedPtr::new(context),
            dropper,
        }
    }

    pub fn get_pointer(&self) -> ErasedPtr {
        self.context
    }
}

impl Drop for FatAutoFreePointer {
    fn drop(&mut self) {
        (self.dropper)(self.context)
    }
}
