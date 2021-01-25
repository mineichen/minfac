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
    container.register_singleton::<(), _>(|_| PluginService);
    container.register_transient::<ioc_rs::Singleton<PluginService>, _>(|c| c as &dyn interface::Service);
    container.register_transient::<ioc_rs::Transient<i32>, _>(|i| i as i64 * 3);
}