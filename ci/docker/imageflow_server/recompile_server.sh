#!/bin/bash
set -e

#export EXTRA_DOCKER_BUILD_PARAMS=--no-cache

(
	cd ..
	export CLEAN_RUST_TARGETS=True
	./test.sh build_if_gcc54
)
./copy_server_from_gcc54.sh