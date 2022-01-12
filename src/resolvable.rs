use super::*;
use crate::{
    service_provider::{ServiceProvider, WeakServiceProvider},
    strategy::{Identifyable, Strategy},
};
use core::{
    iter::{empty, once, Chain, Empty, Once},
    ops::Range,
};

/// Represents anything resolvable by a ServiceProvider.
pub trait Resolvable<TS: Strategy = AnyStrategy>: SealedResolvable<TS> {}

// Sealed, because resolvable module is not pub (Resolvable is reexported in lib.rs)
// Because this mod is not public, external code cannot call these methods but only reference the type
pub trait SealedResolvable<TS: Strategy> {
    /// Used if it's uncertain, wether a type is initializable, e.g.
    /// - Option<i32> for provider.get<Singleton<i32>>()
    type Item;
    /// If a resolvable is used as a dependency for another service, ServiceCollection::build() ensures
    /// that the dependency can be initialized. So it's currently used:
    /// - provider.get<SingletonServices<i32>>() -> EmptyIterator if nothing is registered
    /// - collection.with::<Singleton<i32>>().register_singleton(|_prechecked_i32: i32| {})
    type ItemPreChecked;

    type PrecheckResult;
    type TypeIdsIter: Iterator<Item = usize>;

    /// Resolves a type with the specified provider. There might be multiple calls to this method with
    /// parent ServiceProviders. It will therefore not necessarily be an alias for provider.get() in the future.
    fn resolve(provider: &ServiceProvider<TS>) -> Self::Item;

    /// Called internally when resolving dependencies.
    fn resolve_prechecked(
        provider: &ServiceProvider<TS>,
        key: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked;

    fn precheck(ordered_types: &[TS::Id]) -> Result<Self::PrecheckResult, BuildError<TS>>;

    // Iterates all positions involved in resolving the type. This is required for checking
    // missing or cyclic dependencies
    fn iter_positions(types: &[TS::Id]) -> Self::TypeIdsIter;
}

impl<TS: Strategy> SealedResolvable<TS> for () {
    type Item = ();
    type ItemPreChecked = ();
    type PrecheckResult = ();
    type TypeIdsIter = Empty<usize>;

    fn resolve(_: &ServiceProvider<TS>) -> Self::Item {}
    fn resolve_prechecked(
        _: &ServiceProvider<TS>,
        _: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
    }

    fn precheck(_ordered_types: &[TS::Id]) -> Result<Self::PrecheckResult, BuildError<TS>> {
        Ok(())
    }

    fn iter_positions(_: &[TS::Id]) -> Self::TypeIdsIter {
        empty()
    }
}
impl<TS: Strategy> Resolvable<TS> for () {}

impl<TS: Strategy, T0: Resolvable<TS>, T1: Resolvable<TS>> SealedResolvable<TS> for (T0, T1) {
    type Item = (T0::Item, T1::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked);
    type PrecheckResult = (T0::PrecheckResult, T1::PrecheckResult);
    type TypeIdsIter = Chain<T0::TypeIdsIter, T1::TypeIdsIter>;

    fn resolve(provider: &ServiceProvider<TS>) -> Self::Item {
        (provider.resolve::<T0>(), provider.resolve::<T1>())
    }

    fn resolve_prechecked(
        provider: &ServiceProvider<TS>,
        key: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        (
            T0::resolve_prechecked(provider, &key.0),
            T1::resolve_prechecked(provider, &key.1),
        )
    }

    fn precheck(ordered_types: &[TS::Id]) -> Result<Self::PrecheckResult, BuildError<TS>> {
        let r0 = T0::precheck(ordered_types)?;
        let r1 = T1::precheck(ordered_types)?;
        Ok((r0, r1))
    }

    fn iter_positions(types: &[TS::Id]) -> Self::TypeIdsIter {
        T0::iter_positions(types).chain(T1::iter_positions(types))
    }
}
impl<TS: Strategy, T0: Resolvable<TS>, T1: Resolvable<TS>> Resolvable<TS> for (T0, T1) {}

impl<TS: Strategy, T0: Resolvable<TS>, T1: Resolvable<TS>, T2: Resolvable<TS>> SealedResolvable<TS>
    for (T0, T1, T2)
{
    type Item = (T0::Item, T1::Item, T2::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked, T2::ItemPreChecked);
    type PrecheckResult = (T0::PrecheckResult, T1::PrecheckResult, T2::PrecheckResult);
    type TypeIdsIter = Chain<Chain<T0::TypeIdsIter, T1::TypeIdsIter>, T2::TypeIdsIter>;

    fn resolve(provider: &ServiceProvider<TS>) -> Self::Item {
        (
            provider.resolve::<T0>(),
            provider.resolve::<T1>(),
            provider.resolve::<T2>(),
        )
    }
    fn resolve_prechecked(
        provider: &ServiceProvider<TS>,
        key: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        (
            T0::resolve_prechecked(provider, &key.0),
            T1::resolve_prechecked(provider, &key.1),
            T2::resolve_prechecked(provider, &key.2),
        )
    }

    fn precheck(ordered_types: &[TS::Id]) -> Result<Self::PrecheckResult, BuildError<TS>> {
        let r0 = T0::precheck(ordered_types)?;
        let r1 = T1::precheck(ordered_types)?;
        let r2 = T2::precheck(ordered_types)?;
        Ok((r0, r1, r2))
    }

    fn iter_positions(types: &[TS::Id]) -> Self::TypeIdsIter {
        T0::iter_positions(types)
            .chain(T1::iter_positions(types))
            .chain(T2::iter_positions(types))
    }
}
impl<TS: Strategy, T0: Resolvable<TS>, T1: Resolvable<TS>, T2: Resolvable<TS>> Resolvable<TS>
    for (T0, T1, T2)
{
}

impl<
        TS: Strategy,
        T0: Resolvable<TS>,
        T1: Resolvable<TS>,
        T2: Resolvable<TS>,
        T3: Resolvable<TS>,
    > SealedResolvable<TS> for (T0, T1, T2, T3)
{
    type Item = (T0::Item, T1::Item, T2::Item, T3::Item);
    type ItemPreChecked = (
        T0::ItemPreChecked,
        T1::ItemPreChecked,
        T2::ItemPreChecked,
        T3::ItemPreChecked,
    );
    type PrecheckResult = (
        T0::PrecheckResult,
        T1::PrecheckResult,
        T2::PrecheckResult,
        T3::PrecheckResult,
    );
    #[allow(clippy::type_complexity)]
    type TypeIdsIter =
        Chain<Chain<Chain<T0::TypeIdsIter, T1::TypeIdsIter>, T2::TypeIdsIter>, T3::TypeIdsIter>;

    fn resolve(provider: &ServiceProvider<TS>) -> Self::Item {
        (
            provider.resolve::<T0>(),
            provider.resolve::<T1>(),
            provider.resolve::<T2>(),
            provider.resolve::<T3>(),
        )
    }
    fn resolve_prechecked(
        provider: &ServiceProvider<TS>,
        key: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        (
            T0::resolve_prechecked(provider, &key.0),
            T1::resolve_prechecked(provider, &key.1),
            T2::resolve_prechecked(provider, &key.2),
            T3::resolve_prechecked(provider, &key.3),
        )
    }

    fn precheck(ordered_types: &[TS::Id]) -> Result<Self::PrecheckResult, BuildError<TS>> {
        let r0 = T0::precheck(ordered_types)?;
        let r1 = T1::precheck(ordered_types)?;
        let r2 = T2::precheck(ordered_types)?;
        let r3 = T3::precheck(ordered_types)?;
        Ok((r0, r1, r2, r3))
    }

    fn iter_positions(types: &[TS::Id]) -> Self::TypeIdsIter {
        T0::iter_positions(types)
            .chain(T1::iter_positions(types))
            .chain(T2::iter_positions(types))
            .chain(T3::iter_positions(types))
    }
}
impl<
        TS: Strategy,
        T0: Resolvable<TS>,
        T1: Resolvable<TS>,
        T2: Resolvable<TS>,
        T3: Resolvable<TS>,
    > Resolvable<TS> for (T0, T1, T2, T3)
{
}

impl<TS: Strategy> SealedResolvable<TS> for WeakServiceProvider<TS> {
    // Doesn't make sense to call from the outside
    type Item = ();
    type ItemPreChecked = Self;
    type PrecheckResult = ();
    type TypeIdsIter = Empty<usize>;

    fn resolve(_provider: &ServiceProvider<TS>) -> Self::Item {}

    fn resolve_prechecked(provider: &ServiceProvider<TS>, _: &()) -> Self::ItemPreChecked {
        provider.into()
    }

    fn precheck(_: &[TS::Id]) -> Result<Self::PrecheckResult, BuildError<TS>> {
        Ok(())
    }

    fn iter_positions(_types: &[TS::Id]) -> Self::TypeIdsIter {
        empty()
    }
}
impl<TS: Strategy> Resolvable<TS> for WeakServiceProvider<TS> {}

/// pos must be a valid index in provider.producers
pub(crate) unsafe fn resolve_unchecked<TS: Strategy, T: Identifyable<TS::Id>>(
    provider: &ServiceProvider<TS>,
    pos: usize,
) -> T {
    let entry = provider.get_producers().get_unchecked(pos);
    debug_assert_eq!(entry.get_result_type_id(), &T::get_id());
    entry.execute::<T>(provider)
}

impl<TS: Strategy, T: Identifyable<TS::Id>> SealedResolvable<TS> for AllRegistered<T> {
    type Item = ServiceIterator<T, TS>;
    type ItemPreChecked = ServiceIterator<T, TS>;
    type PrecheckResult = ();
    type TypeIdsIter = Range<usize>;

    fn resolve(provider: &ServiceProvider<TS>) -> Self::Item {
        let next_pos = binary_search::binary_search_first_by_key(
            provider.get_producers(),
            &T::get_id(),
            |f| f.get_result_type_id(),
        );
        ServiceIterator::new(provider.into(), next_pos)
    }

    fn resolve_prechecked(
        provider: &ServiceProvider<TS>,
        _: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        Self::resolve(provider)
    }

    fn precheck(_: &[TS::Id]) -> Result<Self::PrecheckResult, BuildError<TS>> {
        // Todo: Implement to avoid lookup during service resolution
        Ok(())
    }

    fn iter_positions(types: &[TS::Id]) -> Self::TypeIdsIter {
        let id = T::get_id();
        let first = binary_search::binary_search_first_by_key(types, &id, |f| f);

        match first {
            Some(x) => {
                let to = binary_search::binary_search_last_by_key(&types[x..], &id, |f| f).unwrap()
                    + x
                    + 1;
                x..to
            }
            None => 0..0,
        }
    }
}
impl<TS: Strategy, T: Identifyable<TS::Id>> Resolvable<TS> for AllRegistered<T> {}

impl<TS: Strategy, T: Identifyable<TS::Id>> SealedResolvable<TS> for Registered<T> {
    type Item = Option<T>;
    type ItemPreChecked = T;
    type PrecheckResult = usize;
    type TypeIdsIter = Once<usize>;

    fn resolve(provider: &ServiceProvider<TS>) -> Self::Item {
        binary_search::binary_search_last_by_key(provider.get_producers(), &T::get_id(), |f| {
            f.get_result_type_id()
        })
        .map(|index| unsafe { resolve_unchecked::<TS, Self::ItemPreChecked>(provider, index) })
    }

    fn resolve_prechecked(
        provider: &ServiceProvider<TS>,
        index: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        unsafe { resolve_unchecked::<TS, Self::ItemPreChecked>(provider, *index) }
    }

    fn precheck(producers: &[TS::Id]) -> Result<Self::PrecheckResult, BuildError<TS>> {
        binary_search::binary_search_last_by_key(producers, &T::get_id(), |f| f)
            .ok_or_else(BuildError::<TS>::new_missing_dependency::<T>)
    }

    fn iter_positions(types: &[TS::Id]) -> Self::TypeIdsIter {
        let position = binary_search::binary_search_last_by_key(
            types,
            &Self::ItemPreChecked::get_id(),
            |f| f
        ).expect("type be found. This shouldn't be possible, as MissingDependency should have been checked");
        once(position)
    }
}
impl<TS: Strategy, T: Identifyable<TS::Id>> Resolvable<TS> for Registered<T> {}
#[cfg(test)]
mod tests {
    use core::any::TypeId;
    use {super::*, alloc::vec};

    #[test]
    fn resolvable_services_iterate_services_test() {
        let mut types = vec![
            TypeId::of::<i32>(),
            TypeId::of::<i32>(),
            TypeId::of::<i64>(),
        ];
        types.sort();

        assert_eq!(
            2,
            <AllRegistered::<i32> as SealedResolvable<AnyStrategy>>::iter_positions(&types).count()
        );
        assert_eq!(
            1,
            <AllRegistered::<i64> as SealedResolvable<AnyStrategy>>::iter_positions(&types).count()
        );
        assert_eq!(
            0,
            <AllRegistered::<i128> as SealedResolvable<AnyStrategy>>::iter_positions(&types)
                .count()
        );
    }
}
