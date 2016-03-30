#!/bin/bash
if [[ "$(uname -s)" != 'Darwin' ]]; then
    cd ${TRAVIS_BUILD_DIR}
    lcov --directory . --zerocounters
fi
