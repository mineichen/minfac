use core::any::type_name;

use abi_stable::{
    std_types::RResult::{RErr, ROk},
    DynTrait,
};

use crate::{
    resolvable::SealedResolvable,
    strategy::{Identifyable, Strategy},
    untyped::{AutoFreePointer, UntypedFn},
    AliasBuilder, AnyPtr, GenericServiceCollection, InternalBuildResult, Resolvable,
    ServiceProducer, ServiceProvider, UntypedFnFactory, UntypedFnFactoryContext,
};

/// Implemented for fn(), fn(T), fn(T, T) etc. to allow using the same method on ServiceProvider
pub trait Registrar<TS: Strategy> {
    type Item;
    fn register(
        self,
        collection: &mut GenericServiceCollection<TS>,
    ) -> AliasBuilder<Self::Item, TS>;
}

impl<TS: Strategy, T: Identifyable<TS::Id>> Registrar<TS> for fn() -> T {
    type Item = T;

    fn register(
        self,
        collection: &mut GenericServiceCollection<TS>,
    ) -> AliasBuilder<Self::Item, TS> {
        extern "C" fn factory<T: Identifyable<TS::Id>, TS: Strategy + 'static>(
            stage_1_data: AutoFreePointer,
            _ctx: &mut UntypedFnFactoryContext<TS>,
        ) -> InternalBuildResult<TS> {
            extern "C" fn func<T: Identifyable<TS::Id>, TS: Strategy + 'static>(
                _: *const ServiceProvider<TS>,
                stage_2_data: *const AutoFreePointer,
            ) -> T {
                let stage_2_data = unsafe { &*stage_2_data as &AutoFreePointer };
                let ptr = stage_2_data.get_pointer();
                let creator: fn() -> T = unsafe { core::mem::transmute(ptr) };
                creator()
            }
            ROk(UntypedFn::create(func::<T, TS>, stage_1_data))
        }

        let factory = UntypedFnFactory::no_alloc(self as AnyPtr, factory::<T, TS>);
        collection
            .producer_factories
            .push(ServiceProducer::<TS>::new::<T>(factory));

        AliasBuilder::new(collection)
    }
}

impl<TS: Strategy + 'static, T: Identifyable<TS::Id>, TDep: Resolvable<TS> + 'static> Registrar<TS>
    for fn(TDep) -> T
{
    type Item = T;

    fn register(
        self,
        collection: &mut GenericServiceCollection<TS>,
    ) -> AliasBuilder<Self::Item, TS> {
        type InnerContext<TDep, TS> = (<TDep as SealedResolvable<TS>>::PrecheckResult, AnyPtr);
        extern "C" fn factory<
            T: Identifyable<TS::Id>,
            TDep: Resolvable<TS> + 'static,
            TS: Strategy + 'static,
        >(
            outer_ctx: AutoFreePointer, // No-Alloc
            ctx: &mut UntypedFnFactoryContext<TS>,
        ) -> InternalBuildResult<TS> {
            let key = match TDep::precheck(ctx.final_ordered_types) {
                Ok(x) => x,
                Err(x) => return RErr(x.into()),
            };
            let data = TDep::iter_positions(ctx.final_ordered_types);
            ctx.register_cyclic_reference_candidate(
                type_name::<TDep::ItemPreChecked>(),
                DynTrait::from_value(data),
            );
            extern "C" fn func<
                T: Identifyable<TS::Id>,
                TDep: Resolvable<TS> + 'static,
                TS: Strategy + 'static,
            >(
                provider: *const ServiceProvider<TS>,
                outer_ctx: *const AutoFreePointer,
            ) -> T {
                let provider = unsafe { &*provider as &ServiceProvider<TS> };
                let outer_ctx = unsafe { &*outer_ctx as &AutoFreePointer };
                let (key, c): &InnerContext<TDep, TS> =
                    unsafe { &*(outer_ctx.get_pointer() as *const InnerContext<TDep, TS>) };
                let creator: fn(TDep) -> T = unsafe { std::mem::transmute(*c) };
                let arg = TDep::resolve_prechecked_self(provider, key);
                creator(arg)
            }
            let inner: InnerContext<TDep, TS> = (key, outer_ctx.get_pointer());
            ROk(UntypedFn::create(
                func::<T, TDep, TS>,
                AutoFreePointer::boxed(inner),
            ))
        }
        let factory = UntypedFnFactory::no_alloc(self as AnyPtr, factory::<T, TDep, TS>);
        collection
            .producer_factories
            .push(ServiceProducer::<TS>::new::<T>(factory));

        AliasBuilder::new(collection)
    }
}
