#!/bin/bash
set -e

print_modified_ago(){
     echo "Modified" $(( $(date +%s) - $(stat -c%Y "$1") )) "seconds ago"
}

(
	export TARGET_RELEASE_DIR="~/.docker_imageflow_caches.docker_build_if_gcc_54_cache_target_release"
	
	set -x
	cp -p "${TARGET_RELEASE_DIR}/imageflow_server" "./bin/"
	set +x
	print_modified_ago "./bin/imageflow_server"
	set -x
	cp -p "${TARGET_RELEASE_DIR}/imageflow_tool" "./bin/"
	set +x
	print_modified_ago "./bin/imageflow_tool"
)