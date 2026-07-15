#!/bin/sh
# herdr runs this at plugin install/update time (cwd = plugin root).
# Downloads the prebuilt release binary matching this checkout's version and
# platform, verifies its SHA-256, and places it at bin/imebox.
set -eu

REPO="Sawakee/herdr-imebox"
VERSION=$(sed -n 's/^version *= *"\(.*\)"/\1/p' herdr-plugin.toml | head -1)
[ -n "$VERSION" ] || { echo "herdr-imebox: cannot read version from herdr-plugin.toml" >&2; exit 1; }

TARGET=""
case "$(uname -s)/$(uname -m)" in
    Darwin/arm64)          TARGET=aarch64-apple-darwin ;;
    Darwin/x86_64)         TARGET=x86_64-apple-darwin ;;
    Linux/x86_64)          TARGET=x86_64-unknown-linux-musl ;;
    Linux/aarch64 | Linux/arm64) TARGET=aarch64-unknown-linux-musl ;;
esac
if [ -z "$TARGET" ]; then
    echo "herdr-imebox: no prebuilt binary for $(uname -s)/$(uname -m)." >&2
    echo "Build manually instead: cargo build --release && mkdir -p bin && cp target/release/imebox bin/" >&2
    exit 1
fi

ASSET="imebox-v${VERSION}-${TARGET}.tar.gz"
BASE="https://github.com/${REPO}/releases/download/v${VERSION}"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "herdr-imebox: downloading ${ASSET}"
curl -fsSL -o "$TMP/$ASSET" "$BASE/$ASSET"
curl -fsSL -o "$TMP/SHA256SUMS" "$BASE/SHA256SUMS"

if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "$TMP/$ASSET" | awk '{print $1}')
else
    ACTUAL=$(shasum -a 256 "$TMP/$ASSET" | awk '{print $1}')
fi
EXPECTED=$(awk -v a="$ASSET" '$2 == a {print $1}' "$TMP/SHA256SUMS")
if [ -z "$EXPECTED" ] || [ "$ACTUAL" != "$EXPECTED" ]; then
    echo "herdr-imebox: checksum mismatch for $ASSET" >&2
    exit 1
fi

mkdir -p bin
tar -xzf "$TMP/$ASSET" -C bin
chmod +x bin/imebox
echo "herdr-imebox: installed bin/imebox (v${VERSION}, ${TARGET})"
