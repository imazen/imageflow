#!/bin/bash
set -e #Exit on failure.

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
	TRIGGER_ENDPOINT=https://registry.hub.docker.com/u/nathanaeljones/imageflow_server_unsecured/trigger/4cef52f0-76f6-4f2e-9a5e-e2b93b7d2f59/
	echo "Triggering docker cloud build with $PAYLOAD"
	curl -H "Content-Type: application/json" --data "${PAYLOAD}" -X POST "$TRIGGER_ENDPOINT"
fi 

