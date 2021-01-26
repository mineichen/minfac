use ioc_rs::{Singleton, ServiceCollection};

fn main() {
    let mut container = ServiceCollection::new();
    container.register_singleton::<(), _>(|_| 0);
    let provider = container.build().expect("Expected to have all dependencies");
    let nr_ref = provider.get::<Singleton::<i32>>().unwrap();
    drop(provider); 
    assert_eq!(
        2, 
        *nr_ref
    );
}