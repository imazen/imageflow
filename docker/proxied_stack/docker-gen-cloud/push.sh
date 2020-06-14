#!/bin/bash
set -e


docker build -t "imazen/docker-gen-cloud" . 

docker push imazen/docker-gen-cloud