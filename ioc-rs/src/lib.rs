use core::{
    any::{Any, TypeId},
    marker::PhantomData
};
use std::collections::HashMap;

pub mod builder;
pub mod ref_list;

pub struct Container<'a> {
    // bool ist just a placeholder for the type to be resolved
    data: HashMap<TypeId, *const DynamicResolver<'a, bool>>
}

pub trait Resolvable {
    type Result; 
    fn resolve<TFn: Fn(&Self::Result)>(container: &Container, callback: TFn);
}

impl Resolvable for () {
    type Result = ();
    fn resolve<TFn: Fn(&Self::Result)>(_: &Container, callback: TFn) {
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
    fn resolve<TFn: Fn(&Self::Result)>(container: &Container, callback: TFn) {
        T1::resolve(container,|t1| {
            callback(&RefTuple(t1 as *const T1::Result) );
        });
    }
}

pub struct Dynamic<T: Any> {
    phantom: PhantomData<T>
}

impl<T: Any> Resolvable for Dynamic<T> {
    type Result = T;
    fn resolve<TFn: Fn(&Self::Result)>(container: &Container, callback: TFn) {
        if let Some(tmp) = container.data.get(&TypeId::of::<T>()) {
            let reference = *tmp as *const DynamicResolver<T>;
            (unsafe{ &*reference }.factory)(container, &|value| {
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

     fn add<TDependency, T, TFactory, TNext>(mut self, factory: TFactory , next: TNext) 
        where TDependency: Resolvable, 
            T: Any, 
            TNext: FnOnce(Self), 
            TFactory: Fn(&TDependency::Result, &dyn Fn(&T))
    {
        self.data.insert(
            TypeId::of::<T>(),
            &DynamicResolver::new(
                &|c: &Container, cb: &dyn Fn(&T)| { 
                    TDependency::resolve(c, |c| {
                        factory(c, cb)
                    });
                }
            ) as *const DynamicResolver<T> as *const DynamicResolver<bool>);
        next(self);
    }

    pub fn try_resolve<T: Resolvable>(&'a self, callback: &dyn Fn(&T::Result)) {
        T::resolve(&self, callback);
    }
}

pub struct DynamicResolver<'a, T> {
    factory: &'a dyn Fn(&Container, &dyn Fn(&T))
}

impl<'a, T> DynamicResolver<'a, T> {
    pub fn new(
        factory: &'a dyn Fn(&Container, &dyn Fn(&T))
    ) -> Self {
        Self { factory}
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
        Container::new().try_resolve::<()>(&|_| {
            *modified.borrow_mut() = true;
        });
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    #[test]
    fn resolve_single_struct() {
        let modified = RefCell::new(false);
        get_definitions( |container| {
            container.try_resolve::<(Dynamic<TestService>,)>(&|t| {
                assert_eq!(*t.i0().a, 42);
                *modified.borrow_mut() = true;
            });
        }).append_to(Container::new());
        
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    #[test]
    fn resolve_dynamic_twice_results() {
        let modified = RefCell::new(false);
        let stack = &modified as *const RefCell<bool> as usize;
        get_definitions(|w| {
            w.try_resolve::<Dynamic::<Instant>>(&|first| {
                w.try_resolve::<Dynamic::<Instant>>( &|second| {
                    *modified.borrow_mut() = true;
                    println!("Stacksize: {}", stack - second as *const Instant as usize);
                    assert!(*first == *second);
                });
            });
        }).append_to(Container::new());
       
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    #[test]
    fn resolve_dynamic_with_dependency() {
        let modified = RefCell::new(false);
        builder::ResolvableBuilder::new(|c| { 
                c.try_resolve::<Dynamic<f32>>(&|number| {
                    *modified.borrow_mut() = true;
                    assert_eq!(63f32, *number, "42 * 1.5 = 63");
                });
            })    
            .add(|resolve| resolve(&42))
            .with_dependency::<Dynamic<i32>>().add(|dep, resolve| resolve(&(1.5 * (*dep as f32))))
            .append_to(Container::new());

        
        let was_resolved = *modified.borrow();
        assert!(was_resolved);
    }

    fn get_definitions<TNext: FnOnce(Container)>(next: TNext) -> builder::ResolvableBuilder<impl FnOnce(Container<'_>)> {
        // OnceCell would be much more appropriate, because RefCell fails at runtime 
        // (e.g. get_or_insert() fails the second time because a immutable reference exists, even though it wouldn't change the data twice)
        let outer: RefCell<Option<Instant>> = RefCell::new(None);
        
        builder::ResolvableBuilder::new(next)
            .add(|resolve| { resolve(&TestService {a: &42, b: &10})})
            .add(move |resolve| {
                if outer.borrow().is_none()  {
                    *outer.borrow_mut() = Some(Instant::now());
                    thread::sleep(Duration::from_millis(10));  
                }        
                
                resolve(outer.borrow().as_ref().unwrap());
            })
    }

    pub struct TestService<'a> {
        pub a: &'a i32,
        pub b: &'a i32
    }
}
