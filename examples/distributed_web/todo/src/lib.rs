use minfac::ServiceCollection;

mod repository;
mod web;

#[derive(Debug)]
pub struct TodoItem {
    pub id: i32,
    pub title: String,
}

#[no_mangle]
pub extern "C" fn register(collection: &mut ServiceCollection) {
    println!("Register todo");
    repository::register_services(collection);
    web::register_services(collection);
}
