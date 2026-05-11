# 0.1.6

- Remove all usages of `stable_abi`, as some of it's dependencies are outdated/unmaintained and cause long build times
- Breaking: Turn `pub ServiceProviderFactory::create` into `pub(crate) ServiceProviderFactory::create_internal`. This internal API accidentally became public in past releases.
  In the process of removing stable_abi for builds without `abi_stable`-feature, we have to get rid of this function, because it accepts a `abi_stable::std_types::RVec`.
  This is not expected to be used in the wild, but if you rely on it, activate the `abi_stable` feature to make it available and open a ticket on github to talk about it's deprecation.

# 0.1.5

- Add new method WeakServiceProvider::has to keep symmetry with ServiceProvider

# 0.1.4

- Add new method ServiceProvider::has to allow checking, if a type was registered
- Implement core::fmt::Display, core::error::Error and core::clone::Clone for BuildError

# 0.1.3

- Add new method ServiceProvider::resolve to allow the closure to specify dependencies

# 0.1.2

- Add new method ServiceCollection::register_with to allow the closure to specify dependencies
- Allow registration of shared services without std::sync::Arc
- Nested shared dependencies with ShareInner-Trait (No more need to wrap Outer Struct in Arc on register_shared)

# 0.1.1

- Error-Handler, which defaults to panicking, uses "C-unwind" instead of "C" to allow panicking

# 0.1.0

- Make Service-Identification exchangeable, to allow stable identifiers ([TypeId hashes and ordering will vary between releases](https://doc.rust-lang.org/std/any/struct.TypeId.html))
- Start removing trait objects to work toward ABI-Stability
- Fix code which doesn't compile with rust 1.81
- Remove once_cell dependency
- Breaking changes:
  - BuildError::MissingDependency { id } is the id of `T` instead of `Registered<T>`
  - ServiceIterator uses generic parameter `T` instead of `Registered<T>`
  - ErrorHandling:
    - ErrorHandler is ffi-safe, so dylibs can inherit errorhandlers of executables
    - Error-Messages changed. Added information, that they are volatile and should only be used for debugging purpose

# 0.0.1

Initial version which is not FFI safe yet
