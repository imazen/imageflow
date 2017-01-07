
## imageflow_server_unsecured

No HTTPS support, no TLS, no NGINX proxy. Just imageflow_server, that's it. Compiled for Sandy Bridge and higher architectures

Starts the demo server by default on port 39876


```
$imageflow_server help start

Start HTTP server

USAGE:
    imageflow_server start [FLAGS] [OPTIONS] --mount <mount>... --data-dir <data-dir>

FLAGS:
        --demo       Start demo server (on localhost:39876 by default) with mounts /ir4/proxy/unsplash -> http://images.unsplash.com/
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --bind-address <bind-address>    The IPv4 or IPv6 address to bind to (or the hostname, like localhost). 0.0.0.0 binds to all addresses. [default:
                                         localhost]
        --data-dir <data-dir>            An existing directory for logging and caching
        --mount <mount>...               Serve images from the given location using the provided API, e.g --mount "/prefix/:ir4_local:./{}" --mount
                                         "/extern/:ir4_http:http:://domain.com/{}"
                                          Escape colons by doubling, e.g. http:// -> http:://
    -p, --port <port>                    Set the port that the server will listen on [default: 39876]
```


```
$imageflow_server help diagnose

imageflow_server-diagnose 
Diagnostic utilities

USAGE:
    imageflow_server diagnose [FLAGS]

FLAGS:
        --call-panic               Triggers a Rust panic (so you can observe failure/backtrace behavior)
    -h, --help                     Prints help information
        --show-compilation-info    Show all the information stored in this executable about the environment in which it was compiled.
    -V, --version                  Prints version information
```