use core::{
    any::{Any, TypeId},
    marker::PhantomData
};
use std::collections::HashMap;

// mod playground;

pub struct Container<'a> {
    // bool ist just a placeholder
    data: HashMap<TypeId, *const RefResolver<'a, bool>>
}

pub trait Resolvable{
    type Inner: Any;
    fn resolve(&self, resolver: &dyn Resolver<Self::Inner>);
}

pub struct Ref<T, TFn: Fn(&T)> {
    callback: TFn,
    phantom: PhantomData<T>
}

impl<T, TFn: Fn(&T)> Ref<T, TFn> {
    fn new(callback: TFn) -> Self {
        Self {
            callback,
            phantom: PhantomData
        }
    }
}

pub struct Val<T>(T);

impl<'a, T: Any, TFn: Fn(&T)> Resolvable for Ref<T, TFn> {
    type Inner = T;
    fn resolve(&self, resolver: &dyn Resolver<Self::Inner>) {
        resolver.by_ref(&|value| {
            (self.callback)(value);
        });
    }
}

impl<T: Any> Resolvable for Val<T> {
    type Inner = T;
    fn resolve(&self, _resolver: &dyn Resolver<Self::Inner>) {

    }
}

impl<'a> Container<'a> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new()
        }
    }

    pub fn add_ref<T: Any, TNext: Fn(Self)>(mut self, data: &'a RefResolver<'a, T>, next: TNext) {
        self.data.insert(
            TypeId::of::<T>(), 
            data as *const RefResolver<T> as *const RefResolver<bool>
        );
        next(self);
    }

    pub fn try_resolve<T: Resolvable>(&self, resolvable: T) {
        self.call_with_resolver::<T>(&|a| {
            resolvable.resolve(a);
        })
    }

    pub fn call_with_resolver<T: Resolvable>(&self, callback: &dyn Fn(&dyn Resolver<T::Inner>)) {
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
            w.try_resolve(Ref::new(|r: &i32| {
                *modified.borrow_mut() = true;
                assert!(*r == 42);
            }));
        });
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    fn add_to_container<TNext: Fn(Container)>(container: Container, next: TNext) {
        let outer: RefCell<Option<i32>> = RefCell::new(None);
        container.add_ref::<i32, _>(
            &RefResolver::new(&|v| { 
                v(&outer.borrow_mut().get_or_insert(42)); 
            }),
            next
        );
    }
}
