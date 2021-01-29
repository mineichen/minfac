use {
    core::{
        any::Any,
        marker::PhantomData
    }
};
// The family trait for type constructors that have one input lifetime.
pub trait FamilyLt<'a> {
    type Out: 'a;
}

#[derive(Debug)]
pub struct IdFamily<T: Any>(PhantomData<T>);
impl<'a, T: 'a + Any> FamilyLt<'a> for IdFamily<T> {
    type Out = T;
}

#[derive(Debug)]
pub struct RefFamily<T: Any>(PhantomData<T>);
impl<'a, T: 'a + Any> FamilyLt<'a> for RefFamily<T> {
    type Out = &'a T;
}

impl<'a, T: FamilyLt<'a> + 'a> FamilyLt<'a> for Option<T> {
    type Out = Option<T::Out>;
}
impl<'a, T: FamilyLt<'a> + 'a> FamilyLt<'a> for std::sync::Arc<T> {
    type Out = std::sync::Arc<T::Out>;
}

impl<'a, T0: FamilyLt<'a> + 'a, T1: FamilyLt<'a> + 'a> FamilyLt<'a> for (T0, T1) {
    type Out = (
        <T0 as FamilyLt<'a>>::Out,
        <T1 as FamilyLt<'a>>::Out
    );
}
impl<'a, T0: FamilyLt<'a> + 'a, T1: FamilyLt<'a> + 'a, T2: FamilyLt<'a> + 'a> FamilyLt<'a> for (T0, T1, T2) {
    type Out = (
        <T0 as FamilyLt<'a>>::Out,
        <T1 as FamilyLt<'a>>::Out,
        <T2 as FamilyLt<'a>>::Out
    );
}
impl<'a, T0: FamilyLt<'a>, T1: FamilyLt<'a>, T2: FamilyLt<'a>, T3: FamilyLt<'a>> FamilyLt<'a> for (T0, T1, T2, T3) {
    type Out = (
        <T0 as FamilyLt<'a>>::Out,
        <T1 as FamilyLt<'a>>::Out,
        <T2 as FamilyLt<'a>>::Out,
        <T3 as FamilyLt<'a>>::Out
    );
}
pub struct ServiceIteratorFamily<T>(PhantomData<T>);

impl<'a, T: crate::resolvable::Resolvable> FamilyLt<'a> for ServiceIteratorFamily<T> {
    type Out = crate::ServiceIterator<'a, T>;
}