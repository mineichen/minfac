pub trait Service: Send + Sync {
    fn call(&self, a: i32) -> i32;
}
