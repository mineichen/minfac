use core::{
    any::{Any, TypeId}, 
    str::FromStr,
    cell::RefCell
};
use std::collections::HashMap;
use core::marker::PhantomData;
use core::hash::{Hash, Hasher};
use core::fmt::Debug;

/*
pub trait ServiceProvider {
    fn get<T: 'static>(&mut self) -> &mut T;
}
*/


/*
pub struct StaticServiceCollection<'a> {
    services: AnyMap,
    service_descriptors: std::collections::HashMap<&'static str, &'a dyn Any>
}


impl<'a> StaticServiceCollection<'a> {
    pub fn add_singleton<TEffective>(&mut self, service: &'a TEffective) where TEffective: Any {
        //self.services.insert(service);
        let type_name = std::any::type_name::<TEffective>();
        let service_ref = service as &dyn Any;
        self.service_descriptors.insert(type_name, service_ref);
        println!("Name: {}", type_name);        
    }
    
    pub fn build_service_provider(self) -> StaticServiceProvider {
        StaticServiceProvider {
            services: self.services
        }
    }

    pub fn new() -> Self {
        StaticServiceCollection {
            services: AnyMap::new(),
            service_descriptors: std::collections::HashMap::new()
        }
    }
}
*/
struct Cons<T> {
    item: T
    //fn next() : Option<T>;
}

trait List<T> {
    fn next() -> Option<T>;
}

impl<T> List<T> for Cons<T> {
    fn next() -> Option<T> {
        None
    }
}

fn test<TCast, TEff>(input: TEff) where TCast : From<TEff> {
    let a : TCast = input.into();
}

struct VecWrapper<'a> {
    data: Vec<&'a dyn Fn(&dyn Fn(&i32))>
}

impl<'a> VecWrapper<'a> {
    
}
#[cfg(test)]
mod tests {
    use super::*;

    

    #[test]
    fn add_singleton_service() {
        /*
        let mut collection = StaticServiceCollection::new();
        let service = TestService(42);        
        let service_trait = TestService(84);
        
        collection.add_singleton(&service);
        collection.add_singleton(&service_trait);
        
        let mut provider = collection.build_service_provider();
        let service = provider.get::<TestService>();
        assert_eq!(42, service.0);
        */
    }

    
   
    #[test]
    fn add_singleton_as_trait() {
        /*let s = Seq::Empty;
        let test = Some(TestService(41));
        let any_test = &test as &dyn std::any::Any;
        println!("ID: {:?}", any_test.type_id());
        */
        //let service = TestService(42);
        //test::<&dyn TestTrait, _>(&service);
        
        //let dynService : &dyn TestTrait = service.into();
        //let mut collection = StaticServiceCollection::new();
        //collection.add_singleton(&TestService(42) as );
        
        //collection.add_singleton::<&dyn TestTrait, TestService>(service);
        /*
        let mut provider = collection.build_service_provider();
        let service = provider.get::<&dyn TestTrait>();
        assert_eq!(42, service.get_value());*/

    }
/*
    fn blub() {
        let c = Container;
        c.resolve::<i32>(|a, b| {

        });
    }*/

    pub trait ForeignService {
        fn doStuff(&self);
    }

    pub struct Container;
   
    
    pub trait ResolvableRef<T: Sized> {
        fn resolveRef<'a>(&'a mut self, consumer: &'a dyn Fn(&mut Self, &T));
    }

    pub trait Resolvable<T: Sized> {
        fn resolve<TFn: Fn(&mut Self, T)>(&mut self, consumer: TFn);
    }
    
    impl Resolvable<TestService> for Container {
        fn resolve<TFn: Fn(&mut Self, TestService)>(&mut self, consumer: TFn) {
            self.resolveRef(&|a, n: &i32| {
                consumer(a, TestService(*n));
            });
        }
    }
    impl ResolvableRef<i32> for Container {
        fn resolveRef(&mut self, consumer: &dyn Fn(&mut Self, &i32)) where {
            let n = 42;
            consumer(self, &n);
        }
    }

    fn test_old<TFn: Fn(&mut Container, &dyn TestTrait)>(cont: &mut Container, consumer: TFn) {
        let a = TestService(1);
        consumer(cont, &a);
    }

    
    impl Resolvable<Box<TestTrait>> for Container {
        fn resolve<TFn: Fn(&mut Self, Box<TestTrait>)>(&mut self, consumer: TFn) {
            Resolvable::<TestService>::resolve(self, |ioc, n| {
                consumer(ioc, Box::new(n));
            });
        }
    }

    struct AmdStyle<'a, T> {
        factories: HashMap<AmdTypeId, *const dyn Fn(i32)>,
        ctx: PhantomData<&'a T>
    }

    impl<'a, T> AmdStyle<'a, T> {
        fn new() -> Self {
            AmdStyle {
                factories: HashMap::new(),
                ctx: PhantomData
            }
        } 
    }
    struct AmdContext;
    
    #[derive(Debug, PartialEq, Eq, Hash)]
    struct AmdTypeId {
        is_ref: bool, 
        type_id: TypeId
    }

    enum AmdTypeEnum<T> {
        Any(PhantomData<T>),
        Ref(PhantomData<T>),
    }



    impl AmdTypeId {
        fn new(is_ref: bool, type_id: TypeId) -> AmdTypeId {
            AmdTypeId {
                is_ref,
                type_id
            }
        }
    }
    
    pub trait AmdResolvable<'a, C> {
        type Dependency: AmdResolvable<'a, C>;
        
        //fn get_typeid() -> AmdTypeId;
    }

    impl<'a, T> AmdResolvable<'a, T> for () {
        type Dependency = ();
        /*
        fn get_typeid() -> AmdTypeId {
            AmdTypeId::new(false, TypeId::of::<()>())
        }*/
    }

    impl<'a> AmdResolvable<'a, AmdContext> for i32 {
        type Dependency = ();
        /*fn get_typeid() -> AmdTypeId {
            AmdTypeId::new(false, TypeId::of::<i32>())
        }*/
    }

    impl<'a> AmdResolvable<'a, AmdContext> for &'a i32 {
        type Dependency = ();
        /*fn get_typeid() -> AmdTypeId {
            AmdTypeId::new(false, TypeId::of::<*const i32>())
        }*/
    }

    
    impl<'a> AmdResolvable<'a, AmdContext> for String {
        type Dependency = ();/*
        fn get_typeid() -> AmdTypeId {
            AmdTypeId::new(false, TypeId::of::<String>())
        }*/
    } 
   
    impl<'a, T> AmdStyle<'a, T> {
        fn get_resolver<TResult: AmdResolvable<'a, T>>(&self) ->  Option<&'a dyn Fn(TResult::Dependency, &dyn Fn(TResult))> {
            unimplemented!();
            /*
            match self.factories.get(&TResult::get_typeid()) {
                Some(u) => {
                    let v = *u as *const dyn Fn(TResult::Dependency, &dyn Fn(TResult));
                    Some(unsafe {&*v})
                },
                None => None
            }*/
        }

        fn add_resolver<TResult: AmdResolvable<'a, T>>(&mut self, factory: &'a dyn Fn(TResult::Dependency, &dyn Fn(TResult))) {
            unimplemented!();
            
            /*
            self.factories.insert(
                TResult::get_typeid(), 
                factory as *const dyn Fn(TResult::Dependency, &dyn Fn(TResult)) as *const dyn Fn(i32)
            );*/
        }
    }
    
    #[test]
    fn test_amd_style() {
        let mut r = AmdStyle::<AmdContext>::new();
        {
            r.add_resolver(&|_, consumer| {
                consumer(42);
            });
            
            r.add_resolver(&move| _, consumer| {
                consumer(String::from("Test"));
            });
        }
        let resolver = r.get_resolver().unwrap();
        resolver((), &|r: i32| {
            println!("coolio {}", r);
        });
    }

    
    struct Root {
        factories: HashMap<TypeId, *const dyn Fn(i32)>
    }

    impl<'a> Root {

        fn get_resolver<T: 'static>(&self) ->  Option<&dyn Fn(&mut Container, &dyn Fn(&mut Container, T))> {
            match self.factories.get(&TypeId::of::<T>()) {
                Some(u) => {
                    let v = *u as *const dyn Fn(&mut Container, &dyn Fn(&mut Container, T));
                    Some(unsafe {&*v})
                },
                None => None
            }
        }
        fn add_resolver<T: 'static>(&mut self, factory: &dyn Fn(&mut Container, &dyn Fn(&mut Container, T))) {
            self.factories.insert(TypeId::of::<T>(), factory as *const dyn Fn(&mut Container, &dyn Fn(&mut Container, T)) as *const dyn Fn(i32));
        }
    }

    #[test]
    fn test() {
        let mut r = Root { factories: HashMap::new() };
        let data = 42;
        
        r.add_resolver(&|cont, consumer| { 
            consumer(cont, data);
        });
        r.add_resolver(&|cont, consumer| {
            let testTrait: Box<dyn TestTrait> = Box::new(TestService(42));
            consumer(cont, testTrait);
        });
    

        let mut container = Container;
        r.get_resolver().unwrap()(&mut container, &|_cont, val: Box<dyn TestTrait>| {
            println!("Really worked {:?}", val);
        });
    }

    

/*
    impl Resolvable<i32> for Container {
        fn resolve<TFn: Fn(&mut Self, i32)>(&mut self, consumer: TFn) {
            let n = 42;
            consumer(self, n);
        }
    }

*/
    trait TestTrait: Debug {
        fn get_value(&self) -> i32; 
    }
    #[derive(Debug)]
    struct TestService(i32);
    
    impl TestTrait for TestService {
        fn get_value(&self) -> i32 {
            self.0
        }
    }
}
