use ioc_rs::{ServiceCollection, Shared, resolvable::Resolvable};

fn main() {
    let container = ServiceCollection::new();
    let provider = container.build().expect("Expected to have all dependencies");
    Shared::<std::sync::Arc<i32>>::resolve(&provider);
}