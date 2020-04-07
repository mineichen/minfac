use core::{
    any::{Any, TypeId}, 
    marker::PhantomData
};
use std::collections::HashMap;

mod playground;

struct Container<'a> {
    //any_factories: HashMap<TypeId, *const dyn Fn(i32)>,
    //ref_factories: HashMap<TypeId, *const dyn Fn(i32)>,

    data: Vec<RefResolvable<'a, i32>>,
    //ctx: PhantomData<&'a i32>
}

impl<'a> Container<'a> {
    fn new() -> Self {
        Self {
            data: vec!()
        }
    }
    fn add<TNext: FnOnce(Self)>(mut self, data: RefResolvable<'a, i32>, next: TNext) {
        self.data.push(data);
        next(self);
    }

    fn try_resolve<TFn: Fn(&i32)>(&self, callback: TFn) {
        (self.data[0].factory)(&callback);
    }
}

trait Resolvable<T> {

}


struct RefResolvable<'a, T> {
    factory: &'a dyn Fn(&dyn Fn(&T))
}

impl<'a, T> RefResolvable<'a, T> {
    fn new(factory: &'a dyn Fn(&dyn Fn(&T))) -> Self {
        Self {
            factory
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[test]
    fn insert_fn() {
        
        let mut container = Container::new(); 
        add_to_container(container, |w| {
            w.try_resolve(|r| {
                println!("Results in {}", r);
            });
        });
    }

    fn add_to_container<TNext: FnOnce(Container)>(container: Container, next: TNext) {
        let outer: RefCell<Option<i32>> = RefCell::new(None);
        container.add(
            RefResolvable::new(&|v| { 
                v(&outer.borrow_mut().get_or_insert(42)); 
            }),
            next
        );
    }
}