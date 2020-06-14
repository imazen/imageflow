## imazen/imageflow_base_os

* This image should contain the runtime dependencies needed by imageflow_server and imageflow_tool. 
* It should also contain wget, for use in child dockerfiles
* RUST_BACKTRACE=1

It should be rebuilt every commit, master -> latest, (v[0-9].*) -> $1


ubuntu:bionic with imageflow user account - updated with sudo wget libcurl4-openssl-dev  curl libssl-dev ca-certificates libpng-dev