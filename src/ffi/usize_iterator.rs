#[repr(C)]
pub(crate) struct FfiUsizeIterator {
    data: *mut (),
    next: extern "C" fn(*mut ()) -> usize,
    dropper: extern "C" fn(*mut ()),
}

impl FfiUsizeIterator {
    pub fn from_iter<TIter: Iterator<Item = usize>>(value: TIter) -> Self {
        let boxed = Box::new(value);
        let data = Box::into_raw(boxed);

        extern "C" fn dropper<T>(u: *mut ()) {
            drop(unsafe { Box::from_raw(u as *mut T) })
        }

        extern "C" fn next<T: Iterator<Item = usize>>(u: *mut ()) -> T::Item {
            let iter = unsafe { &mut *(u as *mut T) };
            match iter.next() {
                Some(usize::MAX) => unimplemented!("Iterators with usize::MAX are not ffi safe"),
                Some(x) => x,
                None => usize::MAX,
            }
        }
        Self {
            data: data as *mut (),
            dropper: dropper::<TIter>,
            next: next::<TIter>,
        }
    }
}
impl Iterator for FfiUsizeIterator {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        // # Safety
        // This struct can only be constructed here, &mut assueres there is only one Instance
        let v = (self.next)(self.data);
        (v != usize::MAX).then_some(v)
    }
}

impl Drop for FfiUsizeIterator {
    fn drop(&mut self) {
        (self.dropper)(self.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_from_iter() {
        let iter = FfiUsizeIterator::from_iter(0..3);
        assert_eq!(vec!(0, 1, 2), iter.collect::<Vec<_>>());
    }
}
