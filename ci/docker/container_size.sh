#!/bin/bash


# For os x convenience
if [[ "$(uname -s)" == 'Darwin' ]]; then
	eval "$(docker-machine env default)"
fi

docker images


docker history imazen/build_if_gcc54
docker history imazen/build_if_gcc48
docker history imazen/build_if_gcc49

#docker run imazen/build_if_gcc48 du -h / | grep '[0-9\.]\+M'

