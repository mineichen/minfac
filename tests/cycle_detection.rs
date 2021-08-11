use minfac::{self, AllRegistered, BuildError, Registered, ServiceCollection};

#[test]
fn handle_cyclic_references() {
    let mut col = ServiceCollection::new();
    col.with::<Registered<i64>>().register(|_| 0i16);
    col.with::<Registered<i16>>().register(|_| 0i32);
    col.with::<Registered<i32>>().register(|_| 0i64);

    let err = col.build().expect_err("Expected to return error");
    let msg = match err {
        BuildError::CyclicDependency(msg) => msg,
        _ => panic!("Expected BuildError::CyclicDependency"),
    };
    assert!(msg.contains("i32 -> i16"));
    assert!(msg.contains("i16 -> i64"));
    assert!(msg.contains("i64 -> i32"));
}

#[test]
fn one_of_multiple_dependencies_asks_for_dependent_should_trigger_cyclic_dependency() {
    let mut col = ServiceCollection::new();
    col.register(|| 0i32);
    col.with::<Registered<i64>>().register(|_| 1i32);
    col.register(|| 2i32);

    col.with::<AllRegistered<i32>>().register(|_| 42i64);
    let error = col.build().expect_err("Expected to return error");
    let msg = if let BuildError::CyclicDependency(msg) = error {
        msg
    } else {
        panic!("Expected error");
    };
    assert!(msg.contains("minfac::ServiceIterator<minfac::Registered<i32>> ->"));
    assert!(msg.contains("i64 ->"));
}

#[test]
fn service_a_depends_on_other_which_has_reference_to_typeof_a_but_a_is_not_last_registered() {
    let mut col = ServiceCollection::new();
    col.with::<Registered<i32>>().register(|_| 0i64);
    col.with::<Registered<i64>>().register(|_| 1i32);
    col.register(|| 2i32);

    col.build()
        .expect("Expecting constellation to be resolvable");
}
