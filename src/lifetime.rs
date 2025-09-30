use core::fmt::Debug;
use core::fmt::Formatter;
use std::fmt::Write;

use abi_stable::std_types::RVec;

use crate::ffi::FfiStr;

pub extern "C-unwind" fn default_error_handler(error: &LifetimeError) {
    #[cfg(feature = "std")]
    if !std::thread::panicking() {
        panic!("{:?}", error)
    }
}

/// Lifetime-Errors occur when either a WeakServiceProvider or any shared service
/// outlives the ServiceProvider.
#[repr(C)]
pub struct LifetimeError(OutlivedLifetimeErrorVariants);

impl LifetimeError {
    pub(crate) fn new(error: OutlivedLifetimeErrorVariants) -> Self {
        Self(error)
    }
}

impl Debug for LifetimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            OutlivedLifetimeErrorVariants::WeakServiceProvider {
                remaining_references,
            } => {
                write!(
                    f,
                    "Original ServiceProvider was dropped while still beeing used {} times",
                    remaining_references
                )
            }
            OutlivedLifetimeErrorVariants::SharedServices(s) => {
                write!(f, "Some instances outlived their ServiceProvider: {:?}", s)
            }
        }
    }
}

#[repr(C)]
pub(crate) enum OutlivedLifetimeErrorVariants {
    WeakServiceProvider { remaining_references: usize },
    SharedServices(DanglingCheckerResults),
}

#[repr(C)]
pub(crate) struct DanglingCheckerResults(RVec<DanglingCheckerResult>);

impl DanglingCheckerResults {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl FromIterator<DanglingCheckerResult> for DanglingCheckerResults {
    fn from_iter<T: IntoIterator<Item = DanglingCheckerResult>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl Debug for DanglingCheckerResults {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_char('[')?;
        let mut data = self.0.iter();

        if let Some(next) = data.next() {
            f.write_fmt(format_args!("{:?}", next))?;
            for x in data {
                f.write_fmt(format_args!(", {:?}", x))?;
            }
        }

        f.write_char(']')?;
        Ok(())
    }
}

#[repr(C)]
pub(crate) struct DanglingCheckerResult {
    remaining_references: usize,
    typename: FfiStr<'static>,
}

impl DanglingCheckerResult {
    pub fn new(remaining_references: usize, typename: &'static str) -> Self {
        Self {
            remaining_references,
            typename: typename.into(),
        }
    }
}

impl Debug for DanglingCheckerResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} (remaining {})",
            self.typename, self.remaining_references
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::ServiceCollection;

    use super::*;

    #[test]
    fn debug_dangling_checker_result() {
        assert_eq!(
            "foo::bar (remaining 5)",
            format!("{:?}", DanglingCheckerResult::new(5, "foo::bar"))
        );
    }

    #[test]
    fn debug_error_dangling_services() {
        let x: DanglingCheckerResults = [
            DanglingCheckerResult::new(5, "foo::bar"),
            DanglingCheckerResult::new(42, "foo::baz"),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            "Some instances outlived their ServiceProvider: [foo::bar (remaining 5), foo::baz (remaining 42)]",
            format!("{:?}", LifetimeError::new(OutlivedLifetimeErrorVariants::SharedServices(x)))
        );
    }

    #[deny(improper_ctypes_definitions)]
    #[allow(dead_code)]
    pub extern "C" fn assert_stable_abi(_i: LifetimeError, _c: ServiceCollection) {}
}
