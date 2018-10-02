#!/bin/bash
# script expects TRAVIS_COMMIT_RANGE to be set to a commit range to check for changes
# and TRAVIS_BUILD_DIR set to the build dir
# Docker hub only triggers if $TRAVIS_BRANCH or $TRAVIS_TAG are set

# first param is file to monitor for changes
# second is endpoint to inform
inform_docker_hub_if_changed(){

    if [ -z "$1" ]; then
        echo "First parameter must be a file to diff for changes. Exiting." && exit 1;
    fi
    if [ -z "$2" ]; then
        echo "Second parameter must be a docker hub endpoint. Exiting." && exit 1;
    fi

    if [ -z "${TRAVIS_COMMIT_RANGE}" ]; then
        echo "TRAVIS_COMMIT_RANGE not set - should be commit range to check for changes, like 6544f0b..a62c029. Exiting." && exit 1;
    else
        echo "Scanning ${TRAVIS_COMMIT_RANGE} for changes to $1";
        git diff -s --exit-code "${TRAVIS_COMMIT_RANGE}" -- $1
        RETVAL=$?
        if [ $RETVAL -eq 1 ]; then
            echo ... found changes, invoking travis_trigger_docker_cloud.sh
            ./ci/travis_trigger_docker_cloud.sh "$2"
        elif [ $RETVAL -eq 0 ]; then
            echo ... no changes
        else
            echo ... git command failed with error ${RETVAL}
        fi

    fi
}

cd "$TRAVIS_BUILD_DIR" || exit

#inform_docker_hub_if_changed("./docker/imageflow_base_os/Dockerfile","")
#inform_docker_hub_if_changed("./docker/imageflow_build_ubuntu16/Dockerfile","")
#inform_docker_hub_if_changed("./docker/imageflow_build_ubuntu18/Dockerfile","")
inform_docker_hub_if_changed "./docker/imageflow_build_ubuntu18_debug/Dockerfile" "https://registry.hub.docker.com/u/imazen/imageflow_build_ubuntu18_debug/trigger/38852860-517f-49c2-81c2-a033aba36a5b/"

