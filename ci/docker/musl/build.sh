#!/bin/bash

set -e
docker build . -t imazen/musl

export TEST_C=False

../test.sh musl