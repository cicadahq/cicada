#!/bin/sh

set -eu

print_warning() {
    printf "\033[33mWarning\033[0m: %s\n" "$1"
}   

print_error() {
    printf "\033[31mError\033[0m: %s\nTo report this issue go to https://github.com/cicadahq/cicada/issues\n" "$1"
}

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
    ARCHIVE="cicada-x86_64-apple-darwin.tar.gz"
elif [ "$UNAME" = "Darwin" ] && [ "$ARCH" = "aarch64" ]; then
    ARCHIVE="cicada-aarch64-apple-darwin.tar.gz"
elif [ "$UNAME" = "Linux" ] && [ "$ARCH" = "x86_64" ]; then
    ARCHIVE="cicada-x86_64-unknown-linux-gnu.tar.gz"
else
    print_error "Unsupported OS or Architecture: $UNAME $ARCH"
    exit 1
fi

# Check for curl
if ! command -v curl >/dev/null 2>&1; then
    print_error "curl could not be found, please install curl"
    # TODO: Add recommended installation command for each OS
    exit 1
fi

# Check for docker
if ! command -v docker >/dev/null 2>&1; then
    print_warning "docker could not be found, you will not be able to use cicada"

    if [ "$UNAME" = "Darwin" ]; then
        echo "If you are using brew, you can install docker with: brew install --cask docker"
    fi
fi

# Check for deno
if ! command -v deno >/dev/null 2>&1; then
    print_warning "deno could not be found, you will not be able to use cicada"

    if [ "$UNAME" = "Darwin" ]; then
        echo "If you are using brew, you can install deno with: brew install deno"
    fi
fi

# make a temp directory to download the files
TMP_DIR=$(mktemp -d)

curl -fSsL -o "$TMP_DIR/$ARCHIVE" "https://github.com/cicadahq/cicada/releases/latest/download/$ARCHIVE"

# extract the file
tar -xvf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR" >/dev/null

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

# clean up
rm -rf "$TMP_DIR"

if [ "$USER_ID" -eq 0 ]; then
    echo "cicada has been installed to /usr/local/bin"
    echo
    echo "Run 'cicada init' in your project to get started"
else
    if command -v cicada >/dev/null; then
        echo "cicada has been installed to ~/.local/bin/cicada"
        echo
        echo "Run 'cicada init' in your project to get started"
    else
        case $SHELL in	
        */zsh)
            # Check for ZDOTDIR
            if [ -n "${ZDOTDIR:-}" ]; then
                shell_profile="$ZDOTDIR/.zshrc"
            else
                shell_profile="$HOME/.zshrc"
            fi
            ;;
        */bash)
            shell_profile="$HOME/.bashrc"
            ;;
        *)
            # Error out if we don't know what shell we're using
            print_warning "Manually add cicada to your PATH, if you are in a posix shell:"	
            echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""	
            echo
            echo "Then restart your shell and run 'cicada init' in your project to get started"
            exit 0
        esac

        # Add cicada to the path
        echo "export PATH=\"\$HOME/.local/bin:\$PATH\"" >> "$shell_profile"
        
        echo "cicada has been installed to ~/.local/bin/cicada"
        echo
        echo "Restart your shell and run 'cicada init' in your project to get started"
    fi
fi
