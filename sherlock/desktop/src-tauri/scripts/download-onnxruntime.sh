#!/usr/bin/env bash
# Download the correct ONNX Runtime shared library for the current platform.
# Places the library in src-tauri/lib/ for development and bundling.
set -euo pipefail

ORT_VERSION="1.23.0"  # Must match ort crate's expected version (ort 2.0.0-rc.11 -> ORT 1.23)
BASE_URL="https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="${SCRIPT_DIR}/../lib"
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
        LIB_NAME="libonnxruntime.so"
        INNER_DIR="onnxruntime-linux-${ARCH/#x86_64/x64}-${ORT_VERSION}"
        ;;
    Darwin)
        case "${ARCH}" in
            x86_64)        ARCHIVE="onnxruntime-osx-x86_64-${ORT_VERSION}.tgz"
                           INNER_DIR="onnxruntime-osx-x86_64-${ORT_VERSION}" ;;
            arm64|aarch64) ARCHIVE="onnxruntime-osx-arm64-${ORT_VERSION}.tgz"
                           INNER_DIR="onnxruntime-osx-arm64-${ORT_VERSION}" ;;
            *)             echo "Unsupported macOS arch: ${ARCH}"; exit 1 ;;
        esac
        LIB_NAME="libonnxruntime.dylib"
        ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
        ARCHIVE="onnxruntime-win-x64-${ORT_VERSION}.zip"
        LIB_NAME="onnxruntime.dll"
        INNER_DIR="onnxruntime-win-x64-${ORT_VERSION}"
        ;;
    *)
        echo "Unsupported OS: ${OS}"
        exit 1
        ;;
esac

TARGET_PATH="${LIB_DIR}/${LIB_NAME}"

# Skip if already downloaded
if [ -f "${TARGET_PATH}" ]; then
    echo "ONNX Runtime already present at ${TARGET_PATH}"
    exit 0
fi

echo "Downloading ONNX Runtime ${ORT_VERSION} for ${OS}/${ARCH}..."
TMPDIR="$(mktemp -d)"
trap 'rm -rf "${TMPDIR}"' EXIT

if [[ "${ARCHIVE}" == *.zip ]]; then
    curl -fSL "${BASE_URL}/${ARCHIVE}" -o "${TMPDIR}/ort.zip"
    unzip -q "${TMPDIR}/ort.zip" -d "${TMPDIR}"
else
    curl -fSL "${BASE_URL}/${ARCHIVE}" -o "${TMPDIR}/ort.tgz"
    tar -xzf "${TMPDIR}/ort.tgz" -C "${TMPDIR}"
fi

cp "${TMPDIR}/${INNER_DIR}/lib/${LIB_NAME}" "${TARGET_PATH}"

# Also copy versioned symlink targets on Linux (e.g. libonnxruntime.so.1.23.0)
if [ "${OS}" = "Linux" ]; then
    for f in "${TMPDIR}/${INNER_DIR}/lib/${LIB_NAME}".*; do
        [ -f "$f" ] && cp "$f" "${LIB_DIR}/"
    done
fi

echo "ONNX Runtime installed to ${TARGET_PATH}"
