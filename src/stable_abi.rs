use std::{ffi::CStr, hash::{Hasher, Hash}};

use abi_stable::{StableAbi, type_layout::{ModPath, TypeLayout}};

use crate::strategy::Identifyable;


#[derive(Debug)]
pub struct StableAbiStrategy {}

impl crate::Strategy for StableAbiStrategy {
    type Id = StableAbiTypeId;
}

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Clone, Copy, Hash)]
pub struct StableAbiTypeId {
    name: &'static str,
    version: &'static str,
    path: &'static CStr,
    child_name_hash: u64
}

impl<T: StableAbi + 'static> Identifyable<StableAbiTypeId> for T {
    fn get_id() -> StableAbiTypeId {
        get_layout_typeid(Self::LAYOUT)
    }
}

fn get_layout_typeid(layout: &'static TypeLayout) -> StableAbiTypeId {
    let path: ModPath = layout.mod_path();
    // Hack to access path: Both ModPath & it's inner NulStr have repr(transparent)
    // NulStr represents a immutable, static nullterminated string
    let path_str = unsafe {
        let path_trans: &*const i8 = std::mem::transmute(&path);
        CStr::from_ptr(*path_trans)
    };

    // TraitObjects in RBox and RArc have the same type_id otherwise
    let mut hasher = Djb2::default();
    hash_child_layout_names(&mut hasher, layout);
    
    StableAbiTypeId {
        name: layout.name(),
        version: layout.item_info().package_and_version().1,
        path: path_str,
        child_name_hash:  hasher.finish()
    }
}

// String hasher which will not change between between releases as the rust-Hasher might
struct Djb2(u64);
impl Default for Djb2 {
    fn default() -> Self {
        Self(5381)
    }
}
impl Hasher for Djb2 {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = ((self.0 << 5).overflowing_add(self.0)).0.overflowing_add(*byte as u64).0; /* hash * 33 + c */
        }
    }
}

fn hash_child_layout_names(hasher: &mut Djb2, layout: &TypeLayout) {
    if let Some(fields) = layout.get_fields() {
        for a in fields {
            let layout = a.layout();
            layout.name().hash(hasher);
            hash_child_layout_names(hasher, layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use abi_stable::{std_types::{RArc, RBox}, erased_types::TD_Opaque};
    use crate::{GenericServiceCollection, Registered};
    use super::*;

    #[test]
    fn assert_valid_path() {
        let expected = CString::new("minfac::stable_abi::tests").unwrap();
        assert_eq!(
            expected.as_c_str(),
            <Foo as Identifyable<StableAbiTypeId>>::get_id().path
        );
    }

    #[abi_stable::sabi_trait]
    trait BarStableAbi {
        fn get_no(&self) -> i32;
    }

    #[derive(StableAbi)]
    #[repr(C)]
    struct Foo{
        no: RArc<i32>
    }

    struct BarStableAbiImpl { no: i32 }
    impl BarStableAbi for BarStableAbiImpl {
        fn get_no(&self) -> i32 {
            self.no
        }
    }

    #[test]
    fn resolve_ffi_service() {
        let mut col = GenericServiceCollection::<StableAbiStrategy>::new();
        col.with::<Registered<BarStableAbi_TO<RArc<()>>>>().register(|no| Foo { no: RArc::new(no.get_no()) });
        col.register(|| BarStableAbi_TO::from_ptr(RArc::new(BarStableAbiImpl { no: 42}), TD_Opaque));
        let provider = col.build().expect("dependencies are ok");
        let foo: Option<Foo> = provider.get();
        assert_eq!(Some(42), foo.map(|x| *x.no))
    }

    #[test]
    fn should_raise_missing_dependency() {
        let mut col = GenericServiceCollection::<StableAbiStrategy>::new();
        col.with::<Registered<BarStableAbi_TO<RBox<()>>>>().register(|no| Foo { no: RArc::new(no.get_no()) });
        col.register(|| BarStableAbi_TO::from_ptr(RArc::new(BarStableAbiImpl { no: 42 }), TD_Opaque));        
        col.build().expect_err("should have missing dependency");
    }
}