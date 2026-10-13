#!/bin/sh
# Installs the Stellwerk agent {{VERSION}} for the current user, pairs it with a Zentrale and
# starts it in the background. The Zentrale serves this script at /install.sh:
#
#   curl -fsSL https://zentrale.example/install.sh | sh -s -- https://zentrale.example CODE
#
# Without a code the device waits for approval under Devices (pairing in the LAN). Running it
# again on a paired device updates the agent and keeps the pairing.
set -eu

version="{{VERSION}}"
releases="https://github.com/firsttris/stellwerk/releases/download/v$version"
docs="https://firsttris.github.io/stellwerk/installation.html"

server="${1:?usage: install.sh <server> [code]}"
code="${2:-}"

fail() {
    echo "stellwerk: $*" >&2
    exit 1
}

case "$(uname -s)" in
    Linux) os=linux ;;
    Darwin) os=macos ;;
    *) fail "no agent for $(uname -s); see $docs" ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) fail "no agent for $(uname -m); see $docs" ;;
esac

fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$2" "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$2" "$1"
    else
        fail "neither curl nor wget is installed"
    fi
}

sha256_check() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum -c - >/dev/null
    else
        shasum -a 256 -c - >/dev/null
    fi
}

asset="stellwerk-agent-$os-$arch"
bin_dir="$HOME/.local/bin"
agent="$bin_dir/stellwerk-agent"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $asset $version ..."
fetch "$releases/$asset" "$tmp/$asset" || fail "could not download $releases/$asset"
fetch "$releases/SHA256SUMS" "$tmp/SHA256SUMS" || fail "could not download the checksums"
(cd "$tmp" && grep " \*\{0,1\}$asset\$" SHA256SUMS | sha256_check) ||
    fail "the download does not match its checksum"

mkdir -p "$bin_dir"
install -m 755 "$tmp/$asset" "$agent"
if [ "$os" = macos ]; then
    xattr -d com.apple.quarantine "$agent" 2>/dev/null || true
fi

if [ -n "$code" ]; then
    "$agent" join --server "$server" "$code"
else
    "$agent" join --server "$server"
fi
"$agent" service install

case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "The agent is in $bin_dir, which is not in your PATH." ;;
esac
