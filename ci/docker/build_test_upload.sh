#!/bin/bash

set -e
set -x

#export EXTRA_DOCKER_BUILD_PARAMS=--no-cache
export VALGRIND=${VALGRIND:-False}
export RUST_CHANNEL=${RUST_CHANNEL:-nightly}
export SKIP_TESTING=${SKIP_TESTING:-False}
eval "$(docker-machine env default)"

docker build $EXTRA_DOCKER_BUILD_PARAMS -t imazen/$1 ./$1
if [[ "$SKIP_TESTING" == 'True' ]]; then
	echo Skipping tests
else
	./test.sh $1 $RUST_CHANNEL
fi
docker push imazen/$1


