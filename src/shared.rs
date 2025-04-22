use crate::{untyped::FromArcAutoFreePointer, ArcAutoFreePointer};

pub trait ShareInner: Sized + From<Self::Inner> {
    type Inner: FromArcAutoFreePointer;
}
impl<T: ShareInner> From<T> for ArcAutoFreePointer
where
    T::Inner: From<T> + Into<ArcAutoFreePointer>,
{
    fn from(value: T) -> Self {
        (T::Inner::from(value)).into()
    }
}

impl<T: ShareInner> FromArcAutoFreePointer for T
where
    T::Inner: From<T>,
{
    #[allow(private_interfaces)]
    unsafe fn from_ref(value: &ArcAutoFreePointer) -> Self {
        T::from(T::Inner::from_ref(value))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::ServiceCollection;

    use super::*;

    #[test]
    fn register_share_inner() {
        struct Bar(Arc<i32>);

        impl ShareInner for Bar {
            type Inner = Arc<i32>;
        }
        impl From<Arc<i32>> for Bar {
            fn from(value: Arc<i32>) -> Self {
                Self(value)
            }
        }
        impl From<Bar> for Arc<i32> {
            fn from(value: Bar) -> Self {
                value.0
            }
        }

        let mut collection = ServiceCollection::new();
        collection.register_shared(|| Bar(Arc::new(42)));
        let provider = collection.build().unwrap();
        assert_eq!(42, *provider.get::<Bar>().unwrap().0);
        assert_eq!(None, provider.get::<Arc<i32>>());
    }
}
