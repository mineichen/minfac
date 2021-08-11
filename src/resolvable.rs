use {
    super::*,
    core::iter::{Chain, Empty},
};

/// Represents anything resolvable by a ServiceProvider. This
pub trait Resolvable: Any + SealedResolvable {}

pub trait SealedResolvable {
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
    fn resolve(provider: &ServiceProvider) -> Self::Item;

    /// Called internally when resolving dependencies.
    fn resolve_prechecked(
        provider: &ServiceProvider,
        key: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked;

    fn precheck(ordered_types: &[TypeId]) -> Result<Self::PrecheckResult, BuildError>;
    fn iter_positions(types: &[TypeId]) -> Self::TypeIdsIter;
}

impl SealedResolvable for () {
    type Item = ();
    type ItemPreChecked = ();
    type PrecheckResult = ();
    type TypeIdsIter = core::iter::Empty<usize>;

    fn resolve(_: &ServiceProvider) -> Self::Item {}
    fn resolve_prechecked(_: &ServiceProvider, _: &Self::PrecheckResult) -> Self::ItemPreChecked {}

    fn precheck(_ordered_types: &[TypeId]) -> Result<Self::PrecheckResult, BuildError> {
        Ok(())
    }

    fn iter_positions(_: &[TypeId]) -> Self::TypeIdsIter {
        core::iter::empty()
    }
}
impl Resolvable for () {}

impl<T0: Resolvable, T1: Resolvable> SealedResolvable for (T0, T1) {
    type Item = (T0::Item, T1::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked);
    type PrecheckResult = (T0::PrecheckResult, T1::PrecheckResult);
    type TypeIdsIter = Chain<T0::TypeIdsIter, T1::TypeIdsIter>;

    fn resolve(provider: &ServiceProvider) -> Self::Item {
        (provider.resolve::<T0>(), provider.resolve::<T1>())
    }

    fn resolve_prechecked(
        provider: &ServiceProvider,
        key: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        (
            T0::resolve_prechecked(provider, &key.0),
            T1::resolve_prechecked(provider, &key.1),
        )
    }

    fn precheck(ordered_types: &[TypeId]) -> Result<Self::PrecheckResult, BuildError> {
        let r0 = T0::precheck(ordered_types)?;
        let r1 = T1::precheck(ordered_types)?;
        Ok((r0, r1))
    }

    fn iter_positions(types: &[TypeId]) -> Self::TypeIdsIter {
        T0::iter_positions(types).chain(T1::iter_positions(types))
    }
}
impl<T0: Resolvable, T1: Resolvable> Resolvable for (T0, T1) {}

impl<T0: Resolvable, T1: Resolvable, T2: Resolvable> SealedResolvable for (T0, T1, T2) {
    type Item = (T0::Item, T1::Item, T2::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked, T2::ItemPreChecked);
    type PrecheckResult = (T0::PrecheckResult, T1::PrecheckResult, T2::PrecheckResult);
    type TypeIdsIter =
        core::iter::Chain<core::iter::Chain<T0::TypeIdsIter, T1::TypeIdsIter>, T2::TypeIdsIter>;

    fn resolve(provider: &ServiceProvider) -> Self::Item {
        (
            provider.resolve::<T0>(),
            provider.resolve::<T1>(),
            provider.resolve::<T2>(),
        )
    }
    fn resolve_prechecked(
        provider: &ServiceProvider,
        key: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        (
            T0::resolve_prechecked(provider, &key.0),
            T1::resolve_prechecked(provider, &key.1),
            T2::resolve_prechecked(provider, &key.2),
        )
    }

    fn precheck(ordered_types: &[TypeId]) -> Result<Self::PrecheckResult, BuildError> {
        let r0 = T0::precheck(ordered_types)?;
        let r1 = T1::precheck(ordered_types)?;
        let r2 = T2::precheck(ordered_types)?;
        Ok((r0, r1, r2))
    }

    fn iter_positions(types: &[TypeId]) -> Self::TypeIdsIter {
        T0::iter_positions(types)
            .chain(T1::iter_positions(types))
            .chain(T2::iter_positions(types))
    }
}
impl<T0: Resolvable, T1: Resolvable, T2: Resolvable> Resolvable for (T0, T1, T2) {}

impl<T0: Resolvable, T1: Resolvable, T2: Resolvable, T3: Resolvable> SealedResolvable
    for (T0, T1, T2, T3)
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

    fn resolve(provider: &ServiceProvider) -> Self::Item {
        (
            provider.resolve::<T0>(),
            provider.resolve::<T1>(),
            provider.resolve::<T2>(),
            provider.resolve::<T3>(),
        )
    }
    fn resolve_prechecked(
        provider: &ServiceProvider,
        key: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        (
            T0::resolve_prechecked(provider, &key.0),
            T1::resolve_prechecked(provider, &key.1),
            T2::resolve_prechecked(provider, &key.2),
            T3::resolve_prechecked(provider, &key.3),
        )
    }

    fn precheck(ordered_types: &[TypeId]) -> Result<Self::PrecheckResult, BuildError> {
        let r0 = T0::precheck(ordered_types)?;
        let r1 = T1::precheck(ordered_types)?;
        let r2 = T2::precheck(ordered_types)?;
        let r3 = T3::precheck(ordered_types)?;
        Ok((r0, r1, r2, r3))
    }

    fn iter_positions(types: &[TypeId]) -> Self::TypeIdsIter {
        T0::iter_positions(types)
            .chain(T1::iter_positions(types))
            .chain(T2::iter_positions(types))
            .chain(T3::iter_positions(types))
    }
}
impl<T0: Resolvable, T1: Resolvable, T2: Resolvable, T3: Resolvable> Resolvable
    for (T0, T1, T2, T3)
{
}

impl SealedResolvable for WeakServiceProvider {
    // Doesn't make sense to call from the outside
    type Item = ();
    type ItemPreChecked = Self;
    type PrecheckResult = ();
    type TypeIdsIter = Empty<usize>;

    fn resolve(_provider: &ServiceProvider) -> Self::Item {}

    fn resolve_prechecked(provider: &ServiceProvider, _: &()) -> Self::ItemPreChecked {
        provider.into()
    }

    fn precheck(_: &[TypeId]) -> Result<Self::PrecheckResult, BuildError> {
        Ok(())
    }

    fn iter_positions(_types: &[TypeId]) -> Self::TypeIdsIter {
        core::iter::empty()
    }
}
impl Resolvable for WeakServiceProvider {}

/// pos must be a valid index in provider.producers
unsafe fn resolve_unchecked<T: resolvable::Resolvable>(
    provider: &ServiceProvider,
    pos: usize,
) -> T::ItemPreChecked {
    ({
        let entry = provider.immutable_state.producers.get_unchecked(pos);
        debug_assert_eq!(entry.get_result_type_id(), &TypeId::of::<T>());
        entry.borrow_for::<T::ItemPreChecked>()
    })(&provider)
}

impl<'a, T: resolvable::Resolvable> core::iter::Iterator for ServiceIterator<T> {
    type Item = T::ItemPreChecked;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_pos.map(|i| {
            self.next_pos = if let Some(next) = self.provider.0.immutable_state.producers.get(i + 1)
            {
                if next.get_result_type_id() == &TypeId::of::<T>() {
                    Some(i + 1)
                } else {
                    None
                }
            } else {
                None
            };

            unsafe { resolve_unchecked::<T>(&self.provider.0, i) }
        })
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.next_pos.map(|i| {
            // If has_next, last must exist
            let pos = binary_search::binary_search_last_by_key(
                &self.provider.0.immutable_state.producers[i..],
                &TypeId::of::<T>(),
                |f| &f.get_result_type_id(),
            )
            .unwrap();
            unsafe { resolve_unchecked::<T>(&self.provider.0, i + pos) }
        })
    }
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.next_pos
            .map(|i| {
                let pos = binary_search::binary_search_last_by_key(
                    &self.provider.0.immutable_state.producers[i..],
                    &TypeId::of::<T>(),
                    |f| &f.get_result_type_id(),
                )
                .unwrap();
                pos + 1
            })
            .unwrap_or(0)
    }
}

impl<T: Any> SealedResolvable for AllRegistered<T> {
    type Item = ServiceIterator<Registered<T>>;
    type ItemPreChecked = ServiceIterator<Registered<T>>;
    type PrecheckResult = ();
    type TypeIdsIter = core::ops::Range<usize>;

    fn resolve(provider: &ServiceProvider) -> Self::Item {
        let next_pos = binary_search::binary_search_first_by_key(
            &provider.immutable_state.producers,
            &TypeId::of::<Registered<T>>(),
            |f| &f.get_result_type_id(),
        );
        ServiceIterator {
            provider: provider.into(),
            item_type: PhantomData,
            next_pos,
        }
    }

    fn resolve_prechecked(
        provider: &ServiceProvider,
        _: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        Self::resolve(provider)
    }

    fn precheck(_: &[TypeId]) -> Result<Self::PrecheckResult, BuildError> {
        Ok(())
    }

    fn iter_positions(types: &[TypeId]) -> Self::TypeIdsIter {
        let first =
            binary_search::binary_search_first_by_key(types, &TypeId::of::<Registered<T>>(), |f| {
                &f
            });

        match first {
            Some(x) => {
                let to = binary_search::binary_search_last_by_key(
                    &types[x..],
                    &TypeId::of::<Registered<T>>(),
                    |f| &f,
                )
                .unwrap()
                    + x
                    + 1;
                x..to
            }
            None => 0..0,
        }
    }
}
impl<T: Any> Resolvable for AllRegistered<T> {}

impl<T: Any> SealedResolvable for Registered<T> {
    type Item = Option<T>;
    type ItemPreChecked = T;
    type PrecheckResult = usize;
    type TypeIdsIter = core::iter::Once<usize>;

    fn resolve(provider: &ServiceProvider) -> Self::Item {
        binary_search::binary_search_last_by_key(
            &provider.immutable_state.producers,
            &TypeId::of::<Self>(),
            |f| &f.get_result_type_id(),
        )
        .map(|index| unsafe { resolve_unchecked::<Self>(provider, index) })
    }

    fn resolve_prechecked(
        provider: &ServiceProvider,
        index: &Self::PrecheckResult,
    ) -> Self::ItemPreChecked {
        unsafe { resolve_unchecked::<Self>(provider, *index) }
    }

    fn precheck(producers: &[TypeId]) -> Result<Self::PrecheckResult, BuildError> {
        binary_search::binary_search_last_by_key(&producers, &TypeId::of::<Self>(), |f| &f)
            .ok_or_else(|| BuildError::new_missing_dependency::<Self>())
    }

    fn iter_positions(types: &[TypeId]) -> Self::TypeIdsIter {
        let position = binary_search::binary_search_last_by_key(
            types,
            &TypeId::of::<Self>(),
            |f| &f
        ).expect("Type not found. This shouldn't be possible, as MissingDependency should have been checked");
        core::iter::once(position)
    }
}
impl<T: Any> Resolvable for Registered<T> {}
#[cfg(test)]
mod tests {
    use {super::*, alloc::vec};

    #[test]
    fn resolvable_services_iterate_services_test() {
        let mut types = vec![
            TypeId::of::<Registered<i32>>(),
            TypeId::of::<Registered<i32>>(),
            TypeId::of::<Registered<i64>>(),
        ];
        types.sort();

        assert_eq!(2, AllRegistered::<i32>::iter_positions(&types).count());
        assert_eq!(1, AllRegistered::<i64>::iter_positions(&types).count());
        assert_eq!(0, AllRegistered::<i128>::iter_positions(&types).count());
    }
}
