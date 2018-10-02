#!/bin/bash

docker images

docker system df
docker history imazen/imageflow_base_os
docker run "imazen/imageflow_base_os" du -h / | sort -rh | head -n 20

docker history imazen/imageflow_build_ubuntu16
docker history imazen/imageflow_build_ubuntu18
docker history imazen/imageflow_build_ubuntu18_debug




#docker run imazen/build_if_gcc48 du -h / | grep '[0-9\.]\+M'

