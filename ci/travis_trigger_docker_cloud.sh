#!/bin/bash
set -e #Exit on failure.

# Test locally by running
#  TRAVIS_BRANCH=master ./ci/travis_trigger_docker_cloud.sh
#  TRAVIS_TAG=v0.0.10 ./ci/travis_trigger_docker_cloud.sh

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

	# This token has no security value and is rate limited to 10. 
	# It only checks GitHub for the given tag/branch - it does not accept any data.
	TRIGGER_ENDPOINT=https://registry.hub.docker.com/u/imazen/imageflow_server_unsecured/trigger/3682f725-3a98-49dd-9e96-acd594721250/
	echo "Triggering docker cloud build with $PAYLOAD"
	curl -H "Content-Type: application/json" --data "${PAYLOAD}" -X POST "$TRIGGER_ENDPOINT"
fi 

