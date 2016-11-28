# Source Guide


## Rust crates

* imageflow_types - JSON serialization types
* imageflow_core - Defines the FFI exposed in the shared library. Depends on imageflow_types
* imageflow_tool - Command-line app which wraps imageflow_core
* imageflow_server - prototype HTTP serve around imageflow_core
* imageflow_abi - The external FFI API
* imageflow_riapi - Eventual home of querystring-command interpreter


C source is located in ./c_componenets/lib, and ./c_components/tests

Headers for imageflow.dll are located in `bindings/headers`


