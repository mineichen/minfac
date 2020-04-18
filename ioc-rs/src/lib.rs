use core::{
    any::{Any, TypeId},
    marker::PhantomData
};
use std::collections::HashMap;

//mod playground;
pub mod ref_list;

pub struct Container<'a> {
    // bool ist just a placeholder for the type to be resolved
    data: HashMap<TypeId, *const DynamicResolver<'a, bool>>
}

pub trait Resolvable{
    type Result;
    fn resolve(container: &Container, callback: &dyn Fn(&Self::Result));
}

impl Resolvable for () {
    type Result = ();
    fn resolve(_: &Container, callback: &dyn Fn(&Self::Result)) {
        callback(&());
    }
}

pub struct RefTuple<T>(*const T);
impl<T> RefTuple<T> {
    pub fn i0(&self) -> &T { 
        unsafe {&*self.0}
    }
}

impl<T1: Resolvable> Resolvable for (T1,) {
    type Result = RefTuple<T1::Result>;
    fn resolve(container: &Container, callback: &dyn Fn(&Self::Result)) {
        T1::resolve(container,&|t1| {
            callback(&RefTuple(t1 as *const T1::Result) );
        });
    }
}

pub struct Dynamic<T> {
    phantom: PhantomData<T>
}

impl<T: Any> Resolvable for Dynamic<T> {
    type Result = T;
    fn resolve(container: &Container, callback: &dyn Fn(&Self::Result)) {
        if let Some(tmp) = container.data.get(&TypeId::of::<T>()) {
            let reference = *tmp as *const DynamicResolver<T>;
            unsafe{ &*reference }.resolve(&|value| {
                (callback)(value);
            });
        }
    }
}

impl<'a> Container<'a> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new()
        }
    }

    pub fn add<T: Any, TNext: FnOnce(Self)>(mut self, data: &'a DynamicResolver<'a, T>, next: TNext) {
        self.data.insert(
            TypeId::of::<T>(), 
            data as *const DynamicResolver<T> as *const DynamicResolver<bool>
        );
        next(self);
    }

    pub fn try_resolve<T: Resolvable>(&'a self, callback: &dyn Fn(&T::Result)) {
        T::resolve(&self, callback);
    }
}

pub struct DynamicResolver<'a, T> {
    factory: &'a dyn Fn(&(), &dyn Fn(&T))
}

impl<'a, T> DynamicResolver<'a, T> {
    pub fn new(factory: &'a dyn Fn(&(), &dyn Fn(&T))) -> Self {
        Self { factory }
    }
}

trait DynamicResolverTrait<'a, T> {
    fn resolve(&self, callback: &dyn Fn(&T));
}

impl<'a, T: Any> DynamicResolverTrait<'a, T> for DynamicResolver<'a, T> {
    fn resolve(&self, callback: &dyn Fn(&T)) {
        (self.factory)(&(), callback);
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
    fn resolve_empty_tuple() {
        let modified = RefCell::new(false);
        Container::new().try_resolve::<()>(&|t: &()| {
            *modified.borrow_mut() = true;
        });
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    #[test]
    fn resolve_single_struct() {
        let modified = RefCell::new(false);
        add_to_container(Container::new(), |container| {
            container.try_resolve::<(Dynamic<TestService>,)>(&|t| {
                *modified.borrow_mut() = true;
            });
        });
        
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    #[test]
    fn resolve_dynamic_twice_results() {
        let modified = RefCell::new(false);
        let stack = &modified as *const RefCell<bool> as usize;
        add_to_container(Container::new(), |w| {
             w.try_resolve::<Dynamic::<Instant>>(&|first: &Instant| {
                w.try_resolve::<Dynamic::<Instant>>( &|second: &Instant| {
                    *modified.borrow_mut() = true;
                    println!("Stacksize: {}", stack - second as *const Instant as usize);
                    assert!(*first == *second);
                });
            });
        });
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    fn add_to_container<TNext: Fn(Container)>(container: Container, next: TNext) {
        // OnceCell would be much more appropriate, because RefCell fails at runtime 
        // (e.g. get_or_insert() fails the second time because a immutable reference exists, even though it wouldn't change the data twice)
        let outer: RefCell<Option<Instant>> = RefCell::new(None);
        
        container.add(
            &DynamicResolver::new(&move|(), resolve| {       
                if outer.borrow().is_none()  {
                    *outer.borrow_mut() = Some(Instant::now());
                    thread::sleep(Duration::from_millis(10));  
                }        
                
                resolve(outer.borrow().as_ref().unwrap());
            }),
            
            move|c| {
                c.add(&DynamicResolver::new(&move|(), resolve| {
                    let a = &10;
                    let b: &i32 = &42;
                    
                    resolve(&TestService{ a, b });
                }), next)
            }
        );  
    }

    struct TestService<'a> {
        a: &'a i32,
        b: &'a i32
    }
}
