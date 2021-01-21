use {
    core::{
        marker::PhantomData,
        any::{Any, TypeId}
    },
    std::collections::HashMap
};
// The family trait for type constructors that have one input lifetime.
pub trait FamilyLt<'a> {
    type Out;
}

#[derive(Debug)]
pub struct IdFamily<T: Any>(PhantomData<T>);
impl<'a, T: Any> FamilyLt<'a> for IdFamily<T> {
    type Out = T;
}

#[derive(Debug)]
pub struct RefFamily<T: Any>(PhantomData<T>);
impl<'a, T: 'a + Any> FamilyLt<'a> for RefFamily<T> {
    type Out = &'a T;
}
#[derive(Debug)]
pub struct T2Family<T0: Resolvable, T1: Resolvable>(PhantomData<(T0, T1)>);
impl<'a, T0: Resolvable, T1: Resolvable> FamilyLt<'a> for T2Family<T0, T1> {
    type Out = (
        <T0::Item as FamilyLt<'a>>::Out,
        <T1::Item as FamilyLt<'a>>::Out
    );
}
#[derive(Debug)]
pub struct T3Family<T0: Resolvable, T1: Resolvable, T2: Resolvable>(PhantomData<(T0, T1, T2)>);
impl<'a, T0: Resolvable, T1: Resolvable, T2: Resolvable> FamilyLt<'a> for T3Family<T0, T1, T2> {
    type Out = (
        <T0::Item as FamilyLt<'a>>::Out, 
        <T1::Item as FamilyLt<'a>>::Out, 
        <T2::Item as FamilyLt<'a>>::Out
    );
}

#[derive(Debug)]
pub struct T4Family<T0: Resolvable, T1: Resolvable, T2: Resolvable, T3: Resolvable>(PhantomData<(T0, T1, T2, T3)>);
impl<'a, T0: Resolvable, T1: Resolvable, T2: Resolvable, T3: Resolvable> FamilyLt<'a> for T4Family<T0, T1, T2, T3> {
    type Out = (
        <T0::Item as FamilyLt<'a>>::Out, 
        <T1::Item as FamilyLt<'a>>::Out, 
        <T2::Item as FamilyLt<'a>>::Out, 
        <T3::Item as FamilyLt<'a>>::Out
    );
}

pub trait Resolvable: Any {
    type Item: for<'a> FamilyLt<'a>;

    fn resolve<'s>(container: &'s Container) -> Option<<Self::Item as FamilyLt<'s>>::Out>;
}

impl Resolvable for () {
    type Item = IdFamily<()>;

    fn resolve<'s>(_: &'s Container) -> Option<<Self::Item as FamilyLt<'s>>::Out> {
        Some(())
    }
}

impl<T0: Resolvable, T1: Resolvable> Resolvable for (T0, T1) {
    type Item = T2Family<T0, T1>;

    fn resolve<'s>(container: &'s Container) -> Option<<Self::Item as FamilyLt<'s>>::Out> {
        Some((T0::resolve(container).unwrap(), T1::resolve(container).unwrap()))
    }
}
impl<T0: Resolvable, T1: Resolvable, T2: Resolvable> Resolvable for (T0, T1, T2) {
    type Item = T3Family<T0, T1, T2>;

    fn resolve<'s>(container: &'s Container) -> Option<<Self::Item as FamilyLt<'s>>::Out> {
        Some((
            T0::resolve(container).unwrap(), 
            T1::resolve(container).unwrap(),
            T2::resolve(container).unwrap()
        ))
    }
}
impl<T0: Resolvable, T1: Resolvable, T2: Resolvable, T3: Resolvable> Resolvable for (T0, T1, T2, T3) {
    type Item = T4Family<T0, T1, T2, T3>;

    fn resolve<'s>(container: &'s Container) -> Option<<Self::Item as FamilyLt<'s>>::Out> {
        Some((
            T0::resolve(container).unwrap(), 
            T1::resolve(container).unwrap(),
            T2::resolve(container).unwrap(),
            T3::resolve(container).unwrap()
        ))
    }
}
/*
struct R2<T0: Resolvable, T1: Resolvable>(PhantomData<(T0, T1)>);
impl<T0: Resolvable, T1: Resolvable> Resolvable for R2<T0, T1> {
    type Item = IdFamily<(T0::Item, T1::Item)>;

    fn resolve<'s>(container: &'s Container) -> Option<<Self::Item as FamilyLt<'s>>::Out> {
        Some((
            T0::resolve(container).unwrap() as <T0::Item as FamilyLt, 
            T1::resolve(container).unwrap()
        ))
    }
}*/

impl Resolvable for Container {
    type Item = RefFamily<Container>;

    fn resolve<'s>(container: &'s Container) -> Option<<Self::Item as FamilyLt<'s>>::Out> {
        Some(container)
    }
}
struct DynamicRef<T: Any>(PhantomData<T>);
impl<T: Any> Resolvable for DynamicRef<T> {
    type Item = RefFamily<T>;

    fn resolve<'s>(container: &'s Container) -> Option<<Self::Item as FamilyLt<'s>>::Out> {
        container.resolve_registered::<Self>()
    }
}

struct DynamicId<T: Any>(PhantomData<T>);
impl<T: Any> Resolvable for DynamicId<T> {
    type Item = IdFamily<T>;

    fn resolve<'s>(container: &'s Container) -> Option<<Self::Item as FamilyLt<'s>>::Out> {
        container.resolve_registered::<Self>()
    }
}

pub struct Container {
    producers: HashMap<TypeId, *const dyn Fn()>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            producers: HashMap::new()
        }
    }
} 
impl Container {
    pub fn get<'s, T: Resolvable>(&'s self) -> Option<<T::Item as FamilyLt<'s>>::Out> {
        // if TypeId::of::<DynamicRef<Container>>() == TypeId::of::<T>() {
        //     let i = unsafe {std::mem::transmute::<&'s Self, <T::Item as FamilyLt<'s>>::Out>(self)};
        //     return Some(i);
        // }
        T::resolve(self)
    }
    /// Might return Some() for DynamicRef or DynamicId. Others Resolvables are not dynamic
    fn resolve_registered<'s, T: Resolvable>(&'s self) -> Option<<T::Item as FamilyLt<'s>>::Out> {
        self.producers
            .get(&TypeId::of::<T>())
            .map(|f| {                
                let func_ptr = *f as *const dyn Fn(&Container) -> <T::Item as FamilyLt<'s>>::Out;
                let func = unsafe { &* func_ptr};
                
                (func)(&self)
            })
    }
    pub fn register_id<'s, 'a: 's, TDependency: Resolvable, T: Any>(&'s mut self, creator: fn(<TDependency::Item as FamilyLt<'a>>::Out) -> T) {
        let func : Box<dyn Fn(&'a Container) -> T> = Box::new(move |container: &'a Container| {
            let arg = TDependency::resolve(container);
            creator(arg.unwrap())
        });
        
        self.producers.insert(
            TypeId::of::<DynamicId<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        );
    }
    pub fn register_ref<'s, 'a: 's, TDependency: Resolvable, T: Any>(&'s mut self, creator: fn(<TDependency::Item as FamilyLt<'a>>::Out) -> T) {
        let cell = once_cell::sync::OnceCell::new();
        let func : Box<dyn Fn(&'a Container) -> &T> = Box::new(move |container: &'a Container| { 
            unsafe { 
                // Deref is valid because container cannot delete any producers
                // Unless destroying itself
                &*(cell.get_or_init(|| {
                    let arg = TDependency::resolve(container);
                    creator(arg.unwrap())
                }) as *const T)
            }
        });
        
        self.producers.insert(
            TypeId::of::<DynamicRef<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
   
    #[test]
    fn resolve_test() {
        let mut container = Container::new();
        container.register_id::<(), _>(|_| 42);
        container.register_ref::<Container,_>(|_| 42);

        assert_eq!(
            DynamicId::<i32>::resolve(&container).unwrap(), 
            DynamicRef::<i32>::resolve(&container).map(|f| *f).unwrap()
        );
    }

    #[test]
    fn get_registered_dynamic_id() {
        let mut container = Container::new();
        container.register_id::<(),_>(|_| 42);
        assert_eq!(Some(42i32), container.get::<DynamicId<i32>>());
    }
    #[test]
    fn get_registered_dynamic_ref() {
        let mut container = Container::new();
        container.register_ref::<(), i32>(|_| 42);
        assert_eq!(Some(&42i32), container.get::<DynamicRef<i32>>());
    }
    #[test]
    fn get_unkown_returns_none() {
        let container = Container::new();
        assert_eq!(None, container.get::<DynamicId<i32>>());
    }

    #[test]
    fn resolve_tuple_2() {
        let mut container = Container::new();
        container.register_id::<(), i32>(|_| 32);
        container.register_ref::<(), i64>(|_| 64);
        assert_eq!(Some((32, &64)), container.get::<(DynamicId<i32>, DynamicRef<i64>)>());
    }

    #[test]
    fn register_struct_as_dynamic() {
        let mut container = Container::new();        
        container.register_ref::<Container, _>(|_| ServiceImpl(42));
        container.register_id::<DynamicRef<ServiceImpl>, _>(|c| c as &dyn Service);
        let service = container
            .get::<DynamicId<&dyn Service>>()
            .expect("Expected to get a service");
        assert_eq!(42, service.get_value());
    }

    trait Service {
        fn get_value(&self) -> i32;
    }

    struct ServiceImpl(i32);
    impl Service for ServiceImpl {
        fn get_value(&self) -> i32 {
            self.0
        }
    }
}