# imageflow_core

#### Build notes

Run ./build.sh c before attempting to compile this crate

This crate depends on imageflow_helpers, imageflow_riapi, and imageflow_types. 

# Why an operation graph?

1. Graphs are flexible, and easily transmitted over the wire. 
2. You can optimize graphs reasonably easily by eliminating useless steps or merging others.

The graph is always directed and acyclic. 

It's easy to traverse to determine which decoders contributed data to a given
operation or encoder. It also facilitates multi-input and multi-output image jobs.




