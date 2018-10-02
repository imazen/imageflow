#!/usr/bin/env bash

./imageflow_build_ubuntu18/build.sh

docker push imazen/imageflow_build_ubuntu18


./imageflow_build_ubuntu16/build.sh

docker push imazen/imageflow_build_ubuntu16


./imageflow_build_ubuntu18_debug/build.sh

docker push imazen/imageflow_build_ubuntu18_debug


./imageflow_base_os/build.sh

docker push imazen/imageflow_base_os

