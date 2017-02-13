#!/bin/bash
set -e #Exit on failure.

# Test locally by running
#  TRAVIS_BRANCH=master ./ci/travis_trigger_docker_cloud.sh
#  TRAVIS_TAG=v0.0.10 ./ci/travis_trigger_docker_cloud.sh

if [[ -z "$TRAVIS_PULL_REQUEST_SHA" ]]; then
	if [[ -n "$TRAVIS_TAG" ]]; then
		# We can re-enable when tagged releases allow non-localhost connections
		#export CLOUD_SOURCE_NAME="${TRAVIS_TAG}"
		#export CLOUD_SOURCE_TYPE="Tag"
		echo "Skipping docker cloud build for tags (update when we permit non-localhost connections)"
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
	echo "Trigger 1 (server): docker cloud build with $PAYLOAD"
	curl -H "Content-Type: application/json" --data "${PAYLOAD}" -X POST "$TRIGGER_ENDPOINT"

	TRIGGER_ENDPOINT_2=https://registry.hub.docker.com/u/imazen/imageflow_tool/trigger/d4943bd2-6350-4cda-9012-f56fe2deaef8/
	
	echo "Trigger 2 (imageflow_tool) docker cloud build with $PAYLOAD"
	curl -H "Content-Type: application/json" --data "${PAYLOAD}" -X POST "$TRIGGER_ENDPOINT_2"


fi 

