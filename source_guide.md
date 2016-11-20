# Source Guide


## Rust crates

* imageflow_types - JSON serialization types
* imageflow_core - Defines the FFI exposed in the shared library. Depends on imageflow_types
* imageflow_tool - Command-line app which wraps imageflow_core
* imageflow_server - prototype HTTP serve around imageflow_core
* imageflow_abi - Empty crate to re-export a dynamic library with dependencies statically linked
* imageflow_riapia - Eventual home of querystring-command interpreter


C source is located in the root, ./lib, and ./tests

* imageflow.h - NOT what you use when binding to imageflow_abi. The header for the C component

Understanding the C data structures

### flow_context (flow_c)

* Serves as a dependency injection container for codecs and node operations
* Provides colorspace lookup tables (this needs to be refactored away)
* Provides a location for depositing profiling samples
* Provides a heap and object tracking system, to provide 'destructor' like functionality and hierarchical ownership/free. 
* Provides an error state (numeric code, string message, and stacktrace). 

### flow_io

Generic abstraction to permit streams and custom I/O sources to provde read, write, seek capabilities. Codecs should understand flow_io

### flow_job

Contains job-specific configuration, and is where you link flow_io instances to "IO IDs"/"Placeholder IDs".

### flow_graph

A directed acyclic graph, stored in a single allocation, which can be mutated. Mutation happens during creation, 'flattening', optimization, and execution.

Most operation nodes 'flatten' into one or more primitives. 
No optimizations are yet implemented, but ideally we would match patterns (like convert two subsequent FlipV ops to a null-op). Since codec varieties and EXIF orientation flags may cause unexpected additions to the graph, optimization can't be reasoned about very aggresively by the user. 




