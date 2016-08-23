#!/bin/bash

set -e
set -x

#Don't accidentally upload build artifacts unless PACKAGE_SUFFIX is set
if [ -z "$PACKAGE_SUFFIX" ]; then
  rm -rf ${TRAVIS_BUILD_DIR}/artifacts
else
  cd ${TRAVIS_BUILD_DIR}/artifacts/staging
  tar czf ${TRAVIS_BUILD_DIR}/artifacts/${PROJECT_NAME}-build${TRAVIS_BUILD_ID}-${TRAVIS_BRANCH}-${TRAVIS_COMMIT}-${PACKAGE_SUFFIX}.tar.gz *
  cd ${TRAVIS_BUILD_DIR}
fi
