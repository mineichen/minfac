use core::{
    any::{Any, TypeId},
    fmt::Debug,
};

pub trait Strategy: 'static + Debug + Send + Sync {
    type Id: Ord + Debug + Copy + PartialEq + Eq;
}

pub trait Identifyable<T: Ord>: 'static {
    fn get_id() -> T;
}

impl<T: Any> Identifyable<TypeId> for T {
    fn get_id() -> TypeId {
        TypeId::of::<T>()
    }
}

#[derive(PartialEq, Debug)]
pub struct AnyStrategy;
impl Strategy for AnyStrategy {
    type Id = TypeId;
}

pub trait CallableOnce<TParam, TResult> {
    fn new(o: impl 'static + FnOnce(TParam) -> TResult) -> Self;
    fn call(self, p: TParam) -> TResult;
}

impl<TParam, TResult> CallableOnce<TParam, TResult> for Box<dyn FnOnce(TParam) -> TResult> {
    fn new(o: impl 'static + FnOnce(TParam) -> TResult) -> Self {
        Box::new(o) as Box<dyn FnOnce(TParam) -> TResult>
    }

    fn call(self, p: TParam) -> TResult {
        (self)(p)
    }
}
