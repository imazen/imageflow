# Dockerfiles


## for running Imageflow

* imageflow_tool
* imageflow_server_unsecured
* proxied_stack

## for building Imageflow

All Dockerfiles should use user account `imageflow` with uid 1000
Build directory should be /home/imageflow/imageflow
Run directory should be /home/imageflow

* imageflow_base_os is for lightweight deployment; it is not used during build.
* imageflow_build_ubuntu16
* imageflow_build_ubuntu18
* imageflow_build_ubuntu18_debug

## Building

No special requirements. The build scripts are there for convenience.

## Testing

Clone imazen/imageflow, and invoke ./ci/docker/test.sh [imagename] `imazen/` is auto-prefixed to the first argument.


## FAQ

### Cannot connect to the Docker daemon. Is the docker daemon running on this host?

See https://stackoverflow.com/questions/21871479/docker-cant-connect-to-docker-daemon


OS X only:

```
docker-machine start # start virtual machine for docker
docker-machine env  # it helps to get environment variables
eval "$(docker-machine env default)" #set environment variables
```
