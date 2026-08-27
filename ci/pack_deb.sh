#!/bin/bash
# Build a Debian package (.deb) for imageflow from already-built release
# binaries. Runs after the cargo/cross build on the glibc Linux matrix jobs
# (imazen/imageflow#400). Uses dpkg-deb, which ships on the ubuntu runners; no
# cargo-deb, no debian/ directory.
#
# Package layout:
#   /usr/bin/imageflow_tool
#   /usr/lib/libimageflow.so
#   /usr/include/imageflow.h            (bindings/headers/imageflow_default.h)
#   /usr/share/doc/imageflow/copyright  (LICENSE)
#
# Outputs (mirroring pack_artifacts.sh):
#   ./artifacts/github/imageflow_<version>_<arch>.deb
#   ./artifacts/upload/releases/<ref>/imageflow_<version>_<arch>.deb
#   ./artifacts/upload/commits/<sha>/<suffix>/imageflow.deb
#   ./artifacts/upload/commits/latest/<suffix>/imageflow.deb
set -e
set -o pipefail

# ------------------------------------------------------------------------------
# Required environment variables (passed from the workflow):
# - REL_BINARIES_DIR: directory holding the release binaries (trailing slash)
# - IMAGEFLOW_TOOL: tool file name (imageflow_tool)
# - LIBIMAGEFLOW_DYNAMIC: shared library file name (libimageflow.so)
# - DEB_ARCH: Debian architecture (amd64, arm64)
# - GITHUB_REF_NAME: git ref name (tag or branch); tags like v2.1.1-rc11 drive the version
# - GITHUB_SHA: git commit SHA
# - MATRIX_COMMIT_SUFFIX: package-suffix of the current matrix job (linux-x64, ...)
# Optional:
# - DEB_MIN_GLIBC: minimum glibc the binaries need (becomes `Depends: libc6 (>= X)`)
# - DEB_PACKAGE_NAME: defaults to `imageflow`
# - DEB_VERSION: overrides the version derived from GITHUB_REF_NAME
# - DEB_HEADER: header to install as /usr/include/imageflow.h
#               (defaults to bindings/headers/imageflow_default.h)
# ------------------------------------------------------------------------------

required_vars=(
    "REL_BINARIES_DIR"
    "IMAGEFLOW_TOOL"
    "LIBIMAGEFLOW_DYNAMIC"
    "DEB_ARCH"
    "GITHUB_REF_NAME"
    "GITHUB_SHA"
    "MATRIX_COMMIT_SUFFIX"
)
for var in "${required_vars[@]}"; do
    if [ -z "${!var}" ]; then
        echo "Error: Required environment variable $var is not set" >&2
        exit 1
    fi
done

if ! command -v dpkg-deb >/dev/null 2>&1; then
    echo "Error: dpkg-deb is required to build .deb packages" >&2
    exit 1
fi

if [[ "${REL_BINARIES_DIR}" != */ ]]; then
    echo "Error: REL_BINARIES_DIR must end in a slash" >&2
    exit 1
fi

TOOL_PATH="${REL_BINARIES_DIR}${IMAGEFLOW_TOOL}"
LIB_PATH="${REL_BINARIES_DIR}${LIBIMAGEFLOW_DYNAMIC}"
HEADER_PATH="${DEB_HEADER:-bindings/headers/imageflow_default.h}"
for f in "$TOOL_PATH" "$LIB_PATH" "$HEADER_PATH" LICENSE; do
    if [ ! -f "$f" ]; then
        echo "Error: required file not found: $f" >&2
        exit 1
    fi
done

PACKAGE_NAME="${DEB_PACKAGE_NAME:-imageflow}"

# ------------------------------------------------------------------------------
# Version: Debian versions must start with a digit; `~` sorts before the bare
# version, so pre-releases (v2.1.1-rc11 -> 2.1.1~rc11) upgrade cleanly to the
# final release. Non-tag builds get 0.0.0+git.<sha> so they never outrank a
# real release.
# ------------------------------------------------------------------------------
deb_version_from_ref() {
    local ref="$1"
    local v="${ref#v}"
    if [[ "$v" =~ ^([0-9]+\.[0-9]+\.[0-9]+)(-(.+))?$ ]]; then
        local base="${BASH_REMATCH[1]}"
        local pre="${BASH_REMATCH[3]}"
        if [ -n "$pre" ]; then
            # Debian version chars: alnum . + ~ (no '-', that separates the revision)
            pre="$(echo "$pre" | tr -c 'A-Za-z0-9.+~\n' '.')"
            echo "${base}~${pre}"
        else
            echo "$base"
        fi
        return 0
    fi
    echo "0.0.0+git.${GITHUB_SHA:0:8}"
}
VERSION="${DEB_VERSION:-$(deb_version_from_ref "$GITHUB_REF_NAME")}"

echo "Building ${PACKAGE_NAME} ${VERSION} (${DEB_ARCH}) from ${REL_BINARIES_DIR}"

# ------------------------------------------------------------------------------
# Stage the package tree
# ------------------------------------------------------------------------------
PKG_ROOT="./artifacts/deb/${PACKAGE_NAME}_${VERSION}_${DEB_ARCH}"
rm -rf "$PKG_ROOT"
mkdir -p "$PKG_ROOT/DEBIAN" \
         "$PKG_ROOT/usr/bin" \
         "$PKG_ROOT/usr/lib" \
         "$PKG_ROOT/usr/include" \
         "$PKG_ROOT/usr/share/doc/${PACKAGE_NAME}"

install -m 0755 "$TOOL_PATH" "$PKG_ROOT/usr/bin/imageflow_tool"
install -m 0644 "$LIB_PATH" "$PKG_ROOT/usr/lib/libimageflow.so"
install -m 0644 "$HEADER_PATH" "$PKG_ROOT/usr/include/imageflow.h"
install -m 0644 LICENSE "$PKG_ROOT/usr/share/doc/${PACKAGE_NAME}/copyright"

DEPENDS="libc6"
if [ -n "${DEB_MIN_GLIBC:-}" ]; then
    DEPENDS="libc6 (>= ${DEB_MIN_GLIBC})"
fi

INSTALLED_SIZE_KB="$(du -sk "$PKG_ROOT/usr" | cut -f1)"

cat > "$PKG_ROOT/DEBIAN/control" <<EOF
Package: ${PACKAGE_NAME}
Version: ${VERSION}
Section: graphics
Priority: optional
Architecture: ${DEB_ARCH}
Depends: ${DEPENDS}
Installed-Size: ${INSTALLED_SIZE_KB}
Maintainer: Imazen <support@imazen.io>
Homepage: https://www.imageflow.io
Description: Imageflow - secure, high-performance image processing
 imageflow_tool (command-line image resizing/optimization driven by JSON or
 ImageResizer-style querystrings), libimageflow.so (the C ABI used by the
 language bindings) and the imageflow.h header.
 .
 Built from imazen/imageflow commit ${GITHUB_SHA}.
EOF

# ------------------------------------------------------------------------------
# Build and verify
# ------------------------------------------------------------------------------
DEB_FILE_NAME="${PACKAGE_NAME}_${VERSION}_${DEB_ARCH}.deb"
mkdir -p ./artifacts/deb ./artifacts/github
DEB_OUT="./artifacts/deb/${DEB_FILE_NAME}"
dpkg-deb --root-owner-group --build "$PKG_ROOT" "$DEB_OUT"

echo "--- dpkg-deb --info"
dpkg-deb --info "$DEB_OUT"
echo "--- dpkg-deb --contents"
dpkg-deb --contents "$DEB_OUT"

# ------------------------------------------------------------------------------
# Copies for GitHub release and S3 upload (same layout as pack_artifacts.sh)
# ------------------------------------------------------------------------------
cp "$DEB_OUT" "./artifacts/github/${DEB_FILE_NAME}"
mkdir -p "./artifacts/upload/releases/${GITHUB_REF_NAME}" \
         "./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}" \
         "./artifacts/upload/commits/latest/${MATRIX_COMMIT_SUFFIX}"
cp "$DEB_OUT" "./artifacts/upload/releases/${GITHUB_REF_NAME}/${DEB_FILE_NAME}"
cp "$DEB_OUT" "./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}/${PACKAGE_NAME}.deb"
cp "$DEB_OUT" "./artifacts/upload/commits/latest/${MATRIX_COMMIT_SUFFIX}/${PACKAGE_NAME}.deb"

echo "Created ${DEB_FILE_NAME}:"
ls -l "./artifacts/github/${DEB_FILE_NAME}"
