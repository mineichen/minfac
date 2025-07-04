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
