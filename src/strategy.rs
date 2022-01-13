use core::{
    any::{Any, TypeId},
    fmt::Debug,
};

#[cfg_attr(feature = "stable_abi", abi_stable::sabi_trait)]
pub trait Strategy: Debug + Send + Sync {
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