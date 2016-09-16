#!/bin/bash

set -e

# For os x convenience
if [[ "$(uname -s)" == 'Darwin' ]]; then
	eval "$(docker-machine env default)"
fi


#export EXTRA_DOCKER_BUILD_PARAMS=--no-cache
export VALGRIND=${VALGRIND:-False}
export SKIP_TESTING=${SKIP_TESTING:-False}

docker build ${EXTRA_DOCKER_BUILD_PARAMS} -t imazen/$1 ./$1
if [[ "$SKIP_TESTING" == 'True' ]]; then
	echo Skipping tests
else
	./test.sh $1 
fi
docker push imazen/$1


