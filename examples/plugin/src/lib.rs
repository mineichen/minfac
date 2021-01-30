use std::sync::Arc;
struct PluginService;

impl interface::Service for PluginService {
    fn call(&self, a: i32) -> i32 {
        println!("plugin: I cuplicate {}", a);
        a * 2
    }
}

#[no_mangle]
pub fn register(container: &mut ioc_rs::ServiceCollection) {
    println!("plugin: Register Service");
    container.register_shared(|| PluginService);
    container.with::<ioc_rs::Shared<PluginService>>().register_transient(|c| c as Arc<dyn interface::Service>);
    container.with::<ioc_rs::Transient<i32>>().register_transient(|i| i as i64 * 3);
}