# Plugin based architecture with RUST

## Introduction
In an excellent [Talk](https://youtu.be/2dKZ-dWaCiU?t=1158), Uncle Bob complains, that the first thing he noticed when reviewing an application of his son was the fact that it was a RAILS app. He criticised that RAILS, a popular web framework, should remain in the background, as it is just an abstraction over the concept of an IO device, the web.

"The first thing I should see ought to be the reason the system exists... The web is an IO device, and the one thing we learned back in the 1960s was, that we didn't want to know what IO device we are using." Uncle Bob

Following this advice, folders should be named after domain functionality like /cart/add rather than technical descriptions like /persister/cart. Plugin based architecture takes this approach one step further and introduces a dedicated plugin project per feature group. If these projects are integrated as dynamic libraries, features and platform can be released independently as long as everything is built with the same RUSTC version ([Rust does not have a stable ABI](https://people.gnome.org/~federico/blog/rust-stable-abi.html)). Beside features, cross cutting concerns like database access, http-routes, logging or running background tasks could be implemented as reusable plugins too. 

Using a plugin architecture, 
 - Reduces coupling of features to the infrastructure. Files which e.g. know about all http routes or all database migrations violate the [open closed principle](https://de.wikipedia.org/wiki/Open-Closed-Prinzip)
 - allows extensions to be installed on demand to safe space for unused functionality or to protect licensed features
 - makes a rebuild of the entire executable obsolete. This results in significantly faster compile times. See experiment bellow.
 - allows sharing infallible interfaces. In contrast, microservices usually communicate over a network which adds performance overhead and forces callers to handle potential communicaion errors and therefore increases the overall complexity.

 In a plugin architecture, plugins can extend the main application with 
 - Custom UI-Elements 
 - HTTP-Endpoints
 - Background processes
 - CLI-commands 
 - ...

When composing these extensions, one plugin might require functionality from other plugins. E.g. the http endpoint of a feature requires a database connection which might be shared among many plugins. Therefore, the executable application needs a mechanism to link those components together. 

# Obstacles for implementations in Rust 

I remember learning the PHP framework "Symfony2" back in 2011. This was the first framework I've ever seen, whoes core was composed of multiple, independently usable libraries. It even allows replaceing functionality of the core without complicated factory code by overriding the default implementation of an interface with a custom class within the IOC (Inversion of Control) container. Asp.Net Core is useing a similar architecture. In both cases, IOC containers play a crucial role to provide maximal flexibility.

Unfortunately, there was no IOC container available in Rust which supported more advanced features like
 - Services with different lifetimes (a cache lives longer than a HTTP-Request)
 - Early checks at runtime, whether all dependencies of registered services are resolvable
 - Getting all registered services of a type
 - Implement Send+Sync to be threadsafe
 - Using stable Rust

This is why I recently released the first version of [minfac](https://crates.io/crates/minfac) on crates.io. Not only does it provide the mentioned features, but it also eliminates weaknesses frequently stumbled on in other languages like [scope validation](https://docs.microsoft.com/en-us/aspnet/core/fundamentals/dependency-injection?view=aspnetcore-5.0#scope-validation). 

IOC containers solve both challenges of linking implementations of services and service discovery in one thin layer. The following section intends to give a rough overview without going into code too much. 

# Implementation in Rust using minfac
Creating a microframework on top of minfac is straight forward. The following sections illustrate the takeaways when building the [prototype](https://github.com/mineichen/minfac/tree/main/examples/distributed_web). The workspace has the following structure:
 - raf-core: Core infrastructure. It currently only contains the trait `HostedService` without any implementation. The intent of this trait is explained bellow.
 - raf-sql: Registers a SQLite connection, which can be used by other plugins
 - raf-web: Registers a `HostedService` to run a Webserver. It uses the IOC container to detect routes registered by other plugins.
 - runtime: The executable project. The startup sequence is explained bellow. It has dependencies to all projects prefixed with `raf`.
 - todo: Sample plugin. It creates all necessary sql tables on startup and registers http endpoints, which are loading data from sqlite. Notice that no other project, not even the runtime, depends on this project.

Projects prefixed with `raf` define and implement cross cutting concerns and can easily be reused in other projects. The Todo project compiles into a dynamic library and has exactly one public symbol: The register-function. Within this function, a plugin registers various services, which are discoverable and can be used by other plugins or the runtime. The following example illustrates the scenario of a plugin registering an `i32` as a service based on a `u8`, which must be provided by another plugin. If noone registers a `u8`, the application won't be able to start. Other plugins can require the `i32` provided by this plugin in the same way as this plugin requires a registered `u8`. `raf-*` projects have a similar function, but don't declare it as `extern "C"`, because they're statically linked with the executable project.

```rust
#[no_mangle]
pub extern "C" fn register(collection: &mut ServiceCollection) {
    collection
        .with::<u8>()
        .register(|byte| byte as i32 * 2);
}
```

The platform tries to find and call these symbols in all dynamic libraries within the plugins folder using the [libloading](https://crates.io/crates/libloading) crate. After each plugin registered all of its services, the platform tries to build the `ServiceProvider`, which can be used to resolve services. Building the `ServiceProvider` only works, if the dependencies of all registered services are fulfilled. Otherwise, a detailed error is provided to help with debugging. If all dependencies are met, that provider is used to retrieve all registered `Box<dyn HostedService>`, which are then started asynchronously. Once all HostedServces finish execution, the application shuts down.

# Challenges

## Static Context and asynchronous extensions
Because creating dynamic Rust libraries is currently not very common, you cannot easily compile and use external libraries as dynamic libs. This is problematic, as each plugin currently has it's own copy of a common library, even if the cargo workspace assures the same versions to be used. For datastructures, this is not a big problem, but static variables e.g. prevent you from using async functions of [tokio](https://crates.io/crates/tokio) directly. Thats the case because they internally refer to the plugins tokio runtime. In main we just started the plattforms tokio runtime, but the plugins runtime is still stopped. We can however work around this by statically linking tokio-dependent extensions like raf-web and raf-sql together with the platform. If these extensions provide services as trait objects which are wrapping tokio commands, they can be used within dynamically linked plugins too as shown in the prototype.

## SQLx integration
The most difficult library to be integrated into this framework was sqlx. When receiving data from a table, we want to map each row into a typed Rust structure, which would require a generic method on trait objects and this is not possible in rust.

Fortunately, sqlx does a great job in separation of concerns. It was therefore possible to execute queries using a trait object provided by raf-sql and map the rows within the todo-plugin, as the mapping doesn't know anything about tokio and it's static variables. 

Unfortunately, the sqlx executor has the following signature:
```rust
pub trait Executor<'c>: Send + Debug + Sized
```

Because of the `Sized` requirement, executors cannot be used as trait objects. Instead, we have to define our own trait [`raf_sql::SqliteExecutor`](https://github.com/mineichen/minfac/blob/main/examples/distributed_web/raf-sql/src/lib.rs) which is implemented on sqlx::Executors within raf-sql. If more people want to use executors as trait objects, let me know in the comments so it might make sense to bring it to sqlx directly. 

# Compilation time
If all code lives in a single project, changes in any code results in recompilation of the entire project. It is therefore common practice to split big projects into multiple subprojects to reduce build time. When features are even linked dynamically, not eventhe runtime executable has to be recompiled. To demonstrate the improved build times, the following non scientific experiment compares dynamic vs. static linking. Each measurement was taken 8 time by running `time cargo build`:

Hardware: MacBook Pro 2019, 2.6 GHz 6-Core Intel Core i7, 16 GB 2400 MHz DDR4
```
rustc --version
rustc 1.54.0 (a178d0322 2021-07-26)
```
Compile your project and just change any `println!()` argument within the plugin between each run:
1.326, 1.305, 1.363, 1.307, 1.581, 1.597, 1.448, 1.271 -> 1.400 avg. 

Add a reference from `runtime` to `todo` in Cargo.toml and remove the `crate-type = ["cdylib"]` in `todo`. Call `todo::register(&mut collection);` in the beginning of `runtime/main.rs` and compile the workspace. Change any `println!()` argument within the plugin between each run:
2.088, 2.143, 2.114, 2.187, 2.085, 2.086, 2.091, 2.350, 2.013 -> 2.143 agv.


As of today, the todo-plugin still includes hyper & tokio, because `hyper::Body` is used for return values of web handlers. However, the experiment of removing this dependency on a separate [branch](https://github.com/mineichen/minfac/tree/remove_hyper_in_plugin/examples/distributed_web) showed no significant effects on build times.

Without hyper, unlinked:
1.291, 1.319, 1.305, 1.447, 1.431, 1.388, 1.315, 1.312 -> 1.351 avg.
Without hyper, linked:
2.401, 1.990, 2.172, 2.366, 1.970, 2.057, 2.304, 2.213 -> 2.184 avg

The size of a release libtodo.dylib compiled with `cargo build --release` changed from 788’272 to only 682’576 bytes. For my projects, these numbers are not significant enough to justify the increased complexity.

# Summary
A plugin based architecture can easily be implemented in Rust using [minfac](https://crates.io/crates/minfac). 
- It would be great to have the tooling to share library dependencies like tokio as dynamic libraries among plugins so the runtime wouldn't need to link raf-web nor raf-sql  statically
- Even in a project with a single plugin, dynamic linking dropped compile time by 38%

If you'd like to read more about plugin based architectures in Rust, please give a thumbs up. If there is enough demand, I'd like to write a series with step-by-step explanation. 
I'm currently looking for a Job as a Rust developer in Switzerland or remote. If your team is looking for a passionate developer, I'd very much appreciate if you'd consider me for that position.