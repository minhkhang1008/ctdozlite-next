# Tính năng & giới hạn

✅ hoạt động · 🟡 hoạt động một phần · ⛔ chưa có · 🚫 tắt có chủ đích

Kiểm thử chủ yếu trên macOS. Cách dùng chi tiết: [Hướng dẫn sử dụng](HUONG_DAN_SU_DUNG.md).

## Đăng nhập & dữ liệu

| Tính năng | | Ghi chú |
|---|---|---|
| Đăng nhập bằng mã QR | ✅ | Quét bằng app Zalo trên điện thoại; mã tự làm mới trước khi hết hạn (đếm ngược), kèm hướng dẫn khi điện thoại báo không kết nối được máy chủ |
| Đăng nhập bằng số điện thoại + mật khẩu | 🚫 | Đã bỏ khỏi giao diện |
| Tự khôi phục phiên khi mở lại app | ✅ | Lỗi mạng tạm thời không làm mất phiên |
| Lịch sử tin nhắn cục bộ | ✅ | `zalo_history.db`; giữ nguyên khi tắt app, đăng xuất hay đăng nhập lại |
| Nhiều tài khoản | ✅ | Mỗi tài khoản một thư mục `--data-dir` |
| Đồng bộ lịch sử nhóm & Cloud của tôi từ máy chủ | ⛔ | |
| Đồng bộ tin 1-1 từ điện thoại | 🟡 | Hoạt động một phần, không thể sync toàn bộ dữ liệu |
| Nhập lịch sử từ tệp `.db` / `.json` | ✅ | **Cài đặt → Đồng bộ tin nhắn → Nhập lịch sử** |

## Nhắn tin

| Tính năng | | Ghi chú |
|---|---|---|
| Gửi/nhận tin nhắn 1-1 và nhóm | ✅ | Thời gian thực qua WebSocket |
| Trả lời trích dẫn | ✅ | Nhấp đúp vào tin; trong nhóm tự tag người gửi; `Esc` để thoát |
| Tag thành viên (`@`) | ✅ | Tag màu xanh, người được tag nhận thông báo |
| Thả cảm xúc & xem ai đã thả | ✅ | Rê chuột hoặc bấm vào cảm xúc dưới tin nhắn |
| Chuyển tiếp, thu hồi, xoá phía tôi, ghim, đánh dấu sao | ✅ | Rê chuột vào tin để hiện nút Trả lời / Chia sẻ / "…", hoặc chuột phải |
| Chia sẻ tới nhiều hội thoại | ✅ | Tìm kiếm, tab Gần đây / Nhóm / Bạn bè, lọc theo Phân loại, kèm lời nhắn |
| Chọn nhiều tin nhắn | ✅ | Menu "…" → Chọn nhiều tin nhắn: Sao chép, Chia sẻ, Xóa hàng loạt |
| Xem chi tiết tin nhắn | ✅ | Menu "…" → Xem chi tiết |
| Gửi ảnh & tệp | ✅ | Nút đính kèm, kéo-thả (Gửi nhanh / Xem trước), dán ảnh `⌘V` |
| Sticker & gợi ý sticker khi gõ | ✅ | |
| Tìm và gửi GIF | ✅ | `@gif` hoặc `@gif từ khoá` |
| Hẹn giờ gửi tin nhắn | ✅ | Nút `🕒` cạnh ô soạn tin |
| Nhắc lại tin nhắn riêng trên máy | 🟡 | Menu tin → Nhắc hẹn; quản lý tại thanh Nhắc hẹn, app cần mở và đăng nhập |
| Bản nháp từng hội thoại | ✅ | Giữ nội dung, quote, mention; lưu SQLite, đọc lại khi mở app |
| Đã lưu & ghi chú | ✅ | Đánh dấu sao; quản lý Đã lưu ở thanh công cụ dưới cùng |
| Tìm kiếm nâng cao cục bộ | ✅ | from:/in:/has:/before:/after:/is:, cụm từ, #tag; chỉ tìm dữ liệu đã lưu trên máy |
| Xem trước văn bản tài liệu | 🟡 | Bấm Xem trước trên PDF/Office/text; có giới hạn, không tự tải tệp tin nhận được |
| Markdown mở rộng | ✅ | Khối mã/copy, bảng, danh sách; tắt hoặc đổi kiểu một dấu sao tại Cài đặt → Tin nhắn |
| Xem trước liên kết | ✅ | Chỉ cho liên kết bạn gửi; không tự tải liên kết trong tin nhận được |
| Xem video trong app | ✅ | |
| Tin nhắn thoại AAC | ✅ |  |
| Sửa tin nhắn đã gửi | ⛔ | |

## Hội thoại & bạn bè

| Tính năng | | Ghi chú |
|---|---|---|
| Danh sách hội thoại (bạn bè, nhóm, Cloud của tôi) | ✅ | Kéo giãn được cột danh sách |
| Phân loại hội thoại | 🟡 | Hiện và lọc theo nhãn trong tài khoản Zalo (thẻ màu, nút **Phân loại ▾**); gán/sửa nhãn vẫn làm trên điện thoại hoặc Zalo PC |
| Ghim / tắt thông báo / ẩn / xoá lịch sử hội thoại | ✅ | Chuột phải vào hội thoại |
| Tìm hội thoại & nội dung tin nhắn | ✅ | Tìm được cả khi gõ không dấu |
| Tìm người bằng số điện thoại, kết bạn | ✅ | Đã test |

## Gọi thoại & video

| Tính năng | | Ghi chú |
|---|---|---|
| Gọi thoại 1-1 | ✅ | Đã test |
| Gọi video | ✅ | |
| Gọi nhóm | 🟡 | |

## Giao diện & hệ thống

| Tính năng | | Ghi chú |
|---|---|---|
| Giao diện đồ hoạ kiểu Zalo PC, sáng/tối | ✅ | Mở toàn màn hình; tuỳ chỉnh màu qua `theme.json` |
| Giao diện terminal (TUI) | ✅ | `--tui` |
| Thông báo tin nhắn mới (macOS) | ✅ | Notification Center |
| Emoji màu, hiển thị tiếng Việt/CJK | ✅ | Dùng font hệ thống |
| Tự động cập nhật | ✅ | Kiểm tra Releases của repo này khi mở app và mỗi 30 phút; tải, xác minh SHA-256, thay app tại chỗ rồi mở lại (macOS). **Cài đặt → Kiểm tra cập nhật** để kiểm tra thủ công |
| REST API cục bộ & MCP server | 🚫 | Tắt để không mở cổng mạng |
