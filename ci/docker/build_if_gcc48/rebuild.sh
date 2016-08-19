#!/bin/bash
set -e
set -x

eval "$(docker-machine env default)"

export SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
export IMAGE_NAME="$(basename ${SCRIPT_DIR})"
docker build --no-cache -t imazen/${IMAGE_NAME} ${SCRIPT_DIR}

