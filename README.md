# Lightweight Inversion Of Control
``` rust
use {minfac::{Registered, ServiceCollection}};

let mut collection = ServiceCollection::new();
collection
    .with::<Registered<u8>>()
    .register(|byte| byte as i16 * 2);
collection.register(|| 1u8);
let provider = collection.build().expect("Configuration is valid");

assert_eq!(Some(2i16), provider.get::<i16>());

```
# Features
- Register Types/Traits which are not part of your crate (e.g. std::*). No macros needed.
- Service registration from separately compiled dynamic libraries. see `examples/distributed_simple` for more details
- Transient services are retrieved as `T` without any additional frills, SharedServices as `Arc<T>`
- Inheritance instead of scoped services (Service requests can be delegated to parent `ServiceProvider`s)
- Service discovery, (`provider.get_all::<MyService>()` returns an iterator, which lazily generates all registered `MyService` instances)
- Fail fast. When building a `ServiceProvider` all registered services are checked to
  - have all dependencies
  - contain no dependency-cycles
- Common pitfalls of traditional IOC are prevented by design
  - Singleton services cannot reference scoped services, as scoped services don't exist
  - Shared services cannot outlive their `ServiceProvider` (checked at runtime when debug_assertions are enabled)
- `ServiceProvider` implements Send+Sync and is threadsafe without using locks

Visit the examples/documentation for more details
## Required Tasks for stable release
✅ Transient services - New instance per request  
✅ Shared services - Shared instance per ServiceProvider  
✅ Instance services - Shared instance per ServiceProviderFactory  

✅ ServiceProviderFactory for creating ServiceProvider's with minimal overhead  
✅ ServiceProviderFactory inherit services from ServiceProvider  
⬜ ServiceProviderFactory inherit services from other ServiceProviderFactory  

✅ Replaceable strategy for service identification (TypeIds might change between rust versions)  
✅ Recursive dependencies check  
✅ Missing dependencies check  
⬜ Make all structs FFI-Safe  
⬜ Remove global Error handler. Use transition from ServiceCollection to ServiceProvider to replace default Error-Hander
