# Cài đặt & phát hành

## Cài bản dựng sẵn (macOS)

Tải tại [Releases](https://github.com/minhkhang1008/ctdozlite-next/releases):

| Máy | Tệp |
|---|---|
| Apple Silicon (M1–M4) | `ctdozlite-<phiên bản>-macos-arm64.dmg` |
| Intel | `ctdozlite-<phiên bản>-macos-x86_64.dmg` |
| Chạy được trên cả hai | `ctdozlite-<phiên bản>-macos.dmg` |

Mở `.dmg`, kéo `ctdozlite.app` vào **Applications**. App chỉ được ký tạm (ad-hoc), chưa được Apple
công chứng, nên lần đầu mở hãy chuột phải → **Open**, hoặc chạy:

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
