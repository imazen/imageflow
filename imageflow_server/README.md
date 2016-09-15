# imageflow_server playground

So, first thing: THESE ARE THROW-AWAY PROTOTYPES. Don't use them in production, please. Test coverage sucks and we're validating theories & testing dependencies right now. 

This is a learning experiment; a throwaway proxy for testing the HTTP libraries we plan to use. It's a saturday afternoon exercise, not a real product. It's not safe (nor really safer than a C equivalent), and we're not using Rust idomatically or correctly. Nothing is re-entrant, and errors panic the process.  Run with `cargo run --bin imageflow_server`, then open in your browser: http://localhost:3000/proto1/scale_unsplash_jpeg/1200/1200/photo-1436891678271-9c672565d8f6
