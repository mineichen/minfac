use {
    libloading::{Library, Symbol},
    minfac::ServiceCollection,
    std::env::consts::{DLL_PREFIX, DLL_SUFFIX},
    std::sync::Arc,
};

type ServiceRegistrar = unsafe extern "C" fn(&mut minfac::ServiceCollection);

///
/// # Expected output
///
/// plugin: Register Service
/// plugin: I duplicate 2
/// Runtime: service.call(2) = 4
/// Runtime: Get 42 multiplied by 3: 126
///
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut collection = ServiceCollection::new();
    collection.register(|| 42);

    // Lib must be referenced outside of unsafe block, because it's dropped otherwise, sporadically resulting in a segfault
    let _lib = unsafe {
        let lib = Library::new(format!("target/debug/{}plugin{}", DLL_PREFIX, DLL_SUFFIX))?;
        let func: Symbol<ServiceRegistrar> = lib.get(b"register")?;
        func(&mut collection);
        lib
    };

    let provider = collection
        .build()
        .expect("Expected all dependencies to resolve");

    let service = provider
        .get::<Arc<dyn interface::Service>>()
        .expect("Expected plugin to register a &dyn Service");

    println!("Runtime: service.call(2) = {}", service.call(2));

    let number = provider
        .get::<i64>()
        .expect("Expected plugin to register i64");

    println!("Runtime: Get 42 multiplied by 3: {}", number);
    Ok(())
}
