#!/bin/bash
set -e #Exit on failure.

if [[ -z "$1" ]]; then
    echo "travis_publish_docker_images.sh requires a Dockerfile directory a parameter. Exiting." && exit 1;
fi

if [[ -z "$2" ]]; then
    echo "travis_publish_docker_images.sh requires a docker image name as a second parameter. Exiting." && exit 1;
fi

if [[ "$PUBLISH_DOCKER" == "True" ]]; then
    echo Publishing to docker hub enabled
else
    echo "Publishing to docker hub disabled. Exiting."
    exit 1;
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


    if [[ -z "$SOURCE_COMMIT" ]]; then
        export SOURCE_COMMIT="${SOURCE_COMMIT:-$(git rev-parse HEAD)}"
        echo "Updating SOURCE_COMMIT from git rev-parse HEAD"
        echo "SOURCE_COMMIT: $SOURCE_COMMIT"
    fi
    echo "Logging into Docker"
    docker login -u "$DOCKER_USERNAME" -p "$DOCKER_PASSWORD"


    echo "Building image $2:$PUBLISH_DOCKER_TAG in directory $1"

    (cd $1 && docker build -t "$2:$PUBLISH_DOCKER_TAG" --build-arg "SOURCE_COMMIT=$SOURCE_COMMIT" --build-arg "DOCKER_TAG=$PUBLISH_DOCKER_TAG" . && docker push "$2:$PUBLISH_DOCKER_TAG" && echo Exited with code $?)


fi
