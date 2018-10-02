#!/bin/bash
set -e #Exit on failure.

# Change directory to root (call this in a subshell if you have a problem with that)
cd "$( dirname "${BASH_SOURCE[0]}" )"

# To remove (the LARGE) caches this writes to your home directory 
# rm -rf ~/.docker_imageflow_caches

cd ci 

export OVERRIDE="$1"
export OVERRIDE="${OVERRIDE:-debugquiet}"

export UPLOAD_BUILD=False
export UPLOAD_DOCS=False
export IMAGEFLOW_BUILD_OVERRIDE="$OVERRIDE"

./simulate_travis.sh imazen/imageflow_build_ubuntu18
