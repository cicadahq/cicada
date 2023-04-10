#!/bin/sh

set -eu

# Check for curl
if ! command -v curl >/dev/null 2>&1; then
    echo "curl could not be found"
    exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
    echo "Warning: docker could not be found, you will not be able to use cicada"
fi

if ! command -v deno >/dev/null 2>&1; then
    echo "Warning: deno could not be found, you will not be able to use cicada"
fi

# make a temp directory to download the files
TMP_DIR=$(mktemp -d)


# os based
UNAME=$(uname)
ARCH=$(uname -m)

# normalize arch
case $ARCH in
    aarch64 | arm64)
        export ARCH=aarch64
        ;;
    x86_64 | x86-64 | x64 | amd64)
        export ARCH=x86_64
        ;;
esac

if [ "$UNAME" = "Darwin" ] && [ "$ARCH" = "x86_64" ]; then
    PATTERN="cicada-x86_64-apple-darwin.tar.gz"
elif [ "$UNAME" = "Darwin" ] && [ "$ARCH" = "aarch64" ]; then
    PATTERN="cicada-aarch64-apple-darwin.tar.gz"
elif [ "$UNAME" = "Linux" ] && [ "$ARCH" = "x86_64" ]; then
    PATTERN="cicada-x86_64-unknown-linux-gnu.tar.gz"
else
    echo "Unsupported OS or Architecture"
    exit 1
fi

curl -fSsL -o "$TMP_DIR/$PATTERN" "https://github.com/cicadahq/cicada/releases/latest/download/$PATTERN"

# extract the file
tar -xvf "$TMP_DIR/$PATTERN" -C "$TMP_DIR"

USER_ID=$(id -u)

# if root move to /usr/local/bin
if [ "$USER_ID" -eq 0 ]; then
    mkdir -p /usr/local/bin
    DEST=/usr/local/bin/cicada
else
    mkdir -p "$HOME/.local/bin"
    DEST="$HOME/.local/bin/cicada"
fi

# move the file to the current directory
mv "$TMP_DIR/cicada" "$DEST"

if [ "$USER_ID" -eq 0 ]; then
    echo "cicada has been installed to /usr/local/bin"
else
    echo "cicada has been installed to ~/.local/bin"
    echo "Make sure to add $HOME/.local/bin to your PATH"
fi
