# imageflow-server status

The ruby RIAPI server is broken until we get dual static/dynamic builds going. Sorry!

---


#### imageflow-server proto1

You can, however, play with a rust teaser prototype that proxies images from unsplash.

Follow rust compilation steps (take a look at the CI script), and then (from `wrappers/server`)

`cargo run --bin imageflow-server`

Followed by opening this in your browser: 

`http://localhost:3000/proto1/scale_unsplash_jpeg/1200/1200/photo-1436891678271-9c672565d8f6`

This Rust server was built one saturday evening to exercise various bits of the stack. It's not safe (nor safer than a C equivalent), and we're not using Rust idomatically or correctly. Nothing is re-entrant, and errors panic the process. It's one build to throw away; a learning experiment. We didn't even bother to expose any parameters except width/height.
