#!/bin/bash

set -e
set -x

#export EXTRA_DOCKER_BUILD_PARAMS=--no-cache

eval "$(docker-machine env default)"

docker build $EXTRA_DOCKER_BUILD_PARAMS -t imazen/$1 ./$1
./test.sh $1 stable
docker push imazen/$1


