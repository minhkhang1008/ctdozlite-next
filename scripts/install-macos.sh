#!/bin/bash
# Install / update ctdozlite-next on macOS without the Gatekeeper "unidentified developer" block.
#
#   curl -fsSL https://raw.githubusercontent.com/minhkhang1008/ctdozlite-next/main/scripts/install-macos.sh | bash
#
# Why this works: Gatekeeper only blocks files carrying the `com.apple.quarantine` attribute, which
# browsers add to downloads. curl does not, and the attribute is cleared again after copying. The
# DMG is checked against the release's SHA256SUMS.txt before anything is installed.
#
# Options (environment): CTDOZ_VERSION=0.2.1 (default: latest release),
# CTDOZ_APP_DIR=/Applications, CTDOZ_NO_OPEN=1 (do not launch after installing).
set -euo pipefail

REPO="minhkhang1008/ctdozlite-next"
APP_DIR="${CTDOZ_APP_DIR:-/Applications}"
APP_NAME="ctdozlite.app"

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31mLỗi:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || die "Script này chỉ dành cho macOS."

case "$(uname -m)" in
  arm64) SUFFIX="macos-arm64" ;;
  x86_64) SUFFIX="macos-x86_64" ;;
  *) SUFFIX="macos" ;;
esac

VERSION="${CTDOZ_VERSION:-}"
if [ -z "$VERSION" ]; then
  # Follow the /releases/latest redirect instead of the rate-limited API.
  latest_url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")"
  VERSION="${latest_url##*/v}"
fi
case "$VERSION" in
  ''|*[!0-9.]*) die "Không xác định được phiên bản mới nhất (nhận được: '$VERSION')." ;;
esac

DMG="ctdozlite-${VERSION}-${SUFFIX}.dmg"
BASE="https://github.com/$REPO/releases/download/v${VERSION}"
TMP="$(mktemp -d)"
MOUNT=""
cleanup() {
  if [ -n "$MOUNT" ]; then hdiutil detach -quiet "$MOUNT" 2>/dev/null || true; fi
  rm -rf "$TMP"
}
trap cleanup EXIT

say "Tải ctdozlite-next v${VERSION} (${SUFFIX})…"
curl -fL --progress-bar "$BASE/$DMG" -o "$TMP/$DMG"
curl -fsSL "$BASE/SHA256SUMS.txt" -o "$TMP/SHA256SUMS.txt"

say "Kiểm tra SHA-256…"
expected="$(awk -v f="$DMG" '{ n = $2; sub(/^\*/, "", n); sub(/^\.\//, "", n); if (n == f) print $1 }' "$TMP/SHA256SUMS.txt")"
actual="$(shasum -a 256 "$TMP/$DMG" | awk '{ print $1 }')"
[ -n "$expected" ] || die "SHA256SUMS.txt không có dòng cho $DMG."
[ "$expected" = "$actual" ] || die "SHA-256 không khớp — đã huỷ cài đặt (tệp tải về bị hỏng hoặc bị thay đổi)."

say "Cài vào ${APP_DIR}…"
MOUNT="$TMP/mnt"
mkdir -p "$MOUNT"
hdiutil attach -nobrowse -noautoopen -readonly -mountpoint "$MOUNT" "$TMP/$DMG" >/dev/null
[ -d "$MOUNT/$APP_NAME" ] || die "Không tìm thấy $APP_NAME trong $DMG."

# Close a running copy so the bundle can be replaced.
pkill -f "$APP_DIR/$APP_NAME/Contents/MacOS/" 2>/dev/null && sleep 1 || true

SUDO=""
if [ ! -w "$APP_DIR" ] || { [ -e "$APP_DIR/$APP_NAME" ] && [ ! -w "$APP_DIR/$APP_NAME" ]; }; then
  say "Cần quyền quản trị để ghi vào $APP_DIR."
  SUDO="sudo"
fi
$SUDO rm -rf "$APP_DIR/$APP_NAME"
$SUDO ditto "$MOUNT/$APP_NAME" "$APP_DIR/$APP_NAME"
$SUDO xattr -cr "$APP_DIR/$APP_NAME"

say "Đã cài ctdozlite-next v${VERSION} vào $APP_DIR/$APP_NAME"
if [ -z "${CTDOZ_NO_OPEN:-}" ]; then
  open "$APP_DIR/$APP_NAME"
fi
