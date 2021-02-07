use ioc_rs::{ServiceCollection, Dynamic, resolvable::Resolvable};

fn main() {
    let container = ServiceCollection::new();
    let provider = container.build().expect("Expected to have all dependencies");
    Dynamic::<std::sync::Arc<i32>>::resolve(&provider);
}