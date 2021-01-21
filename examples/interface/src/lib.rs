pub trait Service {
    fn call(&self, a: i32) -> i32;
}