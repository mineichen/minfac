extern crate libloading as lib;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lib = lib::Library::new("target/debug/libplugin.dylib")?;
    let mut container = ioc_rs::GenericContainer::new();
    unsafe {
        let func: lib::Symbol<unsafe extern fn(&mut ioc_rs::GenericContainer)> = lib.get(b"register")?;
        func(&mut container);
    }
    let service = container.get::<ioc_rs::DynamicId<&dyn interface::Service>>().unwrap();
    println!("Runtime: service.call(2) = {}", service.call(2));
    Ok(())
}