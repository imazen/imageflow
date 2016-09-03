set -e
set -x

export VALGRIND=${VALGRIND:-False}
export SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
export IMAGE_NAME="$(basename ${SCRIPT_DIR})"
${SCRIPT_DIR}/../test.sh ${IMAGE_NAME} ${$1:-nightly}  
