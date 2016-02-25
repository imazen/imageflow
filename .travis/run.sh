#!/bin/bash

set -e
set -x

if [[ "$(uname -s)" == 'Darwin' ]]; then
    if which pyenv > /dev/null; then
        eval "$(pyenv init -)"
    fi
    pyenv activate conan
    # Pending osx configuration
else
    sudo docker run --rm -v $(pwd):/home/conan lasote/conangcc48 /bin/bash -c "sudo pip install conan --upgrade && .travis/run_tests.sh"	
fi

