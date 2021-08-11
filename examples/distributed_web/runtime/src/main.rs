use futures::future::join_all;
use ioc_rs::ServiceCollection;
use libloading::{Library, Symbol};
use raf_hosted_service::HostedService;
use std::{
    array::IntoIter,
    env::consts::{DLL_PREFIX, DLL_SUFFIX},
    error::Error,
};
use tokio;

type ServiceRegistrar = unsafe extern "C" fn(&mut ioc_rs::ServiceCollection);

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut collection = ServiceCollection::new();

    raf_web::service::register_services(&mut collection);
    raf_sql::service::register(&mut collection);

    // Lib must be referenced outside of unsafe block, because it's dropped otherwise, sporadically resulting in a segfault
    let lib_ids = IntoIter::new(["todo"]);
    let libs = lib_ids
        .map::<Result<_, libloading::Error>, _>(|id| unsafe {
            let lib_path = format!("target/debug/{}{}{}", DLL_PREFIX, id, DLL_SUFFIX);
            let lib = Library::new(lib_path)?;
            let func: Symbol<ServiceRegistrar> = lib.get(b"register")?;
            func(&mut collection);
            Ok(lib)
        })
        .collect::<Vec<_>>();
    println!(
        "WithErrors: {}/{}",
        libs.iter().filter(|x| x.is_err()).count(),
        libs.len()
    );

    let provider = collection.build().expect("all dependencies to resolve");

    let services: Vec<_> = provider
        .get_all::<Box<dyn HostedService>>()
        .map(|i| i.start())
        .collect();

    println!("Start {} servers", services.len());
    join_all(services).await;

    println!("Done");
    Ok(())
}
