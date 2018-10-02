#!/bin/bash
# script expects TRAVIS_COMMIT_RANGE to be set to a commit range to check for changes
# and TRAVIS_BUILD_DIR set to the build dir
# Docker hub only triggers if $TRAVIS_BRANCH or $TRAVIS_TAG are set

#Or invoke script with 'force' parameter to force all to trigger
if [[ "$1" == 'force' ]]; then
    TRAVIS_COMMIT_RANGE=force
    TRAVIS_BUILD_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )/.." && pwd )"
fi



# PURPOSE: to efficiently trigger new docker hub builds if and only if the dockerfiles are edited
# To use, configure a Docker Hub Automated Build, but disable build-on-push
# Modify the Dockerfile Location field for both tags and branches to point to the folder, like /docker/imageflow_tool/
# Generate a trigger URL and use below

# It's safe for trigger tokens to be public, they are deduped and rate limited to 10 and can't do much harm
# Docker hub only checks GitHub for the given tag/branch - it does not accept any data.


# first param is file to monitor for changes
# second is endpoint to inform
trigger_docker_hub_if_changed(){

    if [ -z "$1" ]; then
        echo "First parameter must be a file to diff for changes. Exiting." && exit 1;
    fi
    if [ -z "$2" ]; then
        echo "Second parameter must be a docker hub endpoint. Exiting." && exit 1;
    fi

    if [ -z "${TRAVIS_COMMIT_RANGE}" ]; then
        echo "TRAVIS_COMMIT_RANGE not set - should be commit range to check for changes, like 6544f0b..a62c029. Exiting." && exit 1;
    else
        if [[ "$TRAVIS_COMMIT_RANGE" == 'force' ]]; then
            echo "Forcing trigger for $1"
            {
                export TRAVIS_BRANCH="${TRAVIS_BRANCH:-master}"
                ./ci/travis_trigger_docker_cloud.sh "$2"
            }
        else
            echo "Scanning ${TRAVIS_COMMIT_RANGE} for changes to $1";
            git diff -s --exit-code "${TRAVIS_COMMIT_RANGE}" -- $1
            RETVAL=$?
            if [ $RETVAL -eq 1 ]; then
                echo ... found changes in $1, invoking travis_trigger_docker_cloud.sh
                ./ci/travis_trigger_docker_cloud.sh "$2"
            elif [ $RETVAL -eq 0 ]; then
                echo ... no changes
            else
                echo ... git command failed with error ${RETVAL}
            fi
        fi
    fi
}

cd "$TRAVIS_BUILD_DIR" || exit


trigger_docker_hub_if_changed "./docker/imageflow_base_os/Dockerfile" "https://registry.hub.docker.com/u/imazen/imageflow_base_os/trigger/50b3bc74-1719-4f80-ae72-d141b0dc4b56/"
trigger_docker_hub_if_changed "./docker/imageflow_build_ubuntu18/Dockerfile" "https://registry.hub.docker.com/u/imazen/imageflow_build_ubuntu18/trigger/fb077cf4-066e-46d0-bfe2-7c8d96acfeca/"
trigger_docker_hub_if_changed "./docker/imageflow_build_ubuntu16/Dockerfile" "https://registry.hub.docker.com/u/imazen/imageflow_build_ubuntu16/trigger/125d6410-19fa-433b-be54-512b0372d1a3/"
trigger_docker_hub_if_changed "./docker/imageflow_build_ubuntu18_debug/Dockerfile" "https://registry.hub.docker.com/u/imazen/imageflow_build_ubuntu18_debug/trigger/38852860-517f-49c2-81c2-a033aba36a5b/"

