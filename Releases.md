# 0.1.0
- Make Service-Identification exchangeable, to allow stable identifiers ([TypeId hashes and ordering will vary between releases](https://doc.rust-lang.org/std/any/struct.TypeId.html))
- Breaking changes:
  - BuildError::MissingDependency { id } is the id of `T` instead of `Registered<T>`
  - ServiceIterator uses generic parameter `T` instead of `Registered<T>`
  - ErrorHandling:
    - ErrorHandler is ffi-safe, so dylibs can inherit errorhandlers of executables
    - Error-Messages changed. Added information, that they are volatile and should only be used for debugging purpose

# 0.0.1
Initial version which is not FFI safe yet