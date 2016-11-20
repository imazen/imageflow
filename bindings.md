# Using Imageflow from your language

Imageflow-server can be used directly from your browser, when configured to act as an image server or proxy.
The simplest API is the querystring-driven API. Given an image URL served by Imageflow-server, you simply add commands to the querystring, [like ImageResizer](http://imageresizing.net/docs/v4/docs/basics). Ex. `http://imageflow.server/folder/image.jpg?width=200`

You can also use Imageflow-server's JSON API via HTTP, which expose the full power of the software. Imageflow-server runs as a separate process, and your code communicates with it over HTTP. The RESTful JSON API is easy to use from any language, with or without bindings.

You also have the option of using imageflow-tool, the command-line option. If you're dealing with local files, the overhead is actually pretty low, since imageflow doesn't have a runtime or any real startup costs.

## Using libimageflow in-process

In-process use requires [FFI](https://en.wikipedia.org/wiki/Foreign_function_interface) bindings. In .NET, the term P/Invoke is used instead of FFI.


### C# bindings

@samuelenglard has volunteered to create C# bindings for Imageflow. We're tracking [design here](https://github.com/imazen/imageflow/issues/67).

### Ruby bindings

We have a [limited set of Ruby bindings already](https://github.com/imazen/imageflow/tree/master/bindings/ruby), but the FFI API will soon be refactored to use JSON, and these may lag behind Rust bindings initially.
Currently, you can't use the Ruby bindings unless you compile Imageflow in shared mode (.dll/.so/.dylib), which isn't the default. 

Official Ruby bindings will be released by August 2017. 

### Rust bindings

Our [prototypes of imageflow-tool and imageflow-server](https://github.com/imazen/imageflow/tree/master/wrappers/server) are already written in Rust. We haven't yet exposed a crate that makes these bindings ergonomic, though.
Given we're moving the libimageflow FFI surface to Rust, these will likely improve soon.


### Node bindings

Unofficial Node bindings (via Neon) are likely to happen before December 2016. Sooner, if we get any volunteers. 

Official Node bindings will be released by August 2017. 


### C/C++ 'bindings'

Status: active/complete. imageflow.h always tracks the current API. 

### PHP/Python/Java/Go/Haskell/Swift bindings

Unless we reach our stretch goal, we can't guarantee official PHP bindings. That said, we're happy to work with volunteers who want to create bindings for any language. Please reach out if you're interested!

### Erlang/Elixr

Oooh, fun.

Have you created bindings you want to list here? Send us a PR, we'll add your project!

# REST API Clients 

Delayed until JSON schema is finalized. Not really neccessary for the querystring API, unless you need signatures.
