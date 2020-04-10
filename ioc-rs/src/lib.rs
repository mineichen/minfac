use core::{
    any::{Any, TypeId},
    marker::PhantomData
};
use std::collections::HashMap;

mod playground;

pub struct Container<'a> {
    //ref_factories: HashMap<TypeId, *const dyn Fn(i32)>,

    data: HashMap<TypeId, *const RefResolver<'a, i32>>,
    phantom: PhantomData<&'a i32>
}

pub trait Resolvable{
    type inner: Any;
    const is_ref: bool;
}

struct Ref<T>(T);
struct Val<T>(T);

impl<T: Any> Resolvable for Ref<T> {
    type inner = T;
    const is_ref: bool = true;
}

impl<T: Any> Resolvable for Val<T> {
    type inner = T;
    const is_ref: bool = false;
}

impl<'a> Container<'a> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            phantom: PhantomData
        }
    }
    pub fn add<TNext: FnOnce(Self)>(mut self, data: &'a RefResolver<'a, i32>, next: TNext) {
        self.data.insert(TypeId::of::<i32>(), data);
        next(self);
    }

    pub fn try_resolve<TFn: Fn(&i32)>(&self, callback: TFn) {
        //(self.data.get(&TypeId::of::<i32>()).unwrap().factory)(&callback);
        self.call_with_resolver::<Ref<i32>, _>(|a| {
            a.by_ref(&|myRef| { 
                callback(myRef); 
            })
        })
    }

    pub fn call_with_resolver<T: Resolvable, TFn: Fn(&dyn Resolver<i32>)>(&self, callback: TFn) {
        if let Some(tmp) = self.data.get(&TypeId::of::<i32>()) {
            let reference = *tmp as *const RefResolver<'a, i32>;
            callback(&unsafe{ &*reference });
        }
    }
}

pub trait Resolver<T> {
    fn by_ref(&self, callback: &dyn Fn(&T));
}

pub struct RefResolver<'a, T> {
    factory: &'a dyn Fn(&dyn Fn(&T))
}

impl<'a, T> RefResolver<'a, T> {
    pub fn new(factory: &'a dyn Fn(&dyn Fn(&T))) -> Self {
        Self { factory }
    }
}
impl<'a, T> Resolver<T> for &'a RefResolver<'a, T> {
    fn by_ref(&self, callback: &dyn Fn(&T)) {
        (self.factory)(callback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[test]
    fn insert_fn() {
        let mut modified = RefCell::new(false);
        let mut container = Container::new(); 
        add_to_container(container, |w| {
            w.try_resolve(|r| {
                *modified.borrow_mut() = true;
                assert!(*r == 42);
            });
        });
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    fn add_to_container<TNext: FnOnce(Container)>(mut container: Container, next: TNext) {
        let outer: RefCell<Option<i32>> = RefCell::new(None);
        container.add(
            &RefResolver::new(&|v| { 
                v(&outer.borrow_mut().get_or_insert(42)); 
            }),
            next
        );
    }
}
 /*
            match self.factories.get(&TResult::get_typeid()) {
                Some(u) => {
                    let v = *u as *const dyn Fn(TResult::Dependency, &dyn Fn(TResult));
                    Some(unsafe {&*v})
                },
                None => None
            }*/
        
        
            /*
            self.factories.insert(
                TResult::get_typeid(), 
                factory as *const dyn Fn(TResult::Dependency, &dyn Fn(TResult)) as *const dyn Fn(i32)
            );*/