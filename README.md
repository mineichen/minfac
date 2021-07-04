# Lightweight Inversion of control
``` rust
use {ioc_rs::{Registered, ServiceCollection}};

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
- `#[no_std]`
