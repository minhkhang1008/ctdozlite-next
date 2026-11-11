# ADR 0002: Giao thức tự cập nhật và cấu trúc `latest.json` (Self-Updater Contract)

> **ctdozlite-next (2026-11-12):** Áp dụng lại, nguồn cập nhật là `minhkhang1008/ctdozlite-next`. Trên macOS, khi app chạy từ gói `.app` và tệp `.dmg` đã khớp SHA-256, trình cập nhật mount DMG, kiểm tra `codesign`, thay gói `.app` bằng cách đổi tên (có bản sao lưu để khôi phục) rồi mở lại app sau khi tiến trình cũ thoát; nếu không được thì mở DMG để cài tay.

- **Trạng thái:** Đã chấp nhận (Accepted)
- **Issues liên quan:** [#12](https://github.com/ctdoteam1110/ctdozlite/issues/12), [#29](https://github.com/ctdoteam1110/ctdozlite/issues/29)

## Bối cảnh

Ứng dụng phân phối nhiều định dạng gói cài đặt trên 3 hệ điều hành (`.deb`, `.rpm`, `.AppImage`, `.tar.gz`, `.exe` portable/setup, `.dmg` arm64/x86_64/universal). Trình tự cập nhật (`src/updater.rs`) cần một hợp đồng dữ liệu rõ ràng để xác thực toàn vẹn tệp tải về trước khi khởi chạy hoặc thay thế tệp thực thi.

## Quyết định

1. **Nguồn dữ liệu phát hành:**
   - CI xuất đồng thời `SHA256SUMS.txt` và `latest.json` trong mỗi GitHub Release.
   - `latest.json` tuân theo schema v1:
     ```json
     {
       "schema_version": 1,
       "version": "0.0.3",
       "tag_name": "v0.1.0",
       "published_at": "2026-04-19T00:00:00Z",
       "repository": "https://github.com/ctdoteam1110/ctdozlite",
       "release_url": "https://github.com/ctdoteam1110/ctdozlite/releases/tag/v0.1.0",
       "assets": [
         {
           "name": "ctdozlite-0.0.3-macos-arm64.dmg",
           "sha256": "<64-hex>",
           "size": 12345678,
           "browser_download_url": "https://github.com/ctdoteam1110/ctdozlite/releases/download/v0.1.0/ctdozlite-0.0.3-macos-arm64.dmg"
         }
       ]
     }
     ```
2. **Xác thực bảo mật (`src/updater.rs`):**
   - Kiểm tra tên tệp chống path traversal (`validate_asset_name`).
   - Giới hạn kích thước tải tối thiểu/tối đa và thời gian chờ (`DownloadLimits`).
   - Đối chiếu mã băm `SHA-256` và chữ ký định dạng tệp (PE `MZ`, Mach-O/DMG, ELF/AppImage) trước khi ghi đĩa.
   - Với bản chạy ngay (Portable), thực hiện ghi tạm và tự động hoàn tác (rollback) nếu tiến trình mới khởi chạy thất bại.

