#!/usr/bin/env bash
# Sloth Installer — Linux & macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/AymericChaverot/sloth/main/install.sh | bash
set -euo pipefail

REPO="AymericChaverot/sloth"
INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="sloth"

# ─── Colors ──────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC}  $1"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# ─── Detect platform ────────────────────────────────────
detect_target() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)
            case "$arch" in
                x86_64)  echo "x86_64-unknown-linux-gnu" ;;
                *)       error "Unsupported architecture: $arch. Only x86_64 is supported on Linux." ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                arm64|aarch64) echo "aarch64-apple-darwin" ;;
                x86_64)        echo "x86_64-apple-darwin" ;;
                *)             error "Unsupported architecture: $arch" ;;
            esac
            ;;
        *)
            error "Unsupported OS: $os. Use install.ps1 for Windows."
            ;;
    esac
}

# ─── Fetch latest release tag ───────────────────────────
get_latest_version() {
    local url="https://api.github.com/repos/${REPO}/releases/latest"
    if command -v curl &>/dev/null; then
        curl -fsSL "$url" | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/'
    elif command -v wget &>/dev/null; then
        wget -qO- "$url" | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/'
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# ─── Download and install ───────────────────────────────
install() {
    local target version asset_name download_url tmp_dir

    target="$(detect_target)"
    info "Detected platform: ${target}"

    info "Fetching latest release..."
    version="$(get_latest_version)"
    if [ -z "$version" ]; then
        error "Could not determine latest version. Check your internet connection or if the repository has releases."
    fi
    ok "Latest version: ${version}"

    asset_name="${BINARY_NAME}-${target}.tar.gz"
    download_url="https://github.com/${REPO}/releases/download/${version}/${asset_name}"

    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' EXIT

    info "Downloading ${asset_name}..."
    if command -v curl &>/dev/null; then
        curl -fsSL "$download_url" -o "${tmp_dir}/${asset_name}"
    else
        wget -q "$download_url" -O "${tmp_dir}/${asset_name}"
    fi
    ok "Downloaded successfully."

    info "Extracting..."
    tar xzf "${tmp_dir}/${asset_name}" -C "$tmp_dir"
    ok "Extracted."

    # Ensure install directory exists
    mkdir -p "$INSTALL_DIR"

    info "Installing to ${INSTALL_DIR}/${BINARY_NAME}..."
    mv "${tmp_dir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    ok "Installed ${BINARY_NAME} ${version} to ${INSTALL_DIR}/"

    # ─── PATH configuration ─────────────────────────────
    if echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        ok "${INSTALL_DIR} is already in your PATH."
    else
        warn "${INSTALL_DIR} is not in your PATH."

        local shell_config=""
        case "${SHELL:-}" in
            */zsh)  shell_config="$HOME/.zshrc" ;;
            */bash)
                if [ -f "$HOME/.bash_profile" ]; then
                    shell_config="$HOME/.bash_profile"
                else
                    shell_config="$HOME/.bashrc"
                fi
                ;;
            */fish) shell_config="$HOME/.config/fish/config.fish" ;;
        esac

        if [ -n "$shell_config" ]; then
            echo "" >> "$shell_config"
            echo "# Added by sloth installer" >> "$shell_config"
            if [[ "$shell_config" == *"fish"* ]]; then
                echo "set -gx PATH \$PATH $INSTALL_DIR" >> "$shell_config"
            else
                echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$shell_config"
            fi
            ok "Added ${INSTALL_DIR} to PATH in ${shell_config}"
            warn "Run 'source ${shell_config}' or restart your terminal to apply."
        else
            warn "Could not detect shell config. Add this manually to your shell profile:"
            echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
        fi
    fi

    echo ""
    echo -e "${GREEN}✅ sloth ${version} installed successfully!${NC}"
    echo -e "   Run ${CYAN}sloth --help${NC} to get started."
}

install
