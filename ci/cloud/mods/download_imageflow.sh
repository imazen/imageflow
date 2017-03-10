#!/bin/bash
set -e

mkdir nightly && cd nightly && wget -nv -O ifs.tar.gz https://s3-us-west-1.amazonaws.com/imageflow-nightlies/commits/b02c745686f4270742bdef388fe2d2560e8a2f0a/linux64_sandybridge_glibc223.tar.gz
	tar xvzf ifs.tar.gz && mv ./imageflow_server ../ && cd .. && rm -rf nightly
