extern crate libloading as lib;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lib = lib::Library::new("target/debug/libplugin.dylib")?;
    let mut container = ioc_rs::ServiceCollection::new();
    container.register_transient::<(), i32>(|_| 42);
    unsafe {
        let func: lib::Symbol<unsafe extern fn(&mut ioc_rs::ServiceCollection)> = lib.get(b"register")?;
        func(&mut container);
    }
    let provider = container.build();

    
    let service = provider
        .get::<ioc_rs::Transient<&dyn interface::Service>>()
        .expect("Expected plugin to register a &dyn Service");
    println!("Runtime: service.call(2) = {}", service.call(2));
    
    let number = provider
        .get::<ioc_rs::Transient<i64>>()
        .expect("Expected plugin to register i64");

    println!("Get 42 multiplied by 3: {}", number);
    Ok(())
}