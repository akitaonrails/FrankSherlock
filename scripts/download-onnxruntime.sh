#!/usr/bin/env bash
# Download the correct ONNX Runtime shared library for the current platform.
# Places the library in src-tauri/lib/ for development and bundling.
set -euo pipefail

ORT_VERSION="1.22.0"
BASE_URL="https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="${SCRIPT_DIR}/../sherlock/desktop/src-tauri/lib"
mkdir -p "${LIB_DIR}"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Linux)
        case "${ARCH}" in
            x86_64)  ARCHIVE="onnxruntime-linux-x64-${ORT_VERSION}.tgz" ;;
            aarch64) ARCHIVE="onnxruntime-linux-aarch64-${ORT_VERSION}.tgz" ;;
            *)       echo "Unsupported Linux arch: ${ARCH}"; exit 1 ;;
        esac
        LIB_NAME="libonnxruntime.so.${ORT_VERSION}"
        TARGET_NAME="libonnxruntime.so"
        ;;
    Darwin)
        case "${ARCH}" in
            x86_64)        ARCHIVE="onnxruntime-osx-x86_64-${ORT_VERSION}.tgz" ;;
            arm64|aarch64) ARCHIVE="onnxruntime-osx-arm64-${ORT_VERSION}.tgz" ;;
            *)             echo "Unsupported macOS arch: ${ARCH}"; exit 1 ;;
        esac
        LIB_NAME="libonnxruntime.${ORT_VERSION}.dylib"
        TARGET_NAME="libonnxruntime.dylib"
        ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
        ARCHIVE="onnxruntime-win-x64-${ORT_VERSION}.zip"
        LIB_NAME="onnxruntime.dll"
        TARGET_NAME="onnxruntime.dll"
        ;;
    *)
        echo "Unsupported OS: ${OS}"
        exit 1
        ;;
esac

TARGET_PATH="${LIB_DIR}/${TARGET_NAME}"

# Skip if already downloaded
if [ -f "${TARGET_PATH}" ]; then
    echo "ONNX Runtime already present at ${TARGET_PATH}"
    exit 0
fi

echo "Downloading ONNX Runtime ${ORT_VERSION} for ${OS}/${ARCH}..."
TMPDIR="$(mktemp -d)"
trap 'rm -rf "${TMPDIR}"' EXIT

curl -fSL "${BASE_URL}/${ARCHIVE}" -o "${TMPDIR}/ort_archive"

EXTRACT_DIR="${TMPDIR}/extracted"
mkdir -p "${EXTRACT_DIR}"

case "${ARCHIVE}" in
    *.tgz) tar -xzf "${TMPDIR}/ort_archive" -C "${EXTRACT_DIR}" ;;
    *.zip) unzip -q "${TMPDIR}/ort_archive" -d "${EXTRACT_DIR}" ;;
esac

# Find the library inside the extracted directory (nested under a version dir)
LIB_FILE="$(find "${EXTRACT_DIR}" -name "${LIB_NAME}" -type f | head -1)"
if [ -z "${LIB_FILE}" ]; then
    echo "ERROR: Could not find ${LIB_NAME} in archive"
    exit 1
fi

cp "${LIB_FILE}" "${TARGET_PATH}"
echo "ONNX Runtime installed to ${TARGET_PATH}"
