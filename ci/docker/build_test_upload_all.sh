#!/bin/bash

set -e
set -x

#export EXTRA_DOCKER_BUILD_PARAMS=--no-cache

parallel --progress ./build_test_upload.sh {} ::: build_if_gcc48 build_if_gcc54

./build_server.sh
docker push "imageflow/imageflow_server"