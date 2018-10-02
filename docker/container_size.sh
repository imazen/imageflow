#!/bin/bash

docker images

docker history imazen/imageflow_base_os
docker history imazen/imageflow_build_ubuntu14
docker history imazen/imageflow_build_ubuntu16
docker history imazen/imageflow_build_ubuntu18
docker history imazen/imageflow_build_ubuntu18_debug

#docker run imazen/build_if_gcc48 du -h / | grep '[0-9\.]\+M'

