# Xử lý sự cố

Khi gặp lỗi, chạy lại với `--dev` để xem log mạng trong Dev Inspector, mã lỗi thật thường nằm ở đó.

## Đăng nhập & phiên

**Bị đòi quét QR lại / báo lỗi `600` hoặc `-102`.** Phiên đã hết hạn hoặc bị thay bởi một lần đăng nhập
web khác (Zalo chỉ giữ một phiên web cho mỗi tài khoản). Đăng nhập lại bằng QR; lịch sử tin nhắn vẫn còn.

**Mở lại app phải đăng nhập lại dù chưa đăng xuất.** Kiểm tra bạn có đang mở app với một thư mục dữ
liệu khác lần trước không (bản `.app` dùng `~/Library/Application Support/ctdozlite`, còn `cargo run`
hoặc `--data-dir` dùng thư mục khác) và thư mục đó có quyền ghi không.

**Chạy hai tài khoản bị đăng xuất lẫn nhau.** Mỗi tài khoản cần một thư mục riêng:
`ctdozlite --data-dir ~/.ctdozlite-taikhoan2`.

**Không thấy tin nhắn cũ.** Lịch sử nằm trong thư mục dữ liệu đang dùng. Nếu trước đó bạn chạy với thư
mục khác, nhập lại bằng **Cài đặt → Đồng bộ tin nhắn → Nhập lịch sử** và chọn tệp `zalo_history.db`
cũ (việc nhập chỉ gộp thêm, không xoá gì).

## macOS

**"ctdozlite.app không thể mở được".** App chưa được Apple công chứng: chuột phải → **Open**, hoặc chạy
`xattr -dr com.apple.quarantine /Applications/ctdozlite.app`.

**Không có thông báo tin nhắn mới.** Vào **System Settings → Notifications** và cho phép ctdozlite (hoặc
DailyNotifier) gửi thông báo.

## Biên dịch & hiển thị

**CMake báo `Compatibility with CMake < 3.5 has been removed`** khi build `audiopus_sys`/`opus-sys`:
đặt `export CMAKE_POLICY_VERSION_MINIMUM=3.5` trước `cargo build`.

**Thiếu font tiếng Việt trên Linux tối giản:** cài `fonts-noto-core` hoặc `fonts-dejavu-core`.

**Gọi video không có hình:** cần `ffmpeg` trong `PATH` (`brew install ffmpeg`).

## Mã lỗi thường gặp từ máy chủ Zalo

| Mã | Ý nghĩa |
|---|---|
| `600` / `-102` | `zpw_sek` thiếu hoặc hết hạn → đăng nhập lại |
| `18012` | Đã quét QR nhưng chưa bấm xác nhận trên điện thoại |
| `18060` | Tham số chưa được mã hoá đúng (`ParamCipher v2`) |
| `18022` | Sai chữ ký `signkey` |
| `114` | Tham số không hợp lệ |
| `429` | Gọi API quá dày, chờ một lúc rồi thử lại |
