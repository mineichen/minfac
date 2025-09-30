use std::fmt::Debug;

/// Ffi-safe equivalent of `Result<T, E>`.
#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum FfiResult<T, E> {
    FfiOk(T),
    FfiErr(E),
}

pub use self::FfiResult::*;

impl<T, E> From<Result<T, E>> for FfiResult<T, E> {
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(x) => FfiOk(x),
            Err(e) => FfiErr(e),
        }
    }
}

impl<T, E> From<FfiResult<T, E>> for Result<T, E> {
    fn from(value: FfiResult<T, E>) -> Self {
        match value {
            FfiOk(x) => Ok(x),
            FfiErr(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn from_into() {
        assert_eq!(FfiResult::from(Ok::<u32, u32>(10)), FfiOk(10));
        assert_eq!(FfiResult::from(Err::<u32, u32>(4)), FfiErr(4));

        assert_eq!(Result::from(FfiOk::<u32, u32>(10)), Ok(10));
        assert_eq!(Result::from(FfiErr::<u32, u32>(4)), Err(4));
    }
}
