#!/bin/bash
# Universal script to install dependencies for generating language bindings.
# Can be run in Docker or on a local Debian-based system (like Ubuntu/WSL).

set -e # Exit immediately if a command exits with a non-zero status.

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check if a dpkg package is installed
package_installed() {
    dpkg-query -W -f='${Status}' "$1" 2>/dev/null | grep -q "install ok installed"
}

# Determine if we need to use sudo
if [ "$(id -u)" -ne 0 ]; then
    SUDO="sudo"
else
    SUDO=""
fi

# Ensure apt-get is available
if ! command_exists "apt-get"; then
    echo "Error: apt-get not found. This script is for Debian-based systems." >&2
    exit 1
fi

echo "--- Updating package lists ---"
$SUDO apt-get update

echo "--- Installing required system packages and libraries ---"

PACKAGES_TO_INSTALL=()

# Core build tools
if ! package_installed "build-essential"; then PACKAGES_TO_INSTALL+=("build-essential"); fi
if ! package_installed "default-jre"; then PACKAGES_TO_INSTALL+=("default-jre"); fi

# Image processing libraries
if ! package_installed "libvips-dev"; then PACKAGES_TO_INSTALL+=("libvips-dev"); fi
if ! package_installed "liblcms2-dev"; then PACKAGES_TO_INSTALL+=("liblcms2-dev"); fi

# Language runtimes
if ! command_exists "ruby"; then PACKAGES_TO_INSTALL+=("ruby-full"); fi # ruby-full for dev headers
if ! command_exists "node"; then PACKAGES_TO_INSTALL+=("nodejs"); fi
if ! command_exists "npm"; then PACKAGES_TO_INSTALL+=("npm"); fi
if ! command_exists "wget"; then PACKAGES_TO_INSTALL+=("wget"); fi
if ! command_exists "go"; then PACKAGES_TO_INSTALL+=("golang"); fi
# For building the 'psych' gem
if ! pkg-config --exists yaml-0.1; then PACKAGES_TO_INSTALL+=("libyaml-dev"); fi

if [ ${#PACKAGES_TO_INSTALL[@]} -ne 0 ]; then
    echo "Installing missing packages: ${PACKAGES_TO_INSTALL[*]}"
    $SUDO apt-get install -y "${PACKAGES_TO_INSTALL[@]}"
else
    echo "All required system packages are already installed."
fi

echo "--- Installing OpenAPI Generator CLI ---"
GENERATOR_JAR="/usr/local/lib/openapi-generator-cli.jar"
if [ ! -f "$GENERATOR_JAR" ]; then
    echo "OpenAPI Generator JAR not found. Downloading..."
    GENERATOR_VERSION="7.14.0"
    URL="https://repo1.maven.org/maven2/org/openapitools/openapi-generator-cli/${GENERATOR_VERSION}/openapi-generator-cli-${GENERATOR_VERSION}.jar"
    $SUDO wget "$URL" -O "$GENERATOR_JAR"
else
    echo "OpenAPI Generator JAR is already installed."
fi

echo "--- Updating npm to latest version ---"
$SUDO npm install -g npm@latest

echo "--- Installing Bundler ---"
if ! command_exists "bundle"; then
    echo "Bundler not found. Installing..."
    $SUDO gem install bundler
else
    echo "Bundler is already installed."
fi

echo "--- Dependency installation complete ---"
