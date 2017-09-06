#!/bin/bash
set -e #Exit on failure.

#Underscores are prohibited in prerelease tags
export CI_TAG="v0.9-rc1-1"
export PACKAGE_SUFFIX="win64"
export NUGET_RUNTIME="win7-x64"

./pack.sh
