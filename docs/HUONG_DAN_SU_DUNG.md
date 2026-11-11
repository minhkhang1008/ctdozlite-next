# Hướng dẫn sử dụng ctdozlite

Tài liệu tóm tắt các tùy chọn dòng lệnh, thao tác trên giao diện đồ họa (GUI) và bảng phím tắt của giao diện terminal (TUI).

---

## 1. Tham số dòng lệnh (CLI Flags)

| Tham số | Ý nghĩa | Ví dụ |
|---|---|---|
| `--data-dir <PATH>` | Chọn thư mục lưu phiên, cache, log và cơ sở dữ liệu; đường dẫn tương đối tính từ thư mục khởi chạy. | `zalo-rust --data-dir ./local-data --gui` |
| `--print-data-dir` | Chuẩn bị và in thư mục dữ liệu rồi thoát, không mở giao diện hay kết nối mạng. | `zalo-rust --print-data-dir` |
| `--gui` | Chạy chế độ giao diện đồ họa (mặc định nếu không truyền tham số). | `zalo-rust --gui` |
| `--tui` | Chạy chế độ giao diện dòng lệnh trong terminal. | `zalo-rust --tui` |
| `--qr` | Yêu cầu tạo mã QR đăng nhập ngay khi khởi động TUI. | `zalo-rust --tui --qr` |
| `--dev` | Bật thanh công cụ Dev Inspector để xem log mạng và payload giải mã. | `zalo-rust --gui --dev` |
| `--logout` | Xóa phiên đăng nhập hiện tại (`session.json`) rồi thoát. | `zalo-rust --logout` |
| `--phone <SỐ>` | Điền sẵn số điện thoại khi thử đăng nhập bằng mật khẩu. | `zalo-rust --tui --phone 0912345678` |
| `--pass <MK>` | Điền sẵn mật khẩu. | `zalo-rust --tui --phone 0912345678 --pass ...` |
| `--dump` | Lưu các gói phản hồi JSON ra thư mục `dump/` để phục vụ phân tích. | `zalo-rust --dump` |

---

Trên macOS, bản `.app` mặc định lưu dữ liệu trong `~/Library/Application Support/ctdozlite`, kể cả khi khởi chạy với thư mục làm việc `/`. Bản CLI ngoài bundle giữ thư mục làm việc hiện tại để tiếp tục dùng dữ liệu cũ. `--data-dir` ghi đè cả hai lựa chọn; thư mục chỉ định và thư mục mặc định của bundle có quyền `0700` trên Unix. Nếu thư mục không ghi được, chương trình báo lỗi trước khi mở dữ liệu.

`--version` không chuẩn bị thư mục dữ liệu. Chương trình không tự di chuyển, sao chép hay xóa dữ liệu cũ khi thay đổi thư mục; có thể dùng `--data-dir` để chọn lại thư mục đã có dữ liệu.

## 2. Giao diện đồ họa (GUI)

### 2.1. Đăng nhập
1. Chạy `./target/release/zalo-rust --gui`.
2. Mở ứng dụng Zalo trên điện thoại, quét mã QR hiển thị trên màn hình và bấm **Đăng nhập**.
3. Sau khi xác nhận, phiên làm việc được lưu vào `session.json`. Ở những lần mở tiếp theo, ứng dụng tự nạp lại phiên này mà không cần quét lại mã QR.

### 2.2. Nhắn tin & Gợi ý Sticker tự động
- **Tìm kiếm và lọc hội thoại:** Sử dụng ô tìm kiếm phía trên danh sách bên trái (hỗ trợ tìm tiếng Việt không dấu) hoặc lọc theo các tab *Tất cả*, *Cá nhân*, *Nhóm*.
- **Gợi ý nhãn dán khi đang gõ:**
  - Khi nhập từ khóa hoặc cụm từ ngắn trong ô chat (`xin chào`, `cảm ơn`, `ok`, `haha`, `buồn`, `khóc`, `nhớ`, `cố lên`, `mệt quá`, `đi chơi`, `trà sữa`...), ứng dụng tự động truy vấn API sticker của Zalo và hiển thị dải gợi ý phía trên khung soạn thảo.
  - Bấm trực tiếp vào sticker để gửi, hoặc dùng phím mũi tên `↑` để chọn lên thanh gợi ý, `←` / `→` để di chuyển, `Enter` để gửi và `Esc` để đóng.
  - Bấm biểu tượng kính lúp ở góc phải thanh gợi ý để mở toàn bộ kết quả tìm kiếm của từ khóa đó trong bảng Sticker Picker.
- **Kho nhãn dán (Sticker Picker):**
  - Bấm biểu tượng nhãn dán cạnh ô nhập tin nhắn để mở bảng chọn.
  - Tab **Tìm kiếm** tự động tìm nhãn dán ngay khi gõ từ khóa mà không cần bấm Enter.
  - Tab **Kho nhãn** cho phép duyệt và ghim các bộ nhãn dán từ máy chủ Zalo.
- **Tương tác trên tin nhắn:**
  - Nhấp chuột phải vào bong bóng tin nhắn để thả cảm xúc (reaction), trả lời trích dẫn, chuyển tiếp, sao chép nội dung, ghim, đánh dấu sao, thu hồi tin nhắn với mọi người hoặc xóa ở phía cá nhân.
  - **Nhấp đúp** vào một tin nhắn để trả lời nhanh. Trong nhóm, ô soạn tin tự chèn `@Tên` người gửi ở đầu (có thể xoá như tag thường). Nhấn `Esc` hoặc nút `✕` để thoát chế độ trả lời.
  - Rê chuột vào cảm xúc dưới tin nhắn để xem tên người đã thả; bấm vào để mở hộp thoại **Biểu cảm** (lọc theo từng loại cảm xúc).
- **Nhắc tên (@mention) trong nhóm:** gõ `@` để mở danh sách thành viên, dùng `↑`/`↓` để chọn, `Tab`/`Enter` để chèn, `Esc` để đóng. Tag hiển thị màu xanh và người được tag sẽ nhận thông báo.
- **Tìm và gửi GIF:** gõ `@gif` để xem GIF xu hướng, hoặc `@gif <từ khoá>` (ví dụ `@gif hello`) để tìm. Dùng phím mũi tên để chọn, `Enter` để gửi, `Esc` để đóng; bấm chuột vào GIF cũng gửi ngay. Bật/tắt ở **Cài đặt → Tiện ích → Gợi ý @ → Tìm kiếm GIF**.
- **Ảnh và tệp:**
  - Dán ảnh trực tiếp từ clipboard (`Ctrl+V` trên Windows/Linux, `⌘V` trên macOS): ảnh chụp màn hình, ảnh copy từ trình duyệt/Preview, hoặc tệp ảnh copy trong Finder. Ảnh hiện ở khung xem trước, nhập chú thích rồi nhấn `Enter` để gửi.
  - **Kéo-thả tệp** vào cửa sổ chat: khi kéo, khung chat hiện hai vùng — thả vào vùng tin nhắn (**Gửi nhanh**) để gửi ngay, hoặc thả vào ô soạn tin (**Xem trước khi gửi**) để đính kèm, xem lại, nhập chú thích rồi nhấn `Enter`. Vùng đang trỏ chuột được tô sáng.

### 2.3. Gọi thoại và gọi video 1-1
- Trong cuộc trò chuyện cá nhân, bấm biểu tượng điện thoại hoặc máy quay ở góc phải thanh tiêu đề hội thoại để bắt đầu cuộc gọi.
- Thiết bị thu âm (Mic) và máy ảnh (Camera) có thể chọn trực tiếp trong cửa sổ cuộc gọi hoặc tại mục **Cài đặt -> Cài đặt chung**.

---

## 3. Giao diện dòng lệnh (TUI)

Khởi chạy:
```bash
./target/release/zalo-rust --tui --qr
```

| Phím tắt | Chức năng |
|---|---|
| `↑` / `↓` hoặc `k` / `j` | Di chuyển giữa các cuộc trò chuyện trong danh sách bên trái |
| `Enter` | Gửi tin nhắn đang soạn hoặc chuyển con trỏ vào ô nhập |
| Gõ ký tự bất kỳ | Tự động đưa con trỏ vào ô soạn thảo |
| `Esc` | Rời ô soạn thảo, quay lại danh sách hội thoại |
| `Tab` | Chuyển đổi qua lại giữa danh sách hội thoại và ô nhập |
| `r` | Đồng bộ lại danh bạ và tin nhắn mới |
| `h` | Tải thêm lịch sử tin nhắn của nhóm đang chọn |
| `x` | Đăng xuất và xóa `session.json` |
| `l` | Mở bảng xem log mạng (khi chạy kèm cờ `--dev`) |
| `q` | Thoát ứng dụng |

---

## 4. Tiện ích kiểm thử dòng lệnh (`test_fetch`)

Binary phụ `test_fetch` dùng để kiểm tra nhanh kết nối API trên phiên đã lưu mà không cần mở giao diện:

```bash
# Kiểm tra phiên và in danh sách hội thoại
cargo run --release --bin test_fetch

# Gửi thử tin nhắn cá nhân
cargo run --release --bin test_fetch -- --send <USER_ID> "Nội dung tin nhắn"

# Gửi thử tin nhắn nhóm
cargo run --release --bin test_fetch -- --send-group <GROUP_ID> "Nội dung tin nhắn"
```
