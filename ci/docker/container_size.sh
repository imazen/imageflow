#!/bin/bash

set -e
set -x


eval "$(docker-machine env default)"


docker inspect -f "{{.Volumes}}" imazen/build_if_gcc54 | sed 's/map\[//' | sed 's/]//' | tr ' ' '\n' | sed 's/.*://' | xargs sudo du -d 1 -h
