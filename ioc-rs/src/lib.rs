use core::{
    any::{Any, TypeId},
    marker::PhantomData
};
use std::collections::HashMap;

//mod playground;
pub mod ref_list;

pub struct Container<'a> {
    // bool ist just a placeholder
    data: HashMap<TypeId, *const RefResolver<'a, bool>>
}

pub trait Resolvable{
    type Result: Any;
    fn resolve(container: &Container, callback: &dyn Fn(&Self::Result));
}

impl Resolvable for () {
    type Result = ();
    fn resolve(_: &Container, _callback: &dyn Fn(&Self::Result)) {
        ()
    }
}
/*
impl<T1: ResolvableT, T2> Resolvable for (T1, T2) {
    type Result = (T1, T2);
    fn resolve(&self, container: &Container) {

    }
}*/
pub struct Dynamic<T> {
    phantom: PhantomData<T>
}

impl<'a, T: Any> Resolvable for Dynamic<T> {
    type Result = T;
    fn resolve(container: &Container, callback: &dyn Fn(&Self::Result)) {
        if let Some(tmp) = container.data.get(&TypeId::of::<T>()) {
            let reference = *tmp as *const RefResolver<'a, T>;
            (unsafe{ &*reference }.factory)((), &|value| {
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

    pub fn add_ref<T: Any, TNext: Fn(Self)>(mut self, data: &'a RefResolver<'a, T>, next: TNext) {
        self.data.insert(
            TypeId::of::<T>(), 
            data as *const RefResolver<T> as *const RefResolver<bool>
        );
        next(self);
    }

    pub fn try_resolve<T: Resolvable>(&self, callback: &dyn Fn(&T::Result)) {
        T::resolve(&self, callback);
    }
/*
    pub fn get_resolver<T: Any>(&self) -> Option<*const dyn Resolver<T>> {
        //unimplemented!()
        
        self.data.get(&TypeId::of::<T>())
            .map(|tmp| {
                return *tmp as *const dyn Resolver<T>;
            })
    }*/
}

pub struct RefResolver<'a, T> {
    factory: &'a dyn Fn((), &dyn Fn(&T))
}

impl<'a, T> RefResolver<'a, T> {
    pub fn new(factory: &'a dyn Fn((), &dyn Fn(&T))) -> Self {
        Self { factory }
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
        let stack = &modified as *const RefCell<bool> as usize;
        add_to_container(Container::new(), |w| {
            let a = 1;
            println!("AfterAddStacksize: {}", stack - &w as *const Container as usize);
            println!("AfterAddStacksize: {}", stack - &a as *const i32 as usize);
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
        
        container.add_ref(
            &RefResolver::new(&move|(), v| {       
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
