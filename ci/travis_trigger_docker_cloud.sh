#!/bin/bash
set -e #Exit on failure.

if [ -z "$1" ]; then
    echo "travis_trigger_docker_hub.sh requires a docker hub endpoint url as a parameter. Exiting." && exit 1;
fi

# Test locally by running
#  TRAVIS_BRANCH=master ./ci/travis_trigger_docker_cloud.sh https://registry.hub.docker.com/u/imazen/imageflow_server_unsecured/trigger/3682f725-3a98-49dd-9e96-acd594721250/
#  TRAVIS_TAG=v0.0.10 ./ci/travis_trigger_docker_cloud.sh https://registry.hub.docker.com/u/imazen/imageflow_tool/trigger/d4943bd2-6350-4cda-9012-f56fe2deaef8/

if [[ -z "$TRAVIS_PULL_REQUEST_SHA" ]]; then
	if [[ -n "$TRAVIS_TAG" ]]; then
		export CLOUD_SOURCE_NAME="${TRAVIS_TAG}"
		export CLOUD_SOURCE_TYPE="Tag"
	else
		if [[ -n "$TRAVIS_BRANCH" ]]; then
			export CLOUD_SOURCE_NAME="${TRAVIS_BRANCH}"
			export CLOUD_SOURCE_TYPE="Branch"
		fi
	fi
fi 

if [[ -n "$CLOUD_SOURCE_NAME" ]]; then
	PAYLOAD="{'source_type': '${CLOUD_SOURCE_TYPE}', 'source_name': '${CLOUD_SOURCE_NAME}'}"

	# Endpoint url tokens have no security value and are rate limited to 10.
	# It only checks GitHub for the given tag/branch - it does not accept any data.
	echo "Invoking $1 with $PAYLOAD"
	curl -H "Content-Type: application/json" --data "${PAYLOAD}" -X POST "$1"
fi 

