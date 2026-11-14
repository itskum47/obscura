#!/bin/sh
# Obscura Unified Native Installer (macOS & Linux)
# https://github.com/itskum47/obscura
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh
#   or with options:
#   sh install.sh [--version <vX.Y.Z>] [--variant <default|stealth|no-render|no-render-stealth>] [--install-dir <path>] [-y]

set -eu

REPO="itskum47/obscura"
DEFAULT_INSTALL_DIR="$HOME/.local/bin"
INSTALL_DIR="${OBSCURA_INSTALL_DIR:-$DEFAULT_INSTALL_DIR}"
VERSION="${OBSCURA_VERSION:-latest}"
VARIANT="${OBSCURA_VARIANT:-default}"
NON_INTERACTIVE="${CI:-0}"

# Color codes (only when attached to a terminal)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    BOLD=''
    RESET=''
fi

info() {
    printf "${BLUE}${BOLD}==>${RESET} ${BOLD}%s${RESET}\n" "$1"
}

success() {
    printf "${GREEN}${BOLD}==>${RESET} ${GREEN}%s${RESET}\n" "$1"
}

warn() {
    printf "${YELLOW}${BOLD}warning:${RESET} %s\n" "$1" >&2
}

error() {
    printf "${RED}${BOLD}error:${RESET} %s\n" "$1" >&2
    exit 1
}

# Parse CLI flags
while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            [ $# -ge 2 ] || error "--version requires an argument"
            VERSION="$2"
            shift 2
            ;;
        --variant)
            [ $# -ge 2 ] || error "--variant requires an argument"
            VARIANT="$2"
            shift 2
            ;;
        --install-dir)
            [ $# -ge 2 ] || error "--install-dir requires an argument"
            INSTALL_DIR="$2"
            shift 2
            ;;
        -y|--yes)
            NON_INTERACTIVE="1"
            shift
            ;;
        -h|--help)
            cat <<EOF
Obscura Native Installer

Usage:
  install.sh [options]

Options:
  --version <ver>      Install specific version (default: latest, e.g. v0.3.0)
  --variant <variant>  Binary variant: default, stealth, no-render, no-render-stealth (default: default)
  --install-dir <dir>  Installation destination directory (default: ~/.local/bin)
  -y, --yes            Non-interactive installation without prompts
  -h, --help           Show this help message

Environment variables:
  OBSCURA_VERSION      Same as --version
  OBSCURA_VARIANT      Same as --variant
  OBSCURA_INSTALL_DIR  Same as --install-dir
EOF
            exit 0
            ;;
        *)
            error "Unknown argument: $1. Run with --help for usage."
            ;;
    esac
done

# Validate variant
case "$VARIANT" in
    default|stealth|no-render|no-render-stealth) ;;
    *)
        error "Invalid variant '$VARIANT'. Supported variants: default, stealth, no-render, no-render-stealth"
        ;;
esac

# Detect OS
OS_RAW="$(uname -s)"
case "$OS_RAW" in
    Darwin)
        OS="macos"
        ;;
    Linux)
        OS="linux"
        ;;
    *)
        error "Unsupported operating system: $OS_RAW. Obscura native prebuilt binaries support macOS, Linux, and Windows (via install.ps1)."
        ;;
esac

# Detect Architecture
ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
    x86_64|amd64)
        ARCH="x86_64"
        ;;
    aarch64|arm64)
        ARCH="aarch64"
        ;;
    *)
        error "Unsupported CPU architecture: $ARCH_RAW. Supported architectures are x86_64 and aarch64 (ARM64)."
        ;;
esac

# Verify HTTP downloader
if command -v curl >/dev/null 2>&1; then
    DOWNLOADER="curl"
elif command -v wget >/dev/null 2>&1; then
    DOWNLOADER="wget"
else
    error "Neither 'curl' nor 'wget' was found on your system. Please install curl or wget."
fi

download_file() {
    url="$1"
    output="$2"
    case "$url" in
        file://*)
            src="${url#file://}"
            cp "$src" "$output"
            ;;
        *)
            if [ "$DOWNLOADER" = "curl" ]; then
                curl -fsSL "$url" -o "$output"
            else
                wget -qO "$output" "$url"
            fi
            ;;
    esac
}

download_stdout() {
    url="$1"
    case "$url" in
        file://*)
            src="${url#file://}"
            cat "$src"
            ;;
        *)
            if [ "$DOWNLOADER" = "curl" ]; then
                curl -fsSL "$url"
            else
                wget -qO- "$url"
            fi
            ;;
    esac
}

# Verify SHA256 tool
if command -v sha256sum >/dev/null 2>&1; then
    SHA_TOOL="sha256sum"
elif command -v shasum >/dev/null 2>&1; then
    SHA_TOOL="shasum -a 256"
elif command -v openssl >/dev/null 2>&1; then
    SHA_TOOL="openssl"
else
    error "No SHA-256 tool found. Please install sha256sum, shasum, or openssl."
fi

calc_sha256() {
    file="$1"
    if [ "$SHA_TOOL" = "openssl" ]; then
        openssl dgst -sha256 "$file" | awk '{print $NF}'
    else
        $SHA_TOOL "$file" | awk '{print $1}'
    fi
}

# Resolve target version
info "Resolving release version ($VERSION)..."
if [ "$VERSION" = "latest" ]; then
    LATEST_JSON=$(download_stdout "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)
    if [ -n "$LATEST_JSON" ]; then
        TAG=$(printf '%s' "$LATEST_JSON" | grep '"tag_name":' | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')
    else
        TAG=""
    fi

    # Fallback to redirect location check if GitHub API is rate-limited
    if [ -z "$TAG" ]; then
        if [ "$DOWNLOADER" = "curl" ]; then
            TAG=$(curl -sI "https://github.com/${REPO}/releases/latest" | grep -i '^location:' | sed -E 's|.*/tag/(.*)|\1|' | tr -d '\r\n')
        fi
    fi

    if [ -z "$TAG" ]; then
        error "Unable to resolve latest release tag from GitHub. Specify a version manually with --version <tag>."
    fi
else
    TAG="$VERSION"
    case "$TAG" in
        v*) ;;
        *) TAG="v$TAG" ;;
    esac
fi

# Formulate asset filename based on variant
case "$VARIANT" in
    default)
        ARCHIVE_NAME="obscura-${ARCH}-${OS}.tar.gz"
        ;;
    *)
        ARCHIVE_NAME="obscura-${ARCH}-${OS}-${VARIANT}.tar.gz"
        ;;
esac

RELEASE_BASE_URL="${OBSCURA_DOWNLOAD_BASE:-https://github.com/${REPO}/releases/download/${TAG}}"
DOWNLOAD_URL="${RELEASE_BASE_URL}/${ARCHIVE_NAME}"
CHECKSUMS_URL="${RELEASE_BASE_URL}/checksums.txt"

info "Selected Obscura ${TAG} (${VARIANT} variant) for ${OS}-${ARCH}"
info "Archive: ${ARCHIVE_NAME}"
info "Install target: ${INSTALL_DIR}"

# Create secure temporary directory
TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'obscura-install')"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

ARCHIVE_PATH="$TMP_DIR/$ARCHIVE_NAME"
CHECKSUMS_PATH="$TMP_DIR/checksums.txt"

# Download archive and checksum manifest
info "Downloading ${ARCHIVE_NAME}..."
if ! download_file "$DOWNLOAD_URL" "$ARCHIVE_PATH"; then
    error "Failed to download $DOWNLOAD_URL. Please verify version and platform support."
fi

info "Downloading checksum manifest (checksums.txt)..."
if ! download_file "$CHECKSUMS_URL" "$CHECKSUMS_PATH"; then
    error "Failed to download checksums from $CHECKSUMS_URL. Release integrity verification is mandatory."
fi

# Verify checksum
info "Verifying SHA-256 checksum..."
EXPECTED_SHA=$(grep "$ARCHIVE_NAME" "$CHECKSUMS_PATH" | awk '{print $1}' | head -n 1 || true)
if [ -z "$EXPECTED_SHA" ]; then
    error "Archive $ARCHIVE_NAME not found in release checksums.txt. Aborting installation."
fi

ACTUAL_SHA=$(calc_sha256 "$ARCHIVE_PATH")
if [ "$EXPECTED_SHA" != "$ACTUAL_SHA" ]; then
    error "Checksum verification failed for $ARCHIVE_NAME!\n  Expected: $EXPECTED_SHA\n  Computed: $ACTUAL_SHA\nCorrupted or tampered download. Aborting."
fi
success "Checksum verified: ${ACTUAL_SHA}"

# Safe extraction
info "Extracting binaries..."
EXTRACT_DIR="$TMP_DIR/extracted"
mkdir -p "$EXTRACT_DIR"

if ! tar -xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIR"; then
    error "Extraction failed for $ARCHIVE_NAME"
fi

if [ ! -f "$EXTRACT_DIR/obscura" ]; then
    error "Corrupted archive: 'obscura' binary is missing from $ARCHIVE_NAME"
fi
if [ ! -f "$EXTRACT_DIR/obscura-worker" ]; then
    error "Corrupted archive: 'obscura-worker' binary is missing from $ARCHIVE_NAME"
fi

# Ensure install directory exists and is writable
mkdir -p "$INSTALL_DIR"
if [ ! -w "$INSTALL_DIR" ]; then
    error "Destination directory $INSTALL_DIR is not writable by current user $(id -un)."
fi

# Atomic installation of both binaries with full multi-binary transactional rollback
info "Installing 'obscura' and 'obscura-worker' to $INSTALL_DIR..."

OBSCURA_SRC="$EXTRACT_DIR/obscura"
OBSCURA_DST="$INSTALL_DIR/obscura"
OBSCURA_BAK="$INSTALL_DIR/obscura.bak.$$"
OBSCURA_TMP="$INSTALL_DIR/obscura.tmp.$$"

WORKER_SRC="$EXTRACT_DIR/obscura-worker"
WORKER_DST="$INSTALL_DIR/obscura-worker"
WORKER_BAK="$INSTALL_DIR/obscura-worker.bak.$$"
WORKER_TMP="$INSTALL_DIR/obscura-worker.tmp.$$"

# Step 1: Backup existing binaries if present
[ -f "$OBSCURA_DST" ] && cp -p "$OBSCURA_DST" "$OBSCURA_BAK"
[ -f "$WORKER_DST" ] && cp -p "$WORKER_DST" "$WORKER_BAK"

# Step 2: Prepare temporary staged binaries with execution permissions
cp "$OBSCURA_SRC" "$OBSCURA_TMP"
chmod 755 "$OBSCURA_TMP"
cp "$WORKER_SRC" "$WORKER_TMP"
chmod 755 "$WORKER_TMP"

# Step 3: Install first binary (obscura)
if ! mv "$OBSCURA_TMP" "$OBSCURA_DST"; then
    warn "Failed to install obscura. Rolling back..."
    [ -f "$OBSCURA_BAK" ] && mv "$OBSCURA_BAK" "$OBSCURA_DST"
    [ -f "$OBSCURA_TMP" ] && rm -f "$OBSCURA_TMP"
    [ -f "$WORKER_TMP" ] && rm -f "$WORKER_TMP"
    [ -f "$WORKER_BAK" ] && rm -f "$WORKER_BAK"
    error "Installation failed during move to $OBSCURA_DST"
fi

# Step 4: Install second binary (obscura-worker)
if ! mv "$WORKER_TMP" "$WORKER_DST"; then
    warn "Failed to install obscura-worker. Rolling back entire installation..."
    if [ -f "$OBSCURA_BAK" ]; then
        mv "$OBSCURA_BAK" "$OBSCURA_DST"
    else
        rm -f "$OBSCURA_DST"
    fi
    if [ -f "$WORKER_BAK" ]; then
        mv "$WORKER_BAK" "$WORKER_DST"
    fi
    [ -f "$WORKER_TMP" ] && rm -f "$WORKER_TMP"
    error "Installation failed during move to $WORKER_DST. Restored previous binaries."
fi

# Step 5: Both installations succeeded cleanly. Remove backups.
[ -f "$OBSCURA_BAK" ] && rm -f "$OBSCURA_BAK"
[ -f "$WORKER_BAK" ] && rm -f "$WORKER_BAK"

success "Obscura binaries successfully installed!"

# Check PATH
PATH_CONFIGURED=0
case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        PATH_CONFIGURED=1
        ;;
esac

# Verification run
INSTALLED_BIN="$INSTALL_DIR/obscura"
if [ -x "$INSTALLED_BIN" ]; then
    INSTALLED_VER=$("$INSTALLED_BIN" --version 2>/dev/null || echo "Obscura")
    success "Verified executable: $INSTALLED_VER"
else
    error "Installed binary at $INSTALLED_BIN is not executable."
fi

printf "\n"
printf "${BOLD}========================================================================${RESET}\n"
printf "${GREEN}${BOLD}           Obscura has been successfully installed!${RESET}\n"
printf "${BOLD}========================================================================${RESET}\n"
printf "Installed location: %s\n" "$INSTALL_DIR"
printf "Binaries:           obscura, obscura-worker\n"

if [ "$PATH_CONFIGURED" -eq 0 ]; then
    printf "\n"
    printf "${YELLOW}${BOLD}Note: %s is not currently in your system PATH.${RESET}\n" "$INSTALL_DIR"
    printf "To use 'obscura' from any terminal, add it to your profile:\n\n"

    CURRENT_SHELL="$(basename "${SHELL:-sh}")"
    case "$CURRENT_SHELL" in
        zsh)
            printf "  echo 'export PATH=\"%s:\$PATH\"' >> ~/.zshrc\n" "$INSTALL_DIR"
            printf "  source ~/.zshrc\n"
            ;;
        bash)
            printf "  echo 'export PATH=\"%s:\$PATH\"' >> ~/.bashrc\n" "$INSTALL_DIR"
            printf "  source ~/.bashrc\n"
            ;;
        fish)
            printf "  fish_add_path %s\n" "$INSTALL_DIR"
            ;;
        *)
            printf "  echo 'export PATH=\"%s:\$PATH\"' >> ~/.profile\n" "$INSTALL_DIR"
            printf "  source ~/.profile\n"
            ;;
    esac
    printf "\n"
fi

if [ "$OS" = "macos" ]; then
    printf "${BLUE}${BOLD}macOS Security Notice:${RESET}\n"
    printf "  Prebuilt binaries are ad-hoc signed and not notarized by Apple.\n"
    printf "  Gatekeeper quarantine flags are preserved by default.\n"
    printf "  If macOS Gatekeeper prevents execution, authorize in System Settings -> Privacy & Security,\n"
    printf "  or explicitly clear quarantine:\n"
    printf "    xattr -d com.apple.quarantine %s/obscura %s/obscura-worker\n\n" "$INSTALL_DIR" "$INSTALL_DIR"
fi

printf "\n${BOLD}Quick Start:${RESET}\n"
if [ "$PATH_CONFIGURED" -eq 1 ]; then
    printf "  1. Run diagnostics:      obscura doctor\n"
    printf "  2. Fetch a webpage:      obscura fetch https://example.com --eval \"document.title\"\n"
    printf "  3. Start CDP server:     obscura serve\n"
    printf "  4. Launch MCP server:    obscura mcp\n"
else
    printf "  1. Run diagnostics:      %s/obscura doctor\n" "$INSTALL_DIR"
    printf "  2. Fetch a webpage:      %s/obscura fetch https://example.com --eval \"document.title\"\n" "$INSTALL_DIR"
    printf "  3. Start CDP server:     %s/obscura serve\n" "$INSTALL_DIR"
    printf "  4. Launch MCP server:    %s/obscura mcp\n" "$INSTALL_DIR"
fi
printf "\n"
