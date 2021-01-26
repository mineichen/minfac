use ioc_rs::{Singleton, Transient, ServiceCollection};

fn main() {
    let mut container = ServiceCollection::new();   
    container.register_singleton::<(), _>(|_| ServiceImpl(&1));
    container.register_transient::<Singleton<ServiceImpl>, _>(|c| c as &dyn Service);
    let provider = container.build().expect("Expected to have all dependencies");
    let service = provider.get::<Transient<&dyn Service>>()
        .expect("Expected to get a service");
    drop(provider);
        
    assert_eq!(42, service.get_value());
}

trait Service {
    fn get_value(&self) -> i32;
}

struct ServiceImpl<'a>(&'a i32);
impl<'a> Service for ServiceImpl<'a> {
    fn get_value(&self) -> i32 {
        *self.0
    }
}