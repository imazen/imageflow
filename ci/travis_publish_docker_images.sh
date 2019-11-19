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
    exit 1;
fi

# Test locally by running
#  TRAVIS_BRANCH=master ./ci/travis_trigger_docker_cloud.sh https://registry.hub.docker.com/u/imazen/imageflow_server_unsecured/trigger/3682f725-3a98-49dd-9e96-acd594721250/
#  TRAVIS_TAG=v0.0.10 ./ci/travis_trigger_docker_cloud.sh https://registry.hub.docker.com/u/imazen/imageflow_tool/trigger/d4943bd2-6350-4cda-9012-f56fe2deaef8/

if [[ -z "$TRAVIS_PULL_REQUEST_SHA" ]]; then
	if [[ -n "$TRAVIS_TAG" ]]; then
		export PUBLISH_DOCKER_TAG="${TRAVIS_TAG}"
	else
		if [[ "$TRAVIS_BRANCH" == "master" ]]; then
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

    echo "Building image $2:$PUBLISH_DOCKER_TAG in directory $1"

    (cd $1 && docker build -t "$2:$PUBLISH_DOCKER_TAG" --build-arg "SOURCE_COMMIT=$SOURCE_COMMIT" --build-arg "DOCKER_TAG=$PUBLISH_DOCKER_TAG" . && docker push "$2:$PUBLISH_DOCKER_TAG")


fi
