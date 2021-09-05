# Usage

This example demonstrates the usage of a service whose implementation is not part of the runtime project.

Execute `cargo run` to run the demo.

# Compilation time static vs dynamic linking
If all code lives in a single project, changes in any code results in recompilation of the entire project. It is therefore common practice to split big projects into multiple subprojects to reduce build time. When features are even linked dynamically, not eventhe runtime executable has to be recompiled. To demonstrate the improved build times, the following non scientific experiment compares dynamic vs. static linking. Each measurement was taken 8 time by running `time cargo build`:

Hardware: MacBook Pro 2019, 2.6 GHz 6-Core Intel Core i7, 16 GB 2400 MHz DDR4
```
rustc --version
rustc 1.54.0 (a178d0322 2021-07-26)
```
Compile your project and just change any `println!()` argument within the plugin between each run:
1.326, 1.305, 1.363, 1.307, 1.581, 1.597, 1.448, 1.271 -> 1.400 avg. 

Add a reference from `runtime` to `todo` in Cargo.toml and remove the `crate-type = ["cdylib"]` in `todo`. Call `todo::register(&mut collection);` in the beginning of `runtime/main.rs` and compile the workspace. Change any `println!()` argument within the plugin between each run:
2.088, 2.143, 2.114, 2.187, 2.085, 2.086, 2.091, 2.350 -> 2.143 agv.


As of today, the todo-plugin still includes hyper & tokio, because `hyper::Body` is used for return values of web handlers. However, the experiment of removing this dependency on a separate [branch](https://github.com/mineichen/minfac/tree/remove_hyper_in_plugin/examples/distributed_web) showed no significant effects on build times.

Without hyper, unlinked:
1.291, 1.319, 1.305, 1.447, 1.431, 1.388, 1.315, 1.312 -> 1.351 avg.
Without hyper, linked:
2.401, 1.990, 2.172, 2.366, 1.970, 2.057, 2.304, 2.213 -> 2.184 avg

The size of a release libtodo.dylib compiled with `cargo build --release` changed from 788’272 to only 682’576 bytes. For my projects, these numbers are not significant enough to justify the increased complexity.
