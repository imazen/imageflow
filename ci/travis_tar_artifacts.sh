#!/bin/bash

set -e
set -x

ls -R ./artifacts/*
#Don't accidentally upload build artifacts unless PACKAGE_SUFFIX is set
if [ -z "$PACKAGE_SUFFIX" ]; then
  echo "Dropping artifacts; PACKAGE_SUFFIX not set for this job"
  rm -rf ${TRAVIS_BUILD_DIR}/artifacts
else
  cd ${TRAVIS_BUILD_DIR}/artifacts/staging
  tar czf ${TRAVIS_BUILD_DIR}/artifacts/${PACKAGE_PREFIX}-${TRAVIS_BRANCH}-travisjob-${TRAVIS_JOB_NUMBER}-${TRAVIS_COMMIT}-${PACKAGE_SUFFIX}.tar.gz *
  cd ${TRAVIS_BUILD_DIR}
  ls -R ./artifacts/staging/*
fi
ls -R ./artifacts/*
