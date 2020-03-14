use anymap::AnyMap;
use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::collections::HashMap;

/*
pub trait ServiceProvider {
    fn get<T: 'static>(&mut self) -> &mut T;
}
*/
pub struct StaticServiceProvider {
    services: AnyMap
}

impl /*ServiceProvider for */StaticServiceProvider {
    fn get<T: 'static>(&mut self) -> &mut T {
        match self.services.get_mut::<T>() {
            Some(a) => a,
            None => panic!("Requested unknown service")
        }
    }
}




pub struct StaticServiceCollection<'a> {
    services: AnyMap,
    service_descriptors: std::collections::HashMap<&'static str, &'a dyn Any>
}
/*
fn demo<T: Any + ?Sized>(a: &T) {
    let b = (&a) as &dyn Any;
}*/

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_singleton_service() {
        let mut collection = StaticServiceCollection::new();
        let service = TestService(42);        
        let service_trait = TestService(84);
        
        collection.add_singleton(&service);
        collection.add_singleton(&service_trait);
        /*
        let mut provider = collection.build_service_provider();
        let service = provider.get::<TestService>();
        assert_eq!(42, service.0);*/
    }

   
    #[test]
    fn add_singleton_as_trait() {
        /*let s = Seq::Empty;
        let test = Some(TestService(41));
        let any_test = &test as &dyn std::any::Any;
        println!("ID: {:?}", any_test.type_id());
        */
        let service = TestService(42);
        //test::<&dyn TestTrait, _>(&service);
        
        //let dynService : &dyn TestTrait = service.into();
        let mut collection = StaticServiceCollection::new();
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
    impl Container {
        fn try_attach() {

        }
    }
    
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

    struct Root<'a> {
        factories: HashMap<TypeId, &'a dyn Any>
    }

    impl<'a> Root<'a> {
        fn get_resolver<T: 'static>(&self) ->  Option<&&dyn Fn(&mut Container, &dyn Fn(&mut Container, T))> {
            match self.factories.get(&TypeId::of::<T>()) {
                Some(u) => u.downcast_ref::<&dyn Fn(&mut Container, &dyn Fn(&mut Container, T))>(),
                None => None
            }
        }
        fn add_resolver(&mut self, a: &dyn Fn(&mut Container, &dyn Fn(&mut Container, i32))) {
            //self.factories.insert(TypeId::of::<i32>(), &a);
        }
    }

    #[test]
    fn test() {
        let mut r = Root { factories: HashMap::new() };
        let a: &dyn Fn(&mut Container, &dyn Fn(&mut Container, i32)) = &|cont: &mut Container, consumer: &dyn Fn(&mut Container, i32)| { 
            consumer(cont, 10);
        };
        r.factories.insert(TypeId::of::<i32>(), &a);

        let mut container = Container;
        r.get_resolver().unwrap()(&mut container, &|_cont, val: i32| {
            println!("Really worked {}", val);
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
    trait TestTrait {
        fn get_value(&self) -> i32; 
    }
    #[derive(Debug)]
    struct TestService(i32);
    
    impl TestTrait for TestService {
        fn get_value(&self) -> i32 {
            self.0
        }
    }
    impl Drop for TestService {
        fn drop(&mut self) {

        }
    }
}
