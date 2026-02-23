#!/usr/bin/env bash
# Download the correct PDFium shared library for the current platform.
# Places the library in src-tauri/lib/ for development and bundling.
set -euo pipefail

PDFIUM_VERSION="7699"  # chromium/7699 — stable release from bblanchon/pdfium-binaries
BASE_URL="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${PDFIUM_VERSION}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="${SCRIPT_DIR}/../lib"
mkdir -p "${LIB_DIR}"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Linux)
        case "${ARCH}" in
            x86_64)  ARCHIVE="pdfium-linux-x64.tgz" ;;
            aarch64) ARCHIVE="pdfium-linux-arm64.tgz" ;;
            *)       echo "Unsupported Linux arch: ${ARCH}"; exit 1 ;;
        esac
        LIB_NAME="lib/libpdfium.so"
        TARGET_NAME="libpdfium.so"
        ;;
    Darwin)
        case "${ARCH}" in
            x86_64)       ARCHIVE="pdfium-mac-x64.tgz" ;;
            arm64|aarch64) ARCHIVE="pdfium-mac-arm64.tgz" ;;
            *)            echo "Unsupported macOS arch: ${ARCH}"; exit 1 ;;
        esac
        LIB_NAME="lib/libpdfium.dylib"
        TARGET_NAME="libpdfium.dylib"
        ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
        ARCHIVE="pdfium-win-x64.tgz"
        LIB_NAME="bin/pdfium.dll"
        TARGET_NAME="pdfium.dll"
        ;;
    *)
        echo "Unsupported OS: ${OS}"
        exit 1
        ;;
esac

TARGET_PATH="${LIB_DIR}/${TARGET_NAME}"

# Skip if already downloaded
if [ -f "${TARGET_PATH}" ]; then
    echo "PDFium already present at ${TARGET_PATH}"
    exit 0
fi

echo "Downloading PDFium ${PDFIUM_VERSION} for ${OS}/${ARCH}..."
TMPDIR="$(mktemp -d)"
trap 'rm -rf "${TMPDIR}"' EXIT

curl -fSL "${BASE_URL}/${ARCHIVE}" -o "${TMPDIR}/pdfium.tgz"
tar -xzf "${TMPDIR}/pdfium.tgz" -C "${TMPDIR}"

cp "${TMPDIR}/${LIB_NAME}" "${TARGET_PATH}"
echo "PDFium installed to ${TARGET_PATH}"
