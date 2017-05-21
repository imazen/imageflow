#!/bin/bash
set -e

# The purpose of this script is to compile Imageflow locally (or in a CI simulation docker container), then copy it to *another* docker container, and run a basic smoke test. 
# This can help detect incompatibilites and missing basics, like glibc. 

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
export FROM_IMAGE="imazen/imageflow_base_os"
export BUILD_IMAGE_NAME="imazen/imageflow_build_ubuntu16"
export OUTPUT_IMAGE_NAME="local/if_testing"
export DOCKER_DIR="/home/imageflow"

export SAFE_IMAGE_NAME="$BUILD_IMAGE_NAME"
SAFE_IMAGE_NAME="${SAFE_IMAGE_NAME//\//_}"
SAFE_IMAGE_NAME="${SAFE_IMAGE_NAME//:/_}"


echo "./dockerize.sh $1 $2 $3"
echo "help: ./dockerize.sh (debug|quiet[123]|release|clean|valgrind|test|rusttest)+ localbuild|docker server|tool #SMOKE TESTS ONLY"

export OVERRIDE="$1"
export OVERRIDE="${OVERRIDE:-debugquiet}"
if [[ "$OVERRIDE" == *"debug"* ]]; then
    export PROFILE=debug
else
    export PROFILE=release
fi

export CARGO_TARGET="${CARGO_TARGET:-}"

if [[ -n "$CARGO_TARGET" ]]; then
    export TARGET_DIR="target/${CARGO_TARGET}/"
else 
    export TARGET_DIR="target/"
fi


if [[ "$3" == 'tool' ]]; then
	export BINARY_NAME=imageflow_tool
	export TEST_ENTRYPOINT=(sudo "${DOCKER_DIR}/${BINARY_NAME}" diagnose --self-test)
else
	export BINARY_NAME=imageflow_server
	export TEST_ENTRYPOINT=(sudo "${DOCKER_DIR}/${BINARY_NAME}" diagnose --smoke-test-core)
fi

if [[ "$2" == 'docker' ]]; then

    TARGET_CPU="${TARGET_CPU:-x86-64}"
    WORKING_DIR="${HOME}/.docker_imageflow_caches/.docker_${SAFE_IMAGE_NAME}_${TARGET_CPU}"
	export BINARY_DIR="${WORKING_DIR}_cache/${TARGET_DIR}${PROFILE}"
else
	export BINARY_DIR="${SCRIPT_DIR}/../${TARGET_DIR}${PROFILE}"
fi


if [[ -d "$BINARY_DIR" ]]; then
    export BINARY_DIR
    BINARY_DIR="$(readlink -f "$BINARY_DIR")"
else
    echo "Cannot find $BINARY_DIR"
fi
export BINARY_OUT="$BINARY_DIR/$BINARY_NAME"
export BINARY_COPY="${SCRIPT_DIR}/bin/$BINARY_NAME"
mkdir -p "${SCRIPT_DIR}/bin/" || true
mkdir -p "${BINARY_DIR}" || true &>/dev/null

sep_bar(){
    printf "\n=================== %s ======================\n" "$1"
}
print_modified_ago(){
    if [[ -f "$1" ]]; then
        printf "(modified %s seconds ago)" "$(( $(date +%s) - $(stat -c%Y "$1") ))"
    fi
}

sep_bar "Compiling"
printf "BINARY_OUT=%s " "$BINARY_OUT" && print_modified_ago "$BINARY_OUT" && printf "\n"

export BUILD_QUIETER="${BUILD_QUIETER:-True}"
export UPLOAD_BUILD=False
export UPLOAD_DOCS=False
export IMAGEFLOW_BUILD_OVERRIDE="$OVERRIDE"

if [[ "$2" == 'docker' ]]; then
	( cd "${SCRIPT_DIR}/../ci" && ./simulate_travis.sh "${BUILD_IMAGE_NAME}" )
else
    ( "${SCRIPT_DIR}/../build.sh" "${OVERRIDE}" )

    #if [[ "$PROFILE" == 'debug' ]]; then
    # 	( set -vx && cd "${SCRIPT_DIR}/../${CRATE_NAME}" && cargo build --bin "${BINARY_NAME}" )
	#else
	#    ( set -vx && cd "${SCRIPT_DIR}/../${CRATE_NAME}" && cargo build --bin "${BINARY_NAME}" --release )
	#fide
fi

# Post-compile build info
"${BINARY_OUT}" --version || ( printf "\nBINARY_OUT=%s " "$BINARY_OUT" && print_modified_ago "$BINARY_OUT" && printf "\n" )

# Generate and build Dockerfile
sep_bar "Dockerizing"
(
    cd "$SCRIPT_DIR"
    cp -p "${BINARY_OUT}" .
    printf "\nCreating Dockerfile\n\n"
    printf "FROM %s\n\nEXPOSE 39876\n\nADD %s %s/" "$FROM_IMAGE" "$BINARY_NAME" "$DOCKER_DIR" > Dockerfile
    docker build -t "$OUTPUT_IMAGE_NAME" .
)
sep_bar "Smoke testing in Docker"
docker run --rm "${OUTPUT_IMAGE_NAME}"  "${DOCKER_DIR}/${BINARY_NAME}" --version || printf "Failed to run %s --version!\n" "${BINARY_NAME}"

set +e

if docker run --rm "${OUTPUT_IMAGE_NAME}"  "${TEST_ENTRYPOINT[@]}"; then
    sep_bar "PASSED"
else
    sep_bar "FAILED"
    export TEST_FAILED=1
fi
set -e



if [[ "$TEST_FAILED" == '1' ]]; then
    echo "Entering interactive"
    echo "This creates docker containers and doesn't clean them up. Use this to remove all containers (danger!)"
    # shellcheck disable=SC2016
    echo 'docker rm `docker ps -aq`'

    docker run -i -t   "${OUTPUT_IMAGE_NAME}" /bin/bash

    exit 1
fi

if [[ "$BINARY_NAME" == 'imageflow_server' ]]; then
    docker run -i -t  -p 3000:3000 "${OUTPUT_IMAGE_NAME}" sudo "${DOCKER_DIR}/${BINARY_NAME}" start --demo --port 3000 --bind-address 0.0.0.0
fi


#docker push  "${IMAGE_NAME}"
#docker-cloud stack up --name "$TEST_STACK_NAME"
#docker-cloud stack update "$TEST_STACK_NAME"
#export STACK_UID= $(docker-cloud stack up --name flow3 -f docker-solo.yaml)
#printf "%s" "${STACK_UID}"
#docker-cloud stack redeploy "$TEST_STACK_NAME"



