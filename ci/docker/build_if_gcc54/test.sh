set -e
set -x

export VALGRIND=${VALGRIND:-False}
export SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
export IMAGE_NAME="$(basename ${SCRIPT_DIR})"
export RUST_CHANNEL=$1
export RUST_CHANNEL=${RUST_CHANNEL:-nightly}  
${SCRIPT_DIR}/../test.sh ${IMAGE_NAME} ${RUST_CHANNEL}  
