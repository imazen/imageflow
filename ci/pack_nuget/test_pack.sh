#!/bin/bash
set -e #Exit on failure.

export CI_TAG="v0.1-prerelease1"
export PACKAGE_SUFFIX="ubuntu_14_04-x86_64"
export NUGET_RUNTIME="ubuntu.14.04-x64"

./pack.sh
