#!/bin/bash
set -e #Exit on failure.

#Underscores are prohibited in prerelease tags
export CI_TAG="v0.9-rc1-1"
export PACKAGE_SUFFIX="ubuntu_16_04-x86_64"
export NUGET_RUNTIME="ubuntu.16.04-x64"

./pack.sh
