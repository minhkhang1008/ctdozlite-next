# Cài đặt & phát hành

## Cài bản dựng sẵn (macOS)

**Homebrew:**

```bash
brew tap minhkhang1008/tap
brew trust minhkhang1008/tap     # Homebrew yêu cầu bạn xác nhận tin tap bên thứ ba
brew install --cask ctdozlite-next
```

Homebrew bắt buộc xác nhận tin cậy (`brew trust`) với mọi tap ngoài kho chính thức. Tap
[`minhkhang1008/homebrew-tap`](https://github.com/minhkhang1008/homebrew-tap) chỉ có một tệp cask: tải
`.dmg` từ GitHub Releases (Homebrew đối chiếu SHA-256), cài vào Applications và gỡ cờ cách ly cho app.

**Hoặc một lệnh, không bị Gatekeeper chặn** (script hỏi trước khi cài; muốn đọc trước thì tải về bằng
`curl -fsSLO <đường dẫn bên dưới>` rồi mở ra xem):

```bash
curl -fsSL https://raw.githubusercontent.com/minhkhang1008/ctdozlite-next/main/scripts/install-macos.sh | bash
```

Script (`scripts/install-macos.sh`) tải bản mới nhất đúng chip, đối chiếu SHA-256 với
`SHA256SUMS.txt`, chép `ctdozlite.app` vào `/Applications`, gỡ cờ cách ly rồi mở app. Gatekeeper chỉ
chặn tệp có cờ `com.apple.quarantine` mà trình duyệt gắn vào; `curl` không gắn cờ này. Tuỳ chọn:
`CTDOZ_VERSION=0.2.1` (cài bản cụ thể), `CTDOZ_APP_DIR=~/Applications`, `CTDOZ_YES=1` (không hỏi),
`CTDOZ_NO_OPEN=1`. Kiểm tra nguồn gốc tệp: `gh attestation verify <tệp> --repo minhkhang1008/ctdozlite-next`
(xem mục "An toàn & minh bạch" trong README).

Vì sao không ký "thật": muốn macOS mở app tải bằng trình duyệt mà không hỏi, app phải được ký
Developer ID **và** công chứng với Apple — cần Apple Developer Program (99 USD/năm). Từ macOS 15, mẹo
chuột phải → Open cũng không còn tác dụng với app chưa công chứng. Các bản cập nhật tải trong app
không mang cờ cách ly nên không bị chặn; chỉ lần cài đầu cần cách ở trên.

**Hoặc tải `.dmg` thủ công:**

Tải tại [Releases](https://github.com/minhkhang1008/ctdozlite-next/releases):

| Máy | Tệp |
|---|---|
| Apple Silicon (M1–M4) | `ctdozlite-<phiên bản>-macos-arm64.dmg` |
| Intel | `ctdozlite-<phiên bản>-macos-x86_64.dmg` |
| Chạy được trên cả hai | `ctdozlite-<phiên bản>-macos.dmg` |

Mở `.dmg`, kéo `ctdozlite.app` vào **Applications**. App chỉ được ký tạm (ad-hoc), chưa được Apple
công chứng, nên lần đầu mở macOS sẽ chặn. Vào **System Settings → Privacy & Security → Open Anyway**,
hoặc chạy:

```bash
xattr -dr com.apple.quarantine /Applications/ctdozlite.app
```

Muốn kiểm tra tệp tải về còn nguyên vẹn: tải kèm `SHA256SUMS.txt`, đặt cùng thư mục rồi chạy
`shasum -a 256 -c SHA256SUMS.txt --ignore-missing`.

Mỗi bản Release cũng có gói Windows (`.exe`) và Linux (`.deb`, `.rpm`, `.AppImage`, `.tar.gz`) do CI
dựng tự động, nhưng tôi chỉ kiểm thử thường xuyên trên macOS.

Sau khi cài, app tự kiểm tra bản mới khi mở và mỗi 30 phút. Khi có bản mới, bấm **Cập nhật ngay**:
app tải `.dmg`, kiểm tra SHA-256 và chữ ký, thay chính nó rồi tự mở lại. Nếu app không nằm ở thư mục
ghi được, app sẽ mở `.dmg` để bạn cài tay.

Gỡ cài đặt: xoá `ctdozlite.app`; muốn xoá cả dữ liệu thì xoá thêm
`~/Library/Application Support/ctdozlite`.

## Tự build từ mã nguồn

Cần: Rust 1.75+, Xcode Command Line Tools (`xcode-select --install`) và CMake (`brew install cmake`).

```bash
git clone https://github.com/minhkhang1008/ctdozlite-next.git && cd ctdozlite-next
export CMAKE_POLICY_VERSION_MINIMUM=3.5    # bắt buộc với CMake 4.x
cargo run --release -- --gui
```

Đóng gói `.dmg` trên máy (đúng kiến trúc của máy đang dùng; số phiên bản đọc từ `Cargo.toml`):

```bash
bash packaging/package-local.sh            # → dist/ctdozlite-<phiên bản>-macos-<arch>.dmg
```

`.dmg` này đã kèm helper thông báo DailyNotifier và icon. Trên Windows dùng
`pwsh -File packaging\package-local.ps1`.

## Phát hành phiên bản mới

CI (`.github/workflows/ci-release.yml`) chỉ chạy test (~2 phút) khi push vào `main`, và bỏ qua hoàn
toàn nếu commit chỉ sửa tài liệu. Khi bạn đẩy một tag `v*`, CI dựng gói cho macOS/Windows/Linux
(~10–25 phút) rồi tự tạo bản Release; app đang cài sẽ tự nhận bản mới.

1. Nâng `version` trong `Cargo.toml` (ví dụ `0.2.0`), chạy `cargo check` để cập nhật `Cargo.lock`.
2. Trong `CHANGELOG.md`, đổi `## [Unreleased]` thành `## [0.2.0] - <ngày>` và thêm một mục
   `## [Unreleased]` trống ở trên.
3. Commit, gắn tag rồi đẩy lên. Tag phải trùng phiên bản trong `Cargo.toml` và nằm trên `main`,
   nếu không CI sẽ dừng:

   ```bash
   git commit -am "chore(release): v0.2.0"
   git push origin main
   git tag -a v0.2.0 -m "ctdozlite-next v0.2.0"
   git push origin v0.2.0
   ```

4. Theo dõi tiến trình ở tab **Actions** (hoặc `gh run watch`). Khi xong, bản Release có đủ các gói
   và `SHA256SUMS.txt`.

Có thể chạy lại thủ công: **Actions → CI & Cross-Platform Release → Run workflow**, bật `publish`.
