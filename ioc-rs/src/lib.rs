use {
    core::{
        any::{Any, TypeId},
        marker::PhantomData
    },
    std::collections::HashMap,
    ref_list::{Collection, OptionRefList}
};

pub mod builder;
pub mod ref_list;
pub mod gat;

pub struct Container<'a> {
    // bool ist just a placeholder for the type to be resolved
    data: HashMap<TypeId, *const OptionRefList<'a, *const DynamicResolver<'a, bool>>>
}

trait DynamicContainer {
    fn insert<T: Any>(&mut self, data: &OptionRefList<*const DynamicResolver<T>>);
    fn get<T: Any>(&self) -> Option<&DynamicResolver<T>>;
}

impl<'a> DynamicContainer for Container<'a> {
    fn insert<T: Any>(&mut self, data: &OptionRefList<*const DynamicResolver<T>>) {
        self.data.insert( 
            TypeId::of::<T>(),
            data as *const OptionRefList<*const DynamicResolver<T>> as *const OptionRefList<*const DynamicResolver<bool>>
        );
    }
    fn get<T: Any>(&self) -> Option<&DynamicResolver<'_, T>> {
        match self.data.get(&TypeId::of::<T>()) {
            Some(r_list) => {
                let a: &OptionRefList<*const DynamicResolver<bool>> = unsafe { &**r_list };
                let r = a.iter().next().unwrap();
                let c = *r as *const DynamicResolver<T>;
                Some(unsafe{ &* c })
            },
            None => None
        }
    }
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

pub struct RefTuple2<T, U>(*const T, *const U);
impl<T, U> RefTuple2<T, U> {
    pub fn i0(&self) -> &T {
        unsafe {&*self.0}
    }
    pub fn i1(&self) -> &U {
        unsafe {&*self.1}
    }
}
pub struct RefTuple3<T, U, V>(*const T, *const U, *const V);
impl<T, U, V> RefTuple3<T, U, V> {
    pub fn i0(&self) -> &T {
        unsafe {&*self.0}
    }
    pub fn i1(&self) -> &U {
        unsafe {&*self.1}
    }
    pub fn i2(&self) -> &V {
        unsafe {&*self.2}
    }
}

pub struct RefTuple4<T, U, V, W>(*const T, *const U, *const V, *const W);
impl<T, U, V, W> RefTuple4<T, U, V, W> {
    pub fn i0(&self) -> &T {
        unsafe {&*self.0}
    }
    pub fn i1(&self) -> &U {
        unsafe {&*self.1}
    }
    pub fn i2(&self) -> &V {
        unsafe {&*self.2}
    }
    pub fn i3(&self) -> &W {
        unsafe {&*self.3}
    }
}

impl<T0: Resolvable> Resolvable for (T0,) {
    type Result = RefTuple<T0::Result>;
    fn resolve<TFn: Fn(&Self::Result)>(container: &Container, callback: TFn) {
        T0::resolve(container,|t0| {
            callback(&RefTuple(t0 as *const T0::Result) );
        });
    }
}

impl<T0: Resolvable, T1: Resolvable> Resolvable for (T0, T1) {
    type Result = RefTuple2<T0::Result, T1::Result>;
    fn resolve<TFn: Fn(&Self::Result)>(container: &Container, callback: TFn) {
        T0::resolve(container, |t0| {
            T1::resolve(container,|t1| {
                callback(&RefTuple2(
                    t0 as *const T0::Result, 
                    t1 as *const T1::Result
                ));
            });
        });
    }
}

pub struct Dynamic<T: Any> {
    phantom: PhantomData<T>
}

impl<T: Any> Resolvable for Dynamic<T> {
    type Result = T;
    fn resolve<TFn: Fn(&Self::Result)>(container: &Container, callback: TFn) {
        (container.get::<T>().unwrap().factory)(container, &|value| {
            (callback)(value);
        });
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

        self.insert(
            &OptionRefList::new(
                &DynamicResolver::new(
                    &|c: &Container, cb: &dyn Fn(&T)| { 
                        TDependency::resolve(c, |c| {
                            factory(c, cb)
                        });
                    }
                )
            )
        );
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
            w.try_resolve::<(Dynamic::<Instant>, Dynamic::<Instant>)>(&|tuple| {
                *modified.borrow_mut() = true;
                println!("Stacksize: {}", stack - tuple.i1() as *const Instant as usize);
                assert!(*tuple.i0() == *tuple.i1());
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
