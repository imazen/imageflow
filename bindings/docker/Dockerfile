# Monolithic Dockerfile for generating all language bindings.
# This environment is designed to mirror the local development setup and be used in CI.

# Use a Node.js base image as it's Debian-based and includes npm.
FROM node:latest

# Copy the universal dependency installation script into the image's PATH.
COPY scripts/install_binding_deps.sh /usr/local/bin/install_binding_deps.sh

# Run the script to install all system packages, language runtimes, and tools.
# This script is idempotent and can be run safely multiple times.
RUN install_binding_deps.sh

# Set the default working directory. The project root will be mounted here.
WORKDIR /work
