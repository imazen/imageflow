#!/bin/bash

set -e
set -x


eval "$(docker-machine env default)"

docker images


docker history imazen/build_if_gcc54
docker history imazen/build_if_gcc48
docker history imazen/build_if_gcc49

docker run imazen/build_if_gcc54 du -h / | grep '[0-9\.]\+M'

