use minfac::{
    AllRegistered, BuildError, Registered, Resolvable, ServiceCollection, WeakServiceProvider,
};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

#[test]
#[should_panic(expected = "Panicking while copy exists")]
fn drop_service_provider_with_existing_clone_on_panic_is_recoverable_with_default_error_handler() {
    let mut _outer = None;
    {
        let mut collection = ServiceCollection::new();
        collection.with::<WeakServiceProvider>().register(|p| p);
        let provider = collection.build().unwrap();
        _outer = Some(provider.get::<WeakServiceProvider>().unwrap());
        panic!("Panicking while copy exists");
    }
}

#[test]
#[should_panic(expected = "Panicking while shared exists")]
fn drop_service_provider_with_existing_shared_registered_on_panic_is_recoverable_with_default_error_handler(
) {
    let mut _outer = None;
    {
        let mut collection = ServiceCollection::new();
        collection.register_shared(|| Arc::new(1i32));
        let provider = collection.build().unwrap();
        _outer = provider.get::<Arc<i32>>();
        panic!("Panicking while shared exists");
    }
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(
    expected = "Some instances outlived their ServiceProvider: [Type: i32 (remaining 1)]"
)]
fn drop_service_provider_with_existing_shared_registered_is_panicking() {
    let mut _outer = None;
    {
        let mut collection = ServiceCollection::new();
        collection.register_shared(|| Arc::new(1i32));
        let provider = collection.build().unwrap();
        _outer = provider.get::<Arc<i32>>();
    }
}

#[test]
fn resolve_last() {
    let mut col = ServiceCollection::new();
    col.register(|| 0);
    col.register(|| 5);
    col.register(|| 1);
    col.register(|| 2);
    let provider = col.build().expect("Expected to have all dependencies");
    assert_eq!(Some(2), provider.get());
}

#[test]
fn resolve_shared() {
    let mut col = ServiceCollection::new();
    col.register_shared(|| Arc::new(AtomicI32::new(1)));
    col.with::<WeakServiceProvider>()
        .register_shared(|_| Arc::new(AtomicI32::new(2)));

    let provider = col.build().expect("Should have all Dependencies");
    let service = provider
        .get::<Arc<AtomicI32>>()
        .expect("Expecte to get second");
    assert_eq!(2, service.load(Ordering::Relaxed));
    service.fetch_add(40, Ordering::Relaxed);

    assert_eq!(
        provider
            .get_all::<Arc<AtomicI32>>()
            .map(|c| c.load(Ordering::Relaxed))
            .sum::<i32>(),
        1 + 42
    );
}

#[test]
fn build_with_missing_dep_fails() {
    build_with_missing_dependency_fails::<Registered<String>>(&["Registered", "String"]);
}

#[test]
fn build_with_missing_tuple2_dep_fails() {
    build_with_missing_dependency_fails::<(Registered<String>, Registered<i32>)>(&[
        "Registered",
        "String",
    ]);
}

#[test]
fn build_with_missing_tuple3_dep_fails() {
    build_with_missing_dependency_fails::<(Registered<String>, Registered<i32>, Registered<i32>)>(
        &["Registered", "String"],
    );
}
#[test]
fn build_with_missing_tuple4_dep_fails() {
    build_with_missing_dependency_fails::<(
        Registered<i32>,
        Registered<String>,
        Registered<i32>,
        Registered<i32>,
    )>(&["Registered", "String"]);
}

fn build_with_missing_dependency_fails<T: Resolvable>(missing_msg_parts: &[&str]) {
    fn check(mut col: ServiceCollection, missing_msg_parts: &[&str]) {
        col.register(|| 1);
        match col.build() {
            Ok(_) => panic!("Build with missing dependency should fail"),
            Err(e) => match e {
                BuildError::MissingDependency { name, .. } => {
                    for part in missing_msg_parts {
                        assert!(
                            name.contains(part),
                            "Expected '{}' to contain '{}'",
                            name,
                            part
                        );
                    }
                }
                _ => panic!("Unexpected Error"),
            },
        }
    }
    let mut col = ServiceCollection::new();
    col.with::<T>().register(|_| ());
    check(col, missing_msg_parts);

    let mut col = ServiceCollection::new();
    col.with::<T>().register_shared(|_| Arc::new(()));
    check(col, missing_msg_parts);
}

#[test]
fn resolve_shared_returns_last_registered() {
    let mut collection = ServiceCollection::new();
    collection.register_shared(|| Arc::new(0));
    collection.register_shared(|| Arc::new(1));
    collection.register_shared(|| Arc::new(2));
    let provider = collection
        .build()
        .expect("Expected to have all dependencies");
    let nr_ref = provider.get::<Arc<i32>>().unwrap();
    assert_eq!(2, *nr_ref);
}

#[test]
fn resolve_all_services() {
    let mut collection = ServiceCollection::new();
    collection.register(|| 0);
    collection.register(|| 5);
    collection.register(|| 2);
    let provider = collection
        .build()
        .expect("Expected to have all dependencies");

    // Count
    let mut count_subset = provider.get_all::<i32>();
    count_subset.next();
    assert_eq!(2, count_subset.count());
    assert_eq!(3, provider.get_all::<i32>().count());

    // Last
    assert_eq!(2, provider.get_all::<i32>().last().unwrap());

    let mut sub = provider.get_all::<i32>();
    sub.next();
    assert_eq!(Some(2), sub.last());

    let mut consumed = provider.get_all::<i32>();
    consumed.by_ref().for_each(|_| {});
    assert_eq!(None, consumed.last());

    let mut iter = provider.get_all::<i32>();
    assert_eq!(Some(0), iter.next());
    assert_eq!(Some(5), iter.next());
    assert_eq!(Some(2), iter.next());
    assert_eq!(None, iter.next());
}

#[test]
fn no_dependency_needed_if_service_depends_on_services_which_are_not_present() {
    let mut collection = ServiceCollection::new();
    collection.with::<AllRegistered<String>>().register(|_| 0);

    assert!(collection.build().is_ok())
}

#[test]
fn resolve_shared_services() {
    let mut collection = ServiceCollection::new();
    collection.register_shared(|| Arc::new(0));
    collection.register_shared(|| Arc::new(5));
    collection.register_shared(|| Arc::new(2));
    let provider = collection
        .build()
        .expect("Expected to have all dependencies");

    // Count
    let mut count_subset = provider.get_all::<Arc<i32>>();
    count_subset.next();
    assert_eq!(2, count_subset.count());
    assert_eq!(3, provider.get_all::<Arc<i32>>().count());

    // Last
    assert_eq!(2, *provider.get_all::<Arc<i32>>().last().unwrap());

    let mut sub = provider.get_all::<Arc<i32>>();
    sub.next();
    assert_eq!(Some(2), sub.last().map(|i| *i));

    let mut consumed = provider.get_all::<Arc<i32>>();
    consumed.by_ref().for_each(|_| {});
    assert_eq!(None, consumed.last());

    let mut iter = provider.get_all::<Arc<i32>>().map(|i| *i);
    assert_eq!(Some(0), iter.next());
    assert_eq!(Some(5), iter.next());
    assert_eq!(Some(2), iter.next());
    assert_eq!(None, iter.next());
}

#[test]
fn resolve_test() {
    let mut collection = ServiceCollection::new();
    collection.register(|| 42);
    collection.register_shared(|| Arc::new(42));
    let provider = collection
        .build()
        .expect("Expected to have all dependencies");
    assert_eq!(
        provider.get::<i32>(),
        provider.get::<Arc<i32>>().map(|f| *f)
    );
}

#[test]
fn get_registered_dynamic_id() {
    let mut collection = ServiceCollection::new();
    collection.register(|| 42);
    assert_eq!(
        Some(42i32),
        collection
            .build()
            .expect("Expected to have all dependencies")
            .get()
    );
}
#[test]
fn get_registered_dynamic_ref() {
    let mut collection = ServiceCollection::new();
    collection.register_shared(|| Arc::new(42));
    assert_eq!(
        Some(42i32),
        collection
            .build()
            .expect("Expected to have all dependencies")
            .get::<Arc<i32>>()
            .map(|i| *i)
    );
}

#[test]
fn tuple_dependency_resolves_to_prechecked_type() {
    let mut collection = ServiceCollection::new();
    collection.register(|| 64i64);
    collection
        .with::<(Registered<i64>, Registered<i64>)>()
        .register_shared(|(a, b)| {
            assert_eq!(64, a);
            assert_eq!(64, b);
            Arc::new(42)
        });
    assert_eq!(
        Some(42i32),
        collection
            .build()
            .expect("Expected to have all dependencies")
            .get::<Arc<i32>>()
            .map(|i| *i)
    );
}

#[test]
fn get_unkown_returns_none() {
    let collection = ServiceCollection::new();
    assert_eq!(
        None,
        collection
            .build()
            .expect("Expected to have all dependencies")
            .get::<i32>()
    );
}

#[test]
fn resolve_tuple_2() {
    let mut collection = ServiceCollection::new();
    collection.register(|| 32i32);
    collection.register_shared(|| Arc::new(64i64));
    let provider = collection
        .build()
        .expect("Expected to have all dependencies");
    let (a, b) = provider.resolve_unchecked::<(Registered<i32>, Registered<Arc<i64>>)>();
    assert_eq!(32, a);
    assert_eq!(64, *b);
}

#[test]
fn register_struct_as_dynamic() {
    let mut collection = ServiceCollection::new();
    collection.register_shared(|| Arc::new(42i32));
    collection
        .with::<Registered<Arc<i32>>>()
        .register_shared(|i| Arc::new(ServiceImpl(i)))
        .alias(|a| a as Arc<dyn Service + Send + Sync>);
    let provider = collection
        .build()
        .expect("Expected to have all dependencies");
    let service = provider
        .get::<Arc<dyn Service + Send + Sync>>()
        .expect("Expected to get a service");

    assert_eq!(42, service.get_value());
    drop(service);
    drop(provider);
}

trait Service {
    fn get_value(&self) -> i32;
}

struct ServiceImpl<T: core::ops::Deref<Target = i32>>(T);
impl<T: core::ops::Deref<Target = i32>> Service for ServiceImpl<T> {
    fn get_value(&self) -> i32 {
        *self.0
    }
}

#[test]
fn drop_collection_doesnt_call_any_factories() {
    let mut col = ServiceCollection::new();
    col.register_shared::<Arc<()>>(|| {
        panic!("Should never be called");
    });
    let prov = col.build().unwrap();
    drop(prov);
}
#[test]
fn drop_shareds_after_provider_drop() {
    let mut col = ServiceCollection::new();
    col.register_shared(|| Arc::new(Arc::new(())));
    let prov = col.build().expect("Expected to have all dependencies");
    let inner = prov
        .get::<Arc<Arc<()>>>()
        .expect("Expected to receive the service")
        .as_ref()
        .clone();

    assert_eq!(2, Arc::strong_count(&inner));
    drop(prov);
    assert_eq!(1, Arc::strong_count(&inner));
}

#[test]
fn register_instance() {
    let mut col = ServiceCollection::new();
    col.register_instance(42);
    let prov = col.build().unwrap();
    assert_eq!(Some(42), prov.get());
}

#[test]
fn register_multiple_alias_per_type() {
    let mut col = ServiceCollection::new();
    let mut i8alias = col.register(|| 1i8);
    let mut i16alias = i8alias.alias(|a| a as i16 * 2);
    i8alias.alias(|a| a as i32 * 2);
    i16alias.alias(|a| a as i64 * 2);

    let prov = col.build().unwrap();
    assert_eq!(Some(2i32), prov.get());
    assert_eq!(Some(4i64), prov.get());
}
