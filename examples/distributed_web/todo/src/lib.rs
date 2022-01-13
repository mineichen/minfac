use minfac::ServiceCollection;

mod repository;
mod web;

#[derive(Debug)]
pub struct TodoItem {
    pub id: i32,
    pub title: String,
}

#[no_mangle]
pub extern fn register(collection: &mut ServiceCollection) {
    println!("Register tod");
    repository::register_services(collection);
    web::register_services(collection);
}
