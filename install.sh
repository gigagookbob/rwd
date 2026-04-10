#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ]; then
    echo "Error: This installer requires bash." >&2
    echo "Run: curl -fsSL https://raw.githubusercontent.com/gigagookbob/rwd/main/install.sh | bash" >&2
    exit 1
fi

set -euo pipefail

# rwd install script
# Usage: curl -fsSL https://raw.githubusercontent.com/gigagookbob/rwd/main/install.sh | bash

REPO="gigagookbob/rwd"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="rwd"

# Fetch latest release tag.
# GitHub API may return a compact one-line JSON payload, so extract only the
# "tag_name" field value to avoid accidentally capturing keys like "mentions_count".
VERSION=$(
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep -oE '"tag_name"[[:space:]]*:[[:space:]]*"[^"]+"' \
        | head -n1 \
        | sed -E 's/^"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)"$/\1/'
)

if [ -z "$VERSION" ]; then
    echo "Error: Failed to fetch latest release version."
    exit 1
fi

echo "Installing rwd ${VERSION}..."

# Detect architecture
ARCH=$(uname -m)
OS=$(uname -s)

case "${OS}-${ARCH}" in
    Darwin-arm64)
        ASSET="rwd-aarch64-apple-darwin.tar.gz"
        ;;
    Darwin-x86_64)
        ASSET="rwd-x86_64-apple-darwin.tar.gz"
        ;;
    Linux-x86_64)
        ASSET="rwd-x86_64-unknown-linux-gnu.tar.gz"
        ;;
    *)
        echo "Error: Unsupported platform: ${OS}-${ARCH}"
        echo "Build from source: cargo install --git https://github.com/${REPO}.git"
        exit 1
        ;;
esac

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"

# Download to temp directory
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Downloading: ${DOWNLOAD_URL}"
curl -fsSL "$DOWNLOAD_URL" -o "${TMP_DIR}/${ASSET}"

# Extract archive
tar -xzf "${TMP_DIR}/${ASSET}" -C "$TMP_DIR"

# Locate extracted binary
EXTRACTED=$(find "$TMP_DIR" -type f -name "rwd*" ! -name "*.tar.gz" | head -1)

if [ -z "$EXTRACTED" ]; then
    echo "Error: Failed to locate extracted binary."
    exit 1
fi

# Install binary
chmod +x "$EXTRACTED"
if [ -w "$INSTALL_DIR" ]; then
    mv "$EXTRACTED" "${INSTALL_DIR}/${BINARY_NAME}"
else
    echo "Administrator privileges required to write to ${INSTALL_DIR}."
    sudo mv "$EXTRACTED" "${INSTALL_DIR}/${BINARY_NAME}"
fi

# Create default output directory (~/.rwd/output/)
# This allows immediate use before Obsidian path is configured.
DEFAULT_OUTPUT="${HOME}/.rwd/output"
mkdir -p "$DEFAULT_OUTPUT"

echo ""
echo "rwd ${VERSION} installed successfully!"
echo "Binary location: ${INSTALL_DIR}/${BINARY_NAME}"
echo "Default output directory: ${DEFAULT_OUTPUT}"
echo ""
echo "Next steps:"
echo "  1) Verify install: rwd --version"
echo "  2) Initial setup:  rwd init"
echo "  3) Run analysis:   rwd today"
echo ""
echo "Tip: After \`rwd init\`, config is saved at ~/.config/rwd/config.toml."

case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        echo ""
        echo "Note: ${INSTALL_DIR} is not in PATH for this shell."
        echo "Add this line to ~/.bashrc, then restart your terminal:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac
