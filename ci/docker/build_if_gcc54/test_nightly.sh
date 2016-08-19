set -e
set -x

export VALGRIND=true
export SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
export IMAGE_NAME="$(basename ${SCRIPT_DIR})"
${SCRIPT_DIR}/../test.sh ${IMAGE_NAME} nightly 
