#!/bin/bash

set -e
set -x

#Don't accidentally upload build artifacts unless PACKAGE_SUFFIX is set
if [ -z "$PACKAGE_SUFFIX" ]; then
  rm -rf ${TRAVIS_BUILD_DIR}/artifacts
else
  cd ${TRAVIS_BUILD_DIR}/artifacts/staging
  tar czf ${TRAVIS_BUILD_DIR}/artifacts/${PACKAGE_PREFIX}-${TRAVIS_BRANCH}-travisjob-${TRAVIS_JOB_NUMBER}-${TRAVIS_COMMIT}-${PACKAGE_SUFFIX}.tar.gz *
  cd ${TRAVIS_BUILD_DIR}
fi
