# Lightweight Inversion of control
This library is inspired by .Net's Microsoft.Extensions.DependencyInjection framework.

# Features
- Register Types/Traits which are not part of your crate (e.g. std::*). No macros needed.
- Service registration from dynamic libraries. see `examples/` for more details
- Service discovery, e.g. service_provider.get::<TransientServices<i32>>() returns an iterator of all registered i32
- Singleton-Services without reference counting. If borrowed data doesn't implement Copy/Clone, user has no chance to leak those types (currently has a bug, see tests/compile_errors/use_singleton_with_ref_after_drop.rs).
- No needless unwrap on Options<T>. When building ServiceProviders from ServiceCollections, all dependencies from registered Singleton<T> and Transient<T> are checked for their existence. 
- Fail early. If just a single dependency misses, `build()` fails
- #[no_std] with only `extern crate alloc;` will be possible to be widely usable

# Resolvables
Resolvables represent a type, which a ServiceProvider can resolve to an instance. 

# Dynamic Resolvables
Dynamic resolvables are registered at runtime and can be registered multiple times. If multiple services are registered for the same type, the last registration will be used to resolve a request. It is also possible, to get all registered services as an Iterator.

## Singleton
Dynamic service descriptor, created by calling `ServiceCollection::register_singleton(p)`. Getting a instance by e.g. `ServiceProvider::get::<Singleton<i32>()` uses function pointer `p` to lazy-generate an instance which is then shared between all calls. In this case, a `Option<&i32>` is returned, which lifes as long as the ServiceProvider it's received from. All registered can be received with `ServiceProvider::get::<SingletonServices<i32>() -> impl Iterator<Item=&i32>`.

## Transient
Dynamic service descriptors, created by calling `ServiceCollection::register_transient(p)`. Every time, an instance is requested, a new instance is generated from `p`. This instance, in contrast to singletons, is returned by value (e.g. `Option<i32>`). All registered can be received with `ServiceProvider.get::<TransientServices<i32>() -> impl Iterator<Item=i32>`.

## Constant
Some types are resolvable without a registration call. They are often used for service-dependencies.
 - Tuple of Resolvables (e.g. `ServiceProvider.get::<(Singleton<i32>, Transient<i32>)>()`) to resolve multiple services at once
 - ServiceProvider resolves to a reference to itself, e.g. if the requested type requires e.g. just a `Option<ILog>` if one is registered or if a service uses the ServiceProvider itself.

## Todo
- Add generic State to ServiceProvider & ServiceCollection to allow proper initialization 
  (e.g. a web-framework unconditionally has a Request-Instance on which many services might depend on)
- ServiceProvider hierarchy for ScopedServices (e.g. Each api-request gets it's own ServiceProvider with a common parent ServiceProvider)