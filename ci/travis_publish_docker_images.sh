#!/bin/bash
set -e #Exit on failure.

if [[ -z "$1" ]]; then
    echo "travis_publish_docker_images.sh requires a Dockerfile directory a parameter. Exiting." && exit 1;
fi

if [[ -z "$2" ]]; then
    echo "travis_publish_docker_images.sh requires a docker image name as a second parameter. Exiting." && exit 1;
fi

if [[ -z "$TRAVIS_PULL_REQUEST_SHA" ]]; then
	if [[ -n "$TRAVIS_TAG" ]]; then
		export PUBLISH_DOCKER_TAG="${TRAVIS_TAG}"
	else
		if [[ "$TRAVIS_BRANCH" == "main" ]]; then
			export PUBLISH_DOCKER_TAG="latest"
		fi
	fi
fi
if [[ -n "$PUBLISH_DOCKER_TAG" ]]; then

    echo "Building image $2:$PUBLISH_DOCKER_TAG in directory $1"

    (cd $1 && docker build -t "$2:$PUBLISH_DOCKER_TAG" --build-arg "IMAGEFLOW_DOWNLOAD_URL_TAR_GZ=${IMAGEFLOW_DOWNLOAD_URL_TAR_GZ}" --build-arg "DOCKER_TAG=$PUBLISH_DOCKER_TAG" . && docker push "$2:$PUBLISH_DOCKER_TAG" && echo Exited with code $?)


fi
