#!/bin/bash
# =============================================================================================
#  Trình cài đặt ctdozlite-next cho macOS — mã nguồn mở, đọc được toàn bộ trước khi chạy.
#  Nguồn: https://github.com/minhkhang1008/ctdozlite-next/blob/main/scripts/install-macos.sh
#
#  Script này CHỈ làm đúng 5 việc, theo thứ tự, và hỏi bạn trước khi cài:
#    1. Tải tệp .dmg chính thức từ GitHub Releases của repo trên (đúng chip Apple Silicon/Intel).
#    2. Đối chiếu SHA-256 với SHA256SUMS.txt của cùng bản phát hành — sai là dừng ngay.
#    3. Kiểm tra chữ ký mã (codesign) của ctdozlite.app bên trong.
#    4. Chép ctdozlite.app vào /Applications (chỉ xin mật khẩu nếu thư mục cần quyền quản trị).
#    5. Gỡ cờ "tải từ Internet" (com.apple.quarantine) cho riêng app này rồi mở app.
#
#  Script KHÔNG: cài dịch vụ chạy nền, sửa cài đặt hệ thống, tắt Gatekeeper, gửi dữ liệu đi đâu,
#  hay tải gì khác ngoài 2 tệp ở github.com nói trên. Xoá app = kéo ctdozlite.app vào Thùng rác.
#
#  Vì sao cần bước 5: app chưa được Apple "công chứng" (cần tài khoản trả phí 99 USD/năm), nên
#  macOS chặn mọi app tải bằng trình duyệt chưa công chứng. Tải bằng Terminal + kiểm tra SHA-256 là
#  cách thay thế an toàn: bạn biết chính xác tệp được cài là tệp CI đã build và công bố.
#
#  Tuỳ chọn: CTDOZ_VERSION=0.2.1 (mặc định: bản mới nhất), CTDOZ_APP_DIR=~/Applications,
#            CTDOZ_YES=1 (không hỏi xác nhận), CTDOZ_NO_OPEN=1 (không tự mở app).
# =============================================================================================
set -euo pipefail

REPO="minhkhang1008/ctdozlite-next"
APP_DIR="${CTDOZ_APP_DIR:-/Applications}"
APP_NAME="ctdozlite.app"

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
note() { printf '    %s\n' "$*"; }
die() { printf '\033[1;31mLỗi:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || die "Script này chỉ dành cho macOS."

case "$(uname -m)" in
  arm64) SUFFIX="macos-arm64"; CHIP="Apple Silicon" ;;
  x86_64) SUFFIX="macos-x86_64"; CHIP="Intel" ;;
  *) SUFFIX="macos"; CHIP="Universal" ;;
esac

VERSION="${CTDOZ_VERSION:-}"
if [ -z "${VERSION}" ]; then
  # Đi theo đường dẫn chuyển hướng /releases/latest (không dùng API để tránh giới hạn lượt gọi).
  latest_url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${REPO}/releases/latest")"
  VERSION="${latest_url##*/v}"
fi
case "${VERSION}" in
  ''|*[!0-9.]*) die "Không xác định được phiên bản mới nhất (nhận được: '${VERSION}')." ;;
esac

DMG="ctdozlite-${VERSION}-${SUFFIX}.dmg"
BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
TMP="$(mktemp -d)"
MOUNT=""
cleanup() {
  if [ -n "${MOUNT}" ]; then hdiutil detach -quiet "${MOUNT}" 2>/dev/null || true; fi
  rm -rf "${TMP}"
}
trap cleanup EXIT

say "ctdozlite-next — trình cài đặt mã nguồn mở"
note "Phiên bản : v${VERSION} (${CHIP})"
note "Tải từ    : ${BASE}/${DMG}"
note "Cài vào   : ${APP_DIR}/${APP_NAME}"
echo

say "[1/5] Tải tệp cài đặt chính thức…"
curl -fL --progress-bar "${BASE}/${DMG}" -o "${TMP}/${DMG}"
curl -fsSL "${BASE}/SHA256SUMS.txt" -o "${TMP}/SHA256SUMS.txt"

say "[2/5] Đối chiếu SHA-256 với SHA256SUMS.txt của bản phát hành…"
expected="$(awk -v f="${DMG}" '{ n = $2; sub(/^\*/, "", n); sub(/^\.\//, "", n); if (n == f) print $1 }' "${TMP}/SHA256SUMS.txt")"
actual="$(shasum -a 256 "${TMP}/${DMG}" | awk '{ print $1 }')"
[ -n "${expected}" ] || die "SHA256SUMS.txt không có dòng cho ${DMG}."
[ "${expected}" = "${actual}" ] || die "SHA-256 không khớp — đã huỷ, không cài gì (tệp hỏng hoặc bị thay đổi)."
note "Khớp: ${actual}"

say "[3/5] Kiểm tra chữ ký mã của ứng dụng…"
MOUNT="${TMP}/mnt"
mkdir -p "${MOUNT}"
hdiutil attach -nobrowse -noautoopen -readonly -mountpoint "${MOUNT}" "${TMP}/${DMG}" >/dev/null
[ -d "${MOUNT}/${APP_NAME}" ] || die "Không tìm thấy ${APP_NAME} trong ${DMG}."
codesign --verify --deep --strict "${MOUNT}/${APP_NAME}" 2>/dev/null \
  || die "Chữ ký mã của ứng dụng không hợp lệ — đã huỷ, không cài gì."
note "Chữ ký hợp lệ (ký ad-hoc bởi GitHub Actions, chưa công chứng Apple)."

# Hỏi xác nhận (đọc từ bàn phím ngay cả khi script được chạy qua "curl … | bash").
if [ -z "${CTDOZ_YES:-}" ] && [ -r /dev/tty ]; then
  printf '\n    Cài ctdozlite-next v%s vào %s? [Y/n] ' "${VERSION}" "${APP_DIR}"
  read -r reply < /dev/tty || reply=""
  case "${reply}" in
    [nN]*) die "Đã huỷ theo yêu cầu, không thay đổi gì." ;;
  esac
fi

say "[4/5] Cài ctdozlite.app vào ${APP_DIR}…"
# Đóng bản đang chạy từ đúng vị trí cài (nếu có) để thay được; không đụng tới app nào khác.
pkill -f "${APP_DIR}/${APP_NAME}/Contents/MacOS/" 2>/dev/null && sleep 1 || true
SUDO=""
if [ ! -w "${APP_DIR}" ] || { [ -e "${APP_DIR}/${APP_NAME}" ] && [ ! -w "${APP_DIR}/${APP_NAME}" ]; }; then
  note "Thư mục ${APP_DIR} cần quyền quản trị — macOS sẽ hỏi mật khẩu máy của bạn."
  SUDO="sudo"
fi
${SUDO} rm -rf "${APP_DIR:?}/${APP_NAME}"
${SUDO} ditto "${MOUNT}/${APP_NAME}" "${APP_DIR}/${APP_NAME}"

say "[5/5] Gỡ cờ \"tải từ Internet\" cho riêng ${APP_NAME}…"
${SUDO} xattr -dr com.apple.quarantine "${APP_DIR}/${APP_NAME}" 2>/dev/null || true

echo
say "Xong! Đã cài ctdozlite-next v${VERSION}."
note "Các bản sau app tự cập nhật. Gỡ cài đặt: kéo ${APP_DIR}/${APP_NAME} vào Thùng rác"
note "(dữ liệu nằm ở ~/Library/Application Support/ctdozlite)."
if [ -z "${CTDOZ_NO_OPEN:-}" ]; then
  open "${APP_DIR}/${APP_NAME}"
fi
