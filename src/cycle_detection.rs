use abi_stable::std_types::{RHashMap, RStr};

use crate::ffi::FfiUsizeIterator;

#[derive(Default)]
#[repr(C)]
pub(crate) struct CycleChecker(RHashMap<usize, CycleCheckerValue>);

#[repr(C)]
pub(crate) struct CycleCheckerValue {
    is_visited: bool,
    type_name: RStr<'static>,
    iter: FfiUsizeIterator,
}

impl CycleChecker {
    pub fn register_cyclic_reference_candidate(
        &mut self,
        type_name: &'static str,
        dependencies: FfiUsizeIterator,
        service_descriptor_pos: usize,
    ) {
        self.0.insert(
            service_descriptor_pos,
            CycleCheckerValue {
                is_visited: false,
                type_name: RStr::from_str(type_name),
                iter: dependencies,
            },
        );
    }
    pub fn ok(mut self) -> Result<(), String> {
        self.ok_inner().map_err(|indices| {
            indices
                .into_iter()
                .skip(1)
                .map(|i| self.0.get(&i).unwrap().type_name)
                .fold(
                    self.0.values().next().unwrap().type_name.to_string(),
                    |acc, n| acc + " -> " + n.as_str(),
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
