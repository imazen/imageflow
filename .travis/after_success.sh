#!/bin/bash
if [[ "$(uname -s)" != 'Darwin' ]]; then
    cd ${TRAVIS_BUILD_DIR}
    sudo chmod -R a+rw .
    lcov --directory ./build --capture --output-file coverage.info # capture coverage info
    lcov --remove coverage.info 'tests/*' '.conan/*' '/usr/*' --output-file coverage.info # filter out system and test code
    lcov --list coverage.info # debug before upload
    coveralls-lcov --repo-token ${COVERALLS_TOKEN} coverage.info # uploads to coveralls
fi
