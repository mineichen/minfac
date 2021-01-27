use ioc_rs::{ServiceCollection, Singleton, resolvable::Resolvable};

fn main() {
    let container = ServiceCollection::new();
    let provider = container.build().expect("Expected to have all dependencies");
    Singleton::<i32>::resolve(&provider);
}