use {
    ioc_rs::{Registered, ServiceCollection},
    std::sync::Arc,
};
struct PluginService;

impl interface::Service for PluginService {
    fn call(&self, a: i32) -> i32 {
        println!("plugin: I duplicate {}", a);
        a * 2
    }
}

#[no_mangle]
pub extern "C" fn register(collection: &mut ServiceCollection) {
    println!("plugin: Register Service");
    collection.register_shared(|| Arc::new(PluginService))
        .alias(|x| x as Arc<dyn interface::Service>);
    collection
        .with::<Registered<i32>>()
        .register(|i| i as i64 * 3);
}
