use alloc::collections::BTreeMap;
use core::{marker::PhantomData, ops::Deref};

use crate::ffi::{FfiStr, FfiUsizeIterator};

#[derive(Default)]
#[repr(C)]
pub(crate) struct CycleChecker(BTreeMap<usize, CycleCheckerValue>);

#[repr(C)]
pub(crate) struct CycleCheckerValue {
    is_visited: bool,
    type_name: FfiStr<'static>,
    iter: FfiUsizeIterator,
}

impl CycleChecker {
    pub fn create_inserter(&mut self) -> CycleDetectionInserter<'_> {
        extern "C" fn callback(
            ctx: *mut (),
            type_name: FfiStr<'static>,
            dependencies: FfiUsizeIterator,
            service_descriptor_pos: usize,
        ) {
            let ctx = unsafe { &mut *(ctx as *mut CycleChecker) };
            ctx.0.insert(
                service_descriptor_pos,
                CycleCheckerValue {
                    is_visited: false,
                    type_name,
                    iter: dependencies,
                },
            );
        }
        CycleDetectionInserter {
            ctx: self as *mut Self as *mut (),
            callback,
            phantom: PhantomData,
        }
    }
    pub fn ok(mut self) -> Result<(), String> {
        self.ok_inner().map_err(|indices| {
            indices
                .into_iter()
                .skip(1)
                .map(|i| self.0.get(&i).unwrap().type_name)
                .fold(
                    self.0.values().next().unwrap().type_name.to_string(),
                    |acc, n| acc + " -> " + n.deref(),
                )
        })
    }
    pub fn ok_inner(&mut self) -> Result<(), Vec<usize>> {
        let mut stack = Vec::new();
        let map = &mut self.0;

        loop {
            let pos = match map.keys().next() {
                Some(pos) => *pos,
                _ => break,
            };

            stack.push(pos);
            while let Some(current) = stack.last() {
                if let Some(value) = map.get_mut(current) {
                    if value.is_visited {
                        return Err(stack);
                    }
                    value.is_visited = true;
                    match value.iter.next() {
                        Some(x) => {
                            stack.push(x);
                            continue;
                        }
                        None => {
                            map.remove(current);
                        }
                    };
                }
                stack.pop();
                if let Some(parent) = stack.last() {
                    let state = map.get_mut(parent).unwrap();
                    state.is_visited = false;
                }
            }
        }
        Ok(())
    }
}

#[repr(C)]
pub(crate) struct CycleDetectionInserter<'a> {
    ctx: *mut (),
    callback: extern "C" fn(
        ctx: *mut (),
        type_name: FfiStr<'static>,
        dependencies: FfiUsizeIterator,
        service_descriptor_pos: usize,
    ),
    phantom: PhantomData<&'a ()>,
}

impl<'a> CycleDetectionInserter<'a> {
    pub fn insert(
        &mut self,
        type_name: FfiStr<'static>,
        dependencies: FfiUsizeIterator,
        service_descriptor_pos: usize,
    ) {
        (self.callback)(self.ctx, type_name, dependencies, service_descriptor_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert() {
        let mut checker = CycleChecker::default();
        let mut inserter = checker.create_inserter();
        inserter.insert("foo".into(), FfiUsizeIterator::from_iter(0..10), 42);

        checker.ok().unwrap();
    }
}
