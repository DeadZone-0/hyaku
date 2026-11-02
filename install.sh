#!/bin/sh
set -e

# ==============================================================================
# HYAKU (百) - 100-Floor Co-op Tower MMO Client Installer
# https://github.com/DeadZone-0/hyaku
# ==============================================================================

REPO="DeadZone-0/hyaku"
DEFAULT_SERVER="ws://[2a01:4f9:3080:3192::2124]:3000/ws"

CYAN='\033[0;36m'
GOLD='\033[0;33m'
GREEN='\033[0;32m'
RED='\033[0;31m'
BOLD='\033[1m'
RESET='\033[0m'

print_banner() {
    printf "${CYAN}${BOLD}"
    cat << 'EOF'
  ██╗  ██╗██╗   ██╗ █████╗ ██╗  ██╗██╗   ██╗    (百)
  ██║  ██║╚██╗ ██╔╝██╔══██╗██║ ██╔╝██║   ██║
  ███████║ ╚████╔╝ ███████║█████═╝ ██║   ██║    100-FLOOR CO-OP
  ██╔══██║  ╚██╔╝  ██╔══██║██╔═██╗ ██║   ██║    TOWER ASCENSION
  ██║  ██║   ██║   ██║  ██║██║ ╚██╗╚██████╔╝    
  ╚═╝  ╚═╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝
EOF
    printf "${RESET}\n"
}

info() {
    printf "${CYAN}[HYAKU]${RESET} %s\n" "$1"
}

success() {
    printf "${GREEN}${BOLD}[SUCCESS]${RESET} %s\n" "$1"
}

error() {
    printf "${RED}${BOLD}[ERROR]${RESET} %s\n" "$1" >&2
    exit 1
}

# 1. Detect platform and architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)
            case "$ARCH" in
                x86_64)
                    TARGET="x86_64-unknown-linux-musl"
                    ;;
                aarch64|arm64)
                    TARGET="x86_64-unknown-linux-musl" # Musl x86_64 or fallback
                    ;;
                *)
                    error "Unsupported Linux architecture: $ARCH"
                    ;;
            esac
            ;;
        Darwin)
            case "$ARCH" in
                arm64)
                    TARGET="aarch64-apple-darwin"
                    ;;
                x86_64)
                    TARGET="x86_64-apple-darwin"
                    ;;
                *)
                    error "Unsupported macOS architecture: $ARCH"
                    ;;
            esac
            ;;
        *)
            error "Unsupported Operating System: $OS. Hyaku runs on Linux and macOS."
            ;;
    esac
}

# 2. Determine installation directory
determine_install_dir() {
    if [ "$(id -u)" -eq 0 ]; then
        INSTALL_DIR="/usr/local/bin"
    elif [ -w "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
    else
        INSTALL_DIR="$HOME/.local/bin"
        mkdir -p "$INSTALL_DIR"
    fi
}

# 3. Download and extract binary
install_hyaku() {
    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "$TMP_DIR"' EXIT

    ARCHIVE_NAME="hyaku-${TARGET}.tar.gz"
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE_NAME}"

    info "Detected platform: ${TARGET}"
    info "Fetching latest Hyaku release from GitHub..."

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$DOWNLOAD_URL" -o "${TMP_DIR}/${ARCHIVE_NAME}" || {
            error "Failed to download $DOWNLOAD_URL. Check your internet connection or GitHub Releases."
        }
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "${TMP_DIR}/${ARCHIVE_NAME}" "$DOWNLOAD_URL" || {
            error "Failed to download $DOWNLOAD_URL. Check your internet connection or GitHub Releases."
        }
    else
        error "Neither 'curl' nor 'wget' was found on your system. Please install one to continue."
    fi

    # Cryptographic integrity check (SHA256)
    CHECKSUM_URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE_NAME}.sha256"
    info "Verifying SHA256 integrity..."
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$CHECKSUM_URL" -o "${TMP_DIR}/${ARCHIVE_NAME}.sha256" 2>/dev/null || true
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "${TMP_DIR}/${ARCHIVE_NAME}.sha256" "$CHECKSUM_URL" 2>/dev/null || true
    fi

    if [ -f "${TMP_DIR}/${ARCHIVE_NAME}.sha256" ]; then
        cd "$TMP_DIR"
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum -c "${ARCHIVE_NAME}.sha256" >/dev/null 2>&1 || error "SHA256 checksum verification failed! Download may be corrupted."
            info "Checksum verified OK."
        elif command -v shasum >/dev/null 2>&1; then
            shasum -a 256 -c "${ARCHIVE_NAME}.sha256" >/dev/null 2>&1 || error "SHA256 checksum verification failed! Download may be corrupted."
            info "Checksum verified OK."
        fi
        cd - >/dev/null 2>&1
    fi

    info "Extracting package..."
    tar -xzf "${TMP_DIR}/${ARCHIVE_NAME}" -C "$TMP_DIR"

    # Find the extracted hyaku binary
    BIN_PATH="$(find "$TMP_DIR" -type f -name "hyaku" | head -n 1)"
    if [ -z "$BIN_PATH" ]; then
        error "Downloaded archive did not contain 'hyaku' binary."
    fi

    chmod +x "$BIN_PATH"
    info "Installing binary to ${INSTALL_DIR}/hyaku..."
    cp "$BIN_PATH" "${INSTALL_DIR}/hyaku"

    # Install companion auto-updater command
    cat << 'EOF' > "${INSTALL_DIR}/hyaku-update"
#!/bin/sh
set -e
echo "[HYAKU] Checking for updates and reinstalling latest version..."
curl -fsSL https://raw.githubusercontent.com/DeadZone-0/hyaku/main/install.sh | bash
EOF
    chmod +x "${INSTALL_DIR}/hyaku-update"

    success "Hyaku installed successfully to ${INSTALL_DIR}/hyaku"
    success "Hyaku-update installed to ${INSTALL_DIR}/hyaku-update"
}

# 4. Check PATH environment
check_path() {
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) ;;
        *)
            printf "\n${GOLD}${BOLD}[NOTE]${RESET} %s is not in your \$PATH.\n" "$INSTALL_DIR"
            printf "Add it by appending this line to your ~/.bashrc or ~/.zshrc:\n"
            printf "    ${CYAN}export PATH=\"%s:\$PATH\"${RESET}\n\n" "$INSTALL_DIR"
            ;;
    esac
}

print_instructions() {
    printf "\n${CYAN}==============================================================================${RESET}\n"
    printf "${GOLD}${BOLD}Hyaku (百) is ready for ascension!${RESET}\n"
    printf "${CYAN}==============================================================================${RESET}\n\n"
    printf "  ${BOLD}Run Hyaku:${RESET}\n"
    printf "    ${GREEN}hyaku${RESET}                     # Connects to official Nest cloud server\n"
    printf "    ${GREEN}hyaku -u <YourName>${RESET}       # Set your climber name\n"
    printf "    ${GREEN}hyaku --local${RESET}             # Connect to localhost:3000\n\n"
    printf "  ${BOLD}Auto-Update:${RESET}\n"
    printf "    ${GREEN}hyaku-update${RESET}              # Updates hyaku to the latest GitHub release\n\n"
    printf "  ${BOLD}Cloud Server:${RESET} %s\n\n" "$DEFAULT_SERVER"
}

main() {
    print_banner
    detect_platform
    determine_install_dir
    install_hyaku
    check_path
    print_instructions
}

main "$@"
