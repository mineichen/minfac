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

pub struct ResolvableRef<T, TFn: Fn(&T)> {
    callback: TFn,
    phantom: PhantomData<T>
}

impl<T, TFn: Fn(&T)> ResolvableRef<T, TFn> {
    pub fn new(callback: TFn) -> Self {
        Self {
            callback,
            phantom: PhantomData
        }
    }
}

impl<'a, T: Any, TFn: Fn(&T)> Resolvable for ResolvableRef<T, TFn> {
    type Inner = T;
    fn resolve(&self, resolver: &dyn Resolver<Self::Inner>) {
        resolver.by_ref(&|value| {
            (self.callback)(value);
        });
    }
}

pub struct ResolvableVal<T, TFn: Fn(T)> {
    callback: TFn,
    phantom: PhantomData<T>
}

impl<T, TFn: Fn(T)> ResolvableVal<T, TFn> {
    pub fn new(callback: TFn) -> Self {
        Self {
            callback,
            phantom: PhantomData
        }
    }
}

impl<'a, T: Any, TFn: Fn(T)> Resolvable for ResolvableVal<T, TFn> {
    type Inner = T;
    fn resolve(&self, resolver: &dyn Resolver<Self::Inner>) {
        resolver.by_val(&|value| {
            (self.callback)(value);
        });
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
    fn by_val(&self, callback: &dyn Fn(T));
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
    fn by_val(&self, _callback: &dyn Fn(T)) {
        panic!("RefResolver doesn't support by_val");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{RefCell},
        time:: {
            Duration,
            Instant
        },
        thread
    };

    #[test]
    fn resolve_reference_twice_results() {
        let modified = RefCell::new(false);
        add_to_container(Container::new(), |w| {
            w.try_resolve(ResolvableRef::new(|first: &Instant| {
                w.try_resolve(ResolvableRef::new(|second: &Instant| {
                    *modified.borrow_mut() = true;
                    assert!(*first == *second);
                }));
            }));
        });
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    fn add_to_container<TNext: Fn(Container)>(container: Container, next: TNext) {
        // OnceCell would be much more appropriate, because RefCell fails at runtime 
        // (e.g. get_or_insert() fails the second time because a immutable reference exists, even though it wouldn't change the data twice)
        let outer: RefCell<Option<Instant>> = RefCell::new(None);
        
        container.add_ref(
            &RefResolver::new(&move|v| {       
                if outer.borrow().is_none()  {
                    *outer.borrow_mut() = Some(Instant::now());
                    thread::sleep(Duration::from_millis(10));  
                }        
                
                v(outer.borrow().as_ref().unwrap());
            }),
            next
        );
    }
}
