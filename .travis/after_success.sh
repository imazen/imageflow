#!/bin/bash
if [[ "$(uname -s)" != 'Darwin' ]]; then
    if ! [ -z "$TRAVIS_BUILD_DIR" ] && [ "${TRAVIS_BUILD_DIR+xxx}" = "xxx" ]; then
      cd ${TRAVIS_BUILD_DIR}
      sudo chmod -R a+rw .
      lcov --directory ./build --capture --output-file coverage.info # capture coverage info
      lcov --remove coverage.info 'tests/*' '.conan/*' '/usr/*' --output-file coverage.info # filter out system and test code
      lcov --list coverage.info # debug before upload
    fi
    if ! [ -z "$COVERALLS_TOKEN" ]  && [ "${COVERALLS_TOKEN+xxx}" = "xxx" ]; then
       coveralls-lcov --repo-token ${COVERALLS_TOKEN} coverage.info # uploads to coveralls
    fi
fi
