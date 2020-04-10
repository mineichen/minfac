use core::{
    any::{Any, TypeId}
};
use std::collections::HashMap;

// mod playground;

pub struct Container<'a> {
    // bool ist just a placeholder
    data: HashMap<TypeId, *const RefResolver<'a, bool>>
}

pub trait Resolvable{
    type Inner: Any;
    const IS_REFERENCE: bool;
}

pub struct Ref<T>(T);
pub struct Val<T>(T);

impl<T: Any> Resolvable for Ref<T> {
    type Inner = T;
    const IS_REFERENCE: bool = true;
}

impl<T: Any> Resolvable for Val<T> {
    type Inner = T;
    const IS_REFERENCE: bool = false;
}

impl<'a> Container<'a> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new()
        }
    }
    pub fn add<T: Any, TNext: Fn(Self)>(mut self, data: &'a RefResolver<'a, T>, next: TNext) {
        self.data.insert(
            TypeId::of::<T>(), 
            data as *const RefResolver<T> as *const RefResolver<bool>
        );
        next(self);
    }

    pub fn try_resolve<T: Resolvable, TFn: Fn(&T::Inner)>(&self, callback: TFn) {
        self.call_with_resolver::<Ref<T::Inner>, _>(|a| {
            a.by_ref(&|value| { 
                callback(value); 
            })
        })
    }

    pub fn call_with_resolver<T: Resolvable, TFn: Fn(&dyn Resolver<T::Inner>)>(&self, callback: TFn) {
        if let Some(tmp) = self.data.get(&TypeId::of::<T::Inner>()) {
            let reference = *tmp as *const RefResolver<'a, T::Inner>;
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
        let modified = RefCell::new(false);
        add_to_container(Container::new(), |w| {
            w.try_resolve::<Ref<i32>, _>(|r| {
                *modified.borrow_mut() = true;
                assert!(*r == 42);
            });
        });
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    fn add_to_container<TNext: Fn(Container)>(container: Container, next: TNext) {
        let outer: RefCell<Option<i32>> = RefCell::new(None);
        container.add::<i32, _>(
            &RefResolver::new(&|v| { 
                v(&outer.borrow_mut().get_or_insert(42)); 
            }),
            next
        );
    }
}
