# A simple Docker image for building the native libimageflow.so library.
# The project source code should be mounted as a volume.
FROM rust:latest

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    nasm \
    pkg-config \
    libssl-dev \
    libpng-dev \
    && rm -rf /var/lib/apt/lists/*

# Set a working directory
WORKDIR /work
