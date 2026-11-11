# Tính năng & giới hạn

Trang này cho biết **cái gì đang chạy, cái gì chạy một phần, cái gì chưa có** — để bạn không mất thời gian báo lại những lỗi đã biết.

> **Kiểm chứng trên nhánh `main` (v0.1.0).** Bảng này được cập nhật mỗi lần release.
> Nếu bạn đang dùng bản build cũ hơn, một số dòng có thể đã khác.

## Cách đọc

| Ký hiệu | Nghĩa |
|---|---|
| ✅ | Hoạt động bình thường |
| 🟡 | Hoạt động một phần — có giới hạn được ghi rõ |
| 🐞 | Đã biết lỗi — đang có issue theo dõi |
| ⛔ | Chưa hỗ trợ |
| 🚫 | Không nằm trong kế hoạch |

Cột **Issue** trỏ tới nơi đang theo dõi. Nếu issue đã đóng, tính năng đã được sửa.

---

## Đăng nhập & phiên làm việc

| Tính năng | Trạng thái | Ghi chú | Issue |
|---|---|---|---|
| Đăng nhập bằng mã QR | ✅ | Quét bằng app Zalo trên điện thoại | |
| Đăng nhập bằng số điện thoại + mật khẩu | ✅ | `--phone`, `--pass` | |
| Tự khôi phục phiên từ `session.json` | ✅ | Giữ nguyên phiên khi gặp lỗi mạng / HTTP 404 tạm thời; tự failover qua nhiều host ứng viên | [#21](https://github.com/ctdoteam1110/ctdozlite/issues/21), [#27](https://github.com/ctdoteam1110/ctdozlite/issues/27) |
| Đăng xuất | ✅ | Chỉ xoá phiên xác thực; lịch sử được cất vào `accounts/<uin>_history.db` và tự khôi phục khi đăng nhập lại | [#21](https://github.com/ctdoteam1110/ctdozlite/issues/21) |
| Nhiều tài khoản / thư mục dữ liệu riêng | ✅ | Hỗ trợ `--data-dir <PATH>` hoặc biến môi trường `CTDOZLITE_DATA_DIR` | [#30](https://github.com/ctdoteam1110/ctdozlite/issues/30) |

---

## Nhắn tin & Hẹn giờ

| Tính năng | Trạng thái | Ghi chú | Issue |
|---|---|---|---|
| Gửi/nhận tin nhắn văn bản (1-1 và nhóm) | ✅ | | |
| Trả lời trích dẫn | ✅ | Chuột phải hoặc **nhấp đúp** vào bong bóng tin nhắn; trong nhóm tự chèn `@Tên` người gửi (xoá được), `Esc` để thoát chế độ trả lời | |
| Chuyển tiếp tin nhắn | ✅ | | |
| Thả cảm xúc (reaction) | ✅ | Rê chuột vào cảm xúc để xem tên người thả; bấm để mở hộp thoại **Biểu cảm** theo từng loại | |
| Thu hồi tin nhắn | ✅ | Thu hồi với mọi người | |
| Xoá tin nhắn phía tôi | ✅ | Không ảnh hưởng người khác | |
| Ghim / đánh dấu sao | ✅ | Lưu trữ bền vững trong SQLite, sắp xếp mới nhất trước | [#9](https://github.com/ctdoteam1110/ctdozlite/issues/9) |
| Hẹn giờ gửi tin nhắn & Nhắc hẹn | ✅ | Bấm nút `🕒` cạnh ô nhập tin nhắn; hỗ trợ mốc nhanh (`+15m`, `+1h`, `18:00`, `08:00 mai`) và giờ tùy chỉnh | [#38](https://github.com/ctdoteam1110/ctdozlite/issues/38), [#24](https://github.com/ctdoteam1110/ctdozlite/issues/24) |
| Gửi ảnh và tệp đính kèm | ✅ | Kéo-thả tệp vào khung chat: thả vào vùng tin nhắn để **Gửi nhanh**, thả vào ô soạn tin để **Xem trước khi gửi** (kèm chú thích) | |
| Gửi sticker, gợi ý sticker khi gõ | ✅ | Có kho sticker + hỗ trợ nạp sticker tùy chỉnh từ `custom_stickers.json` | [#7](https://github.com/ctdoteam1110/ctdozlite/issues/7) |
| Dán ảnh trực tiếp (`Ctrl+V` / `⌘V`) | ✅ | Windows và macOS (ảnh chụp màn hình, ảnh PNG/TIFF, tệp ảnh copy từ Finder); Linux cần `wl-paste` hoặc `xclip` | |
| Tìm và gửi GIF | ✅ | Gõ `@gif` (GIF xu hướng) hoặc `@gif từ khoá` trong ô soạn tin; bật/tắt ở **Cài đặt → Tiện ích → Gợi ý @** | |
| Nhắc tên (@mention) trong nhóm | ✅ | Gõ `@` để chọn thành viên (↑/↓/Tab/Enter); tag hiển thị màu xanh và được gửi thành tag thật, kể cả khi trả lời trích dẫn | |
| Xem trước liên kết (OpenGraph) | ✅ | | |
| Tin nhắn thoại | ⛔ | Chưa ghi âm và gửi được tin nhắn thoại | |
| Chỉnh sửa tin nhắn đã gửi | ⛔ | | |

---

## Hội thoại & lịch sử

| Tính năng | Trạng thái | Ghi chú | Issue |
|---|---|---|---|
| Danh sách hội thoại (bạn bè, nhóm, Cloud của tôi) | ✅ | Tự động thử qua danh sách host dự phòng khi một domain trả 404/lỗi mạng | [#22](https://github.com/ctdoteam1110/ctdozlite/issues/22), [#27](https://github.com/ctdoteam1110/ctdozlite/issues/27) |
| Tên nhóm và ảnh đại diện nhóm | ✅ | Đồng bộ đầy đủ qua `getAllGroups` + `getGroupInfo` | [#8](https://github.com/ctdoteam1110/ctdozlite/issues/8) |
| Quản lý hội thoại (Ghim / Tắt thông báo / Ẩn / Xoá lịch sử) | ✅ | Chuột phải vào hội thoại ở danh sách bên trái | [#5](https://github.com/ctdoteam1110/ctdozlite/issues/5), [#24](https://github.com/ctdoteam1110/ctdozlite/issues/24) |
| Nhận tin nhắn mới theo thời gian thực | ✅ | Polling trong worker + WebSocket | [#28](https://github.com/ctdoteam1110/ctdozlite/issues/28) |
| Tìm kiếm hội thoại & nội dung tin nhắn | ✅ | Tìm không dấu theo tên hội thoại và nội dung tin nhắn trong `zalo_history.db` | [#5](https://github.com/ctdoteam1110/ctdozlite/issues/5) |
| Lưu lịch sử cục bộ (SQLite) | ✅ | `zalo_history.db` là DB duy nhất của tài khoản đang đăng nhập; giữ nguyên qua khởi động lại và đăng nhập lại | |
| Đồng bộ lịch sử cũ (History Sync) | ✅ | Đồng bộ phân trang có lưu checkpoint (`history_sync_cursors`) cho nhóm và Cloud của tôi, kèm giải mã backup lịch sử Zalo PC | [#19](https://github.com/ctdoteam1110/ctdozlite/issues/19), [#10](https://github.com/ctdoteam1110/ctdozlite/issues/10) |
| Nút "Làm mới hội thoại" | ✅ | Làm mới danh sách bạn bè, nhóm, tin nhắn mới và hiển thị rõ trạng thái / lỗi nếu có | [#23](https://github.com/ctdoteam1110/ctdozlite/issues/23) |
| Nhập (Migrate) lịch sử từ máy/thư mục khác | ✅ | **Cài đặt → Đồng bộ tin nhắn → Chọn file hoặc thư mục để nhập** (hỗ trợ `.db`/`.sqlite` và `.json`/`.jsonl`) | [#9](https://github.com/ctdoteam1110/ctdozlite/issues/9) |

---

## Tự động hoá, REST API & MCP

| Tính năng | Trạng thái | Ghi chú | Issue |
|---|---|---|---|
| Auto-Reply (Tự động trả lời theo từ khóa) | ✅ | Cấu hình trong **Cài đặt → Tiện ích (`autoreply_rules.json`)** | [#35](https://github.com/ctdoteam1110/ctdozlite/issues/35) |
| Local HTTP REST API (`127.0.0.1`) | 🚫 | **Đã tắt có chủ đích trong bản build cá nhân (bảo mật).** Trước đây chạy với `--api-port <PORT>` hoặc `CTDOZLITE_API_PORT` (`/api/v1/status`, `/threads`, `/messages`, `/send`, `/schedule`, `/autoreply`) | [#35](https://github.com/ctdoteam1110/ctdozlite/issues/35) |
| Model Context Protocol (MCP Server) | 🚫 | **Đã tắt có chủ đích trong bản build cá nhân (bảo mật).** Trước đây chạy `--mcp` (stdio JSON-RPC 2.0) hoặc gọi `POST /api/v1/mcp` để tích hợp Claude Desktop / Cursor / AI Agent | [#35](https://github.com/ctdoteam1110/ctdozlite/issues/35) |

---

## Bạn bè

| Tính năng | Trạng thái | Ghi chú | Issue |
|---|---|---|---|
| Tìm người dùng bằng số điện thoại | ✅ | | |
| Gửi lời mời kết bạn | ✅ | | |
| Chấp nhận / từ chối lời mời | ✅ | | |
| Huỷ lời mời đã gửi | ✅ | | |

---

## Gọi thoại & video

| Tính năng | Trạng thái | Ghi chú | Issue |
|---|---|---|---|
| Gọi thoại 1-1 | 🟡 | Đã có luồng gọi; chất lượng phụ thuộc mạng | |
| Gọi video | 🟡 | Cần `ffmpeg` trong `PATH` để thu và giải mã hình | |
| Gọi nhóm | 🟡 | | |

---

## Giao diện & nền tảng

| Tính năng | Trạng thái | Ghi chú | Issue |
|---|---|---|---|
| Giao diện đồ họa (GUI) | ✅ | `--gui`, mặc định | |
| Giao diện terminal (TUI) | ✅ | `--tui` | |
| Thông báo tin nhắn mới (macOS) | ✅ | Dùng thông báo hệ thống (Notification Center); không thông báo tin do chính mình gửi | |
| Chủ đề sáng / tối & Theme JSON tùy chỉnh | ✅ | Hỗ trợ ghi đè bảng màu qua `theme.json` (**Cài đặt → Giao diện**) | [#7](https://github.com/ctdoteam1110/ctdozlite/issues/7) |
| Hiển thị tiếng Việt và tiếng Trung (CJK) | ✅ | Tự nạp font hệ thống đa ngôn ngữ + fallback Noto Sans CJK | [#6](https://github.com/ctdoteam1110/ctdozlite/issues/6), [#5](https://github.com/ctdoteam1110/ctdozlite/issues/5) |
| Tự động kiểm tra & cập nhật phiên bản mới | 🚫 | **Đã tắt có chủ đích trong bản build cá nhân (không kết nối GitHub API).** Cập nhật thủ công bằng cách build lại | [#12](https://github.com/ctdoteam1110/ctdozlite/issues/12), [#29](https://github.com/ctdoteam1110/ctdozlite/issues/29) |
| iOS (Sắp có) | 🟡 | Hoạt động dưới dạng phiên phụ (`zpw_type=30`), sẽ phát hành gói `.ipa` trong bản cập nhật tới | |

---

## Báo một lỗi mới

Trước khi mở issue, kiểm tra [trang Đã biết](DA_BIET.md). Nếu chưa có, mở issue kèm:

1. Hệ điều hành và phiên bản (Windows 11, macOS 15, Fedora 41…).
2. Phiên bản app (góc trên bên trái, `Native v…`).
3. Chế độ đang chạy: GUI hay TUI.
4. Bước tái hiện, càng ngắn càng tốt.
5. Log: chạy lại với `--dev` và dán phần lỗi.

Xem [Hướng dẫn sử dụng](HUONG_DAN_SU_DUNG.md) để biết chi tiết các tham số.

