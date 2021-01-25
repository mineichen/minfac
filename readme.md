# Lightweight Inversion of control
This library is inspired by .Net's Microsoft.Extensions.DependencyInjection framework.

# Features
- Service registration from dynamic libraries. see `examples/` for more details
- Service discovery, e.g. service_provider.get::<TransientServices<i32>>() returns a iterator of all registered i32
- Singleton-Services without reference counting. If borrowed data doesn't implement Copy/Clone, user has no chance to introduce memory leaks
- Options. When building ServiceProviders from ServiceCollections, all dependencies from registered Singleton<T> and Transient<T> are checked for their existence. If just a single dependency misses, `build()` fails
- Register Types/Traits which are not part of your crate (e.g. std::*) 
- #[no_std] capable

# Resolvables
Resolvables represent a type, for which a ServiceProvider can provide instances. 

# Dynamic Resolvables
Dynamic resolvables are registered at runtime and can be registered multiple times. If multiple services are registered for the same type, the last registration will be used to resolve a request. It is however possible, to get all registered services.

## Singleton
Dynamic service descriptor, created by calling `ServiceCollection::register_singleton(p)`. Getting a instance by e.g. `serviceProvider.get::<Singleton<i32>()` uses registration argument `p` to lazy-generate an instance which is then shares between all calls. In this case, a `Option<&i32>` is returned, who lifes as long as the serviceProvider it's received from. All registered can be received by `ServiceProvider::get::<SingletonServices<i32>() -> impl Iterator<Item=&i32>`.

## Transient
Dynamic service descriptors, created by calling `ServiceCollection::register_transient(p)`. Every time, an instance is requested,
a new instance is generated from `p`. This instance is, in contrast to singleton, returned by value (e.g. `Option<i32>`). All registered can be received by `ServiceProvider.get::<TransientServices<i32>() -> impl Iterator<Item=i32>`.

## Constant
Some types are resolvable without a registration call. They are often used for service-dependencies.

 - Tuple of Resolvables (e.g. `ServiceProvider.get::<(Singleton<i32>, Transient<i32>)>()`) to resolve multiple service at once
 - ServiceProvider resolves to a reference to itself, e.g. if the requested type requires e.g. just a `Option<ILog>` if one is registered.

## Todo
- Add generic State to ServiceProvider & ServiceCollection to allow proper initialization (ServiceProvider for Request-Context unconditionally has a Request-Object attached)
- ServiceProvider hierarchy for ScopedServices (e.g. Each api-request gets it's own ServiceProvider with a common parent ServiceProvider)