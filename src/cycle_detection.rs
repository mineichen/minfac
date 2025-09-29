use abi_stable::{
    erased_types::interfaces::IteratorInterface,
    std_types::{RBox, RHashMap},
    DynTrait,
};

#[derive(Default)]
pub(crate) struct CycleChecker(RHashMap<usize, CycleCheckerValue>);

pub(crate) struct CycleCheckerValue {
    is_visited: bool,
    type_description: &'static str,
    iter: DynTrait<'static, RBox<()>, IteratorInterface<usize>>, // Use RVec
}

impl CycleChecker {
    pub fn register_cyclic_reference_candidate(
        &mut self,
        type_name: &'static str,
        dependencies: DynTrait<'static, RBox<()>, IteratorInterface<usize>>,
        service_descriptor_pos: usize,
    ) {
        self.0.insert(
            service_descriptor_pos,
            CycleCheckerValue {
                is_visited: false,
                type_description: type_name,
                iter: dependencies,
            },
        );
    }
    pub fn ok(mut self) -> Result<(), String> {
        self.ok_inner().map_err(|indices| {
            indices
                .into_iter()
                .skip(1)
                .map(|i| self.0.get(&i).unwrap().type_description)
                .fold(
                    self.0.values().next().unwrap().type_description.to_string(),
                    |acc, n| acc + " -> " + n,
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
