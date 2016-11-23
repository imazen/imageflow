#!/bin/bash
set -e

#export EXTRA_DOCKER_BUILD_PARAMS=--no-cache


# For os x convenience
if [[ "$(uname -s)" == 'Darwin' ]]; then
	eval "$(docker-machine env default)"
fi

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
IMAGE_NAME="$(basename "${SCRIPT_DIR}")"


set -x

# shellcheck disable=SC2086
docker build ${EXTRA_DOCKER_BUILD_PARAMS} -t "imazen/${IMAGE_NAME}" "${SCRIPT_DIR}"

docker history "imazen/${IMAGE_NAME}"

docker run "imazen/${IMAGE_NAME}" du -h / | grep '[0-9\.]\+M'

