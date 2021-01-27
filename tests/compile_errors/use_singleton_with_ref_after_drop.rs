use ioc_rs::{Singleton, Transient, ServiceCollection};

fn main() {
    let mut container = ServiceCollection::new();   
    container.register_singleton(|| ServiceImpl(&1));
    container.with::<Singleton<ServiceImpl>>().register_transient(|c| c as &dyn Service);
    let provider = container.build().expect("Expected to have all dependencies");
    let service = provider.get::<Transient<&dyn Service>>()
        .expect("Expected to get a service");
    drop(provider); // Must fail to compile
        
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