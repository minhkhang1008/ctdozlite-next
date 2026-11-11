# Changelog

Mọi thay đổi đáng chú ý của `ctdozlite` được ghi nhận tại tệp này theo chuẩn [Keep a Changelog](https://keepachangelog.com/vi/1.1.0/) và [Semantic Versioning](https://semver.org/).
Xem thêm quy ước tại [ADR 0003 — Chính sách Changelog & Release Notes](docs/adr/0003-changelog-policy.md).

> **ctdozlite-next** bắt đầu từ phiên bản `0.2.0`. Các phiên bản từ `0.1.0` trở về trước là lịch sử của
> dự án gốc [ctdoteam1110/ctdozlite](https://github.com/ctdoteam1110/ctdozlite), giữ lại để tham khảo; các liên kết issue trong đó trỏ về repo gốc.

## [Unreleased]

### Added (Tính năng mới)
- **Cài trên macOS không bị Gatekeeper chặn, miễn phí:** `scripts/install-macos.sh` (một lệnh `curl … | bash`) tải bản mới nhất đúng chip, kiểm tra SHA-256, cài vào Applications và gỡ cờ cách ly; kèm mẫu Homebrew Cask (`packaging/homebrew/`) cho tap riêng.

## [0.2.1] - 2026-11-12

### Added (Tính năng mới)
- **Tự động cập nhật từ Releases của ctdozlite-next:** Kiểm tra khi mở app và mỗi 30 phút, báo qua Notification Center; trên macOS tải `.dmg`, xác minh SHA-256 và chữ ký, thay app tại chỗ rồi tự mở lại.
- **Thanh thao tác khi rê chuột vào tin nhắn:** Ba nút Trả lời, Chia sẻ và "…" (menu Copy / Ghim / Đánh dấu / Tuỳ chọn khác / Thu hồi / Xóa chỉ ở phía tôi) như Zalo PC.
- **Hộp thoại Chia sẻ mới:** Tìm kiếm không dấu, tab Gần đây / Nhóm trò chuyện / Bạn bè, lọc theo **Phân loại** (nhãn hội thoại của tài khoản Zalo, `re-notes/40`), chọn nhiều hội thoại, xem trước nội dung và gửi kèm lời nhắn.
- **Chọn nhiều tin nhắn:** Bấm để chọn từng tin; thanh "n Đã chọn" có Sao chép, Chia sẻ, Xóa (chỉ ở phía tôi) và Hủy (`Esc`).
- **Phân loại ở danh sách hội thoại:** Thẻ màu theo nhãn tài khoản Zalo trên từng hội thoại và bộ lọc **Phân loại ▾** (thay cho nhãn cục bộ cố định trước đây). Chỉ đọc; gán nhãn trên điện thoại hoặc Zalo PC.
- **Mã QR đăng nhập tự làm mới:** Đếm ngược theo `qr_timeout` của Zalo, tự lấy mã mới khi hết hạn hoặc khi quét/xác nhận lỗi (tối đa 3 lần liên tiếp), kèm hướng dẫn xử lý khi điện thoại báo "không có kết nối với máy chủ".
- **Xem chi tiết tin nhắn:** Người gửi, thời gian đầy đủ, loại tin, trạng thái gửi, số cảm xúc và mã tin nhắn.

### Changed (Thay đổi)
- **Đổi tên hiển thị thành ctdozlite-next:** Tiêu đề cửa sổ, thanh tiêu đề, Cài đặt, thông báo và `--version`. Tên tệp chạy, bundle id và thư mục dữ liệu giữ nguyên.
- **Giao diện tối dễ nhìn hơn:** Nền tối hơn, bong bóng tin nhắn màu xám để tách khỏi nền.
- **"Copy link" cho tin liên kết:** Menu tin nhắn ghi đúng loại nội dung được sao chép.
- **Mở app toàn màn hình:** Phóng to sau khung hình đầu (đặt ngay lúc tạo cửa sổ làm cửa sổ không viền biến mất trên macOS).
- **Bỏ đăng nhập bằng số điện thoại:** Màn hình đăng nhập chỉ còn mã QR (dòng lệnh vẫn có `--phone`/`--pass`).
- **Dọn repo:** Xoá `.mailmap` (chỉ dùng để gộp danh tính người đóng góp bản gốc trong `git log`).
- **Phát hành nhanh hơn:** CI chỉ chạy test khi push `main` (bỏ qua nếu chỉ sửa tài liệu); bộ cài chỉ dựng khi có tag `v*`. Ghi chú phát hành lấy từ CHANGELOG nên hộp thoại cập nhật hiện danh sách thay đổi.

### Fixed (Sửa lỗi)
- **Giao diện sáng:** Chữ, huy hiệu phiên bản và nút cửa sổ trên thanh tiêu đề xanh nay màu trắng (trước đây cùng màu nền nên không đọc được); khung "Tổng số tin nhắn / Mã định danh" không còn nền xanh.
- **Thanh soạn tin bị đẩy khuất khi đang trả lời:** Chiều cao khu vực dưới danh sách tin nhắn nay được đo thực tế thay vì ước lượng; phần xem trước tin được trả lời chỉ còn một dòng.

## [0.2.0] - 2026-11-12

> Bản phát hành đầu tiên của ctdozlite-next (tối ưu macOS). Các thay đổi giao thức có bằng chứng trong `re-notes/38`, `re-notes/39`.

### Security (Bảo mật)
- **Tắt tự động cập nhật và REST API/MCP (`src/updater.rs`, `src/api_mcp.rs`):** Không còn kết nối tới GitHub API hay mở cổng HTTP cục bộ.
- **Danh sách domain cho phép (`src/session.rs`):** Mọi request mang `zpw_sek`/cookie phiên (HTTP, WebSocket, đồng bộ lịch sử, upload multipart) đều kiểm tra `is_trusted_zalo_url` (`*.zalo.me`, `*.zaloapp.com`, `*.zadn.vn`, `*.zdn.vn`).
- **Quyền tệp & giới hạn bộ nhớ:** `session.json`, `zalo_history.db` và tệp tạm khi giải mã giữ quyền `0600` (thư mục `0700`); luồng giải nén đồng bộ lịch sử có trần dung lượng RAM.

### Added (Tính năng mới)
- **Tìm và gửi GIF bằng lệnh `@gif` (`src/chat.rs`, `src/gui/workspace.rs`):** `@gif` hiện GIF xu hướng, `@gif <từ khoá>` tìm GIF (Giphy qua máy chủ Zalo); chọn bằng phím mũi tên/chuột, `Enter` để gửi. Công tắc "Tìm kiếm GIF" trong Cài đặt nay có tác dụng thật. Giao thức: `re-notes/39_gif_search_send.md`.
- **Dán ảnh `⌘V` trên macOS (`src/gui/macos_clipboard.rs`):** Đọc ảnh trực tiếp từ `NSPasteboard` (PNG, TIFF, tệp ảnh copy trong Finder) thay cho `osascript`; bắt phím `⌘V` cả khi clipboard chỉ có ảnh.
- **Kéo-thả tệp hai vùng (`src/gui/workspace.rs`, `src/gui/macos_drag.rs`):** Khi kéo tệp vào, khung chat hiện vùng **Gửi nhanh** (gửi ngay) và **Xem trước khi gửi** (đính kèm, thêm chú thích); vùng đang trỏ chuột được tô sáng.
- **Nhấp đúp để trả lời (`src/gui/workspace.rs`):** Vào chế độ trả lời, trong nhóm tự chèn `@Tên` người gửi (xoá được); `Esc` để thoát. Tin trả lời trong nhóm gửi kèm `mentionInfo` để tag hoạt động thật.
- **Nhắc tên (@mention) trong nhóm:** Gửi tag thật qua `group/mention`; tag hiển thị màu xanh trong bong bóng và ngay trong ô soạn tin; điều hướng danh sách gợi ý bằng `↑`/`↓`/`Tab`/`Enter`. Giao thức: `re-notes/38_group_mentions.md`.
- **Xem ai đã thả cảm xúc (`src/gui/reactions_dialog.rs`):** Tooltip hiện tên người thả khi rê chuột; hộp thoại **Biểu cảm** kiểu Zalo PC với tab theo từng loại cảm xúc.
- **Thông báo tin nhắn mới bằng Notification Center của macOS (`src/native_notifications.rs`):** Thay popup tự vẽ; không thông báo tin do chính mình gửi.
- **Emoji màu từ font hệ thống (`src/emoji_raster.rs`)** và emoji cỡ lớn khi tin nhắn chỉ có emoji.

### Changed (Thay đổi)
- **Giao diện theo Zalo PC:** Bảng màu sáng/tối, thanh điều hướng, danh sách hội thoại, bong bóng tin nhắn và ô soạn tin theo kích thước/màu của chat.zalo.me.
- **Liên kết trong tin nhắn:** URL trần hiển thị màu xanh, gạch chân khi rê chuột và bấm được.

### Fixed (Sửa lỗi)
- **Mất toàn bộ tin nhắn cũ khi khởi động lại hoặc đăng nhập lại; "Đồng bộ tin nhắn" như không có tác dụng (`src/account_manager.rs`, `src/store.rs`):** Lịch sử từng bị tách giữa `zalo_history.db` (nơi ghi) và `accounts/<uin>_history.db` (nơi đọc và nơi đồng bộ điện thoại ghi vào). Nay tài khoản đang đăng nhập chỉ dùng `zalo_history.db`; đăng nhập lại sẽ khôi phục bản đã cất, dữ liệu bị tách từ bản cũ được gộp một lần (không trùng, không mất tin).
- **Không kéo giãn được cột danh sách hội thoại:** Hàng hội thoại rộng hơn vùng cuộn 2px khiến panel tự nới ra sau mỗi lần vẽ.
- **Phím ↑/↓/Tab/Enter của danh sách @ không hoạt động ở lần dùng đầu tiên trên macOS:** Sự kiện IME `Enabled` bị hiểu nhầm là đang gõ dấu.
- **Căn lề bong bóng tin nhắn đến không có ảnh đại diện.**

## [0.1.0] - 2026-11-09

### Added (Tính năng mới & Cập nhật)
- **Đồng bộ tin nhắn Sync v2 di động bằng Wasmtime (`src/zproto_host.rs`, `src/history_sync.rs`):** Nhúng trực tiếp engine WASM `libzproto_wasm` của Zalo PC thông qua runtime Wasmtime thuần Rust, loại bỏ hoàn toàn việc phụ thuộc vào Node.js bên ngoài khi thực hiện đồng bộ tin nhắn từ thiết bị di động (hỗ trợ bắt tay Curve25519/X25519, giải mã session key và streaming gói tin).
- **Giao thức ghim và bỏ ghim tin nhắn qua API máy chủ (`src/chat.rs`, `src/endpoints.rs`):** Tích hợp reverse-engineering các endpoint bảng tin Zalo (`friendboard-wpa` cho chat 1-1 và `groupboard-wpa` cho nhóm), cấu trúc hóa dữ liệu `MSG_TOPIC` với đầy đủ định danh tin nhắn (`global_msg_id`, `client_msg_id`).
- **Ghi nhận đóng góp từ cộng đồng:** Bổ sung các đóng góp tiêu biểu từ @gr219 (xử lý `file_done`, spawn helper ETXTBSY), @Leoaxo098 (khóa composer nhóm bị giới hạn), và @hoanglong149 (nghiên cứu WASM Sync v2) vào danh sách vinh danh và cập nhật quy mô codebase lên hơn 73.300+ dòng.

### Fixed (Sửa lỗi & Tinh chỉnh giao diện)
- **Sửa lỗi gửi lại thông báo và đếm lại tin nhắn chưa đọc khi mở lại ứng dụng ([#122](https://github.com/ctdoteam1110/ctdozlite/issues/122)):** Khi khởi động ứng dụng, nạp trực tiếp điểm đọc từ đúng cơ sở dữ liệu tài khoản (`accounts/<uin>_history.db`) thay vì file mặc định. Bổ sung bộ lọc so sánh `last_seen_id` khi đồng bộ nền để không bao giờ đếm lại các tin nhắn đã đọc thành tin chưa đọc.
- **Tối ưu hóa layout Responsive & ngăn vỡ khung chat trên màn hình nhỏ ([#123](https://github.com/ctdoteam1110/ctdozlite/issues/123)):** Khôi phục các điểm ngắt responsive (`is_compact_w`, `is_narrow_w`), tự động ẩn thanh thông tin drawer bên phải khi cửa sổ quá hẹp. Cho phép co giãn sidebar hội thoại linh hoạt theo tỷ lệ (tối đa 42% chiều rộng màn hình). Thêm cơ chế `.truncate()` ở tiêu đề chat để ngăn tràn chữ đè lên cụm nút gọi điện/video.
- **Ẩn các liên hệ chưa có tin nhắn trong danh sách chat chính:** Chỉ hiển thị những người dùng đã có phát sinh tin nhắn qua lại trong danh sách hội thoại bên trái (chuẩn hành vi Zalo gốc), ẩn các tài khoản danh bạ trống giúp sidebar gọn gàng hơn.
- **Sửa lỗi ký tự ô vuông (Tofu) và thiết kế lại popup thông báo:** Thay thế toàn bộ ký tự Unicode checkmark `✓` và dấu đóng `✕` không được font hỗ trợ bằng icon vector Lucide (`lucide::CHECK`, `lucide::X`). Mở rộng kích thước, padding và chiều cao thanh nhập liệu trong popup thông báo nổi góc màn hình.
- **Hợp nhất cảm xúc tin nhắn (Reaction):** Gộp các phản ứng cùng loại emoji thành một biểu tượng duy nhất kèm số lượng tổng, bổ sung hộp thoại chi tiết xem danh sách người thả cảm xúc.

## [0.0.9] - 2026-11-08

### Added (Tính năng mới & Cập nhật)
- **Họp nhóm Zavi trực tiếp (`src/call.rs`, `src/gui/workspace.rs`):** Tích hợp nút gọi nhóm "Họp Zavi" trên header thanh công cụ nhóm, hỗ trợ báo hiệu và kết nối luồng hội nghị đa điểm Zavi.
- **Popup thông báo nổi góc màn hình (Corner Notification) & Âm thanh báo (`src/gui/modals.rs`, `src/media.rs`):** Hiển thị thẻ thông báo tin nhắn mới ngoài desktop kèm nút mở chat nhanh, trả lời trực tiếp từ thông báo và phát âm thanh chuông báo tin nhắn (`play_notification_chime`).
- **Nút cuộn nhanh xuống tin nhắn mới nhất (`src/gui/workspace.rs`):** Tự động hiển thị nút mũi tên trôi ở góc dưới bên phải khung chat khi cuộn lên xem tin nhắn cũ, bấm để nhảy tức thì về tin nhắn mới nhất và triệt tiêu hiện tượng giật hình (flicker).
- **Hỗ trợ ghim tin nhắn cá nhân và đánh dấu sao cục bộ:** Bổ sung trường `is_pinned`, `is_starred` trong bong bóng tin nhắn và thanh thông báo tin ghim ở đầu cuộc trò chuyện.

### Fixed (Sửa lỗi)
- **Sửa lỗi đơ cuộc gọi video (Video call freeze):** Tinh chỉnh tiến trình bắt tay ZRTP và bộ đệm RTP/RTCP của luồng webcam/video, tự động đóng màn hình gọi khi đối phương đã nghe máy ở thiết bị khác.
- **Bảo mật & Tránh lộ tên file tạm:** Sửa lỗi đặt tên ảnh dán từ clipboard (`clip_preview_*`), ngăn ngừa rủi ro SSRF và loại trừ tệp lưu khóa riêng tư `sync_v2_keys.json` khỏi kho Git.
- **Tập tin đính kèm bất đồng bộ ([#85](https://github.com/ctdoteam1110/ctdozlite/issues/85)):** Đợi tín hiệu `file_done` trước khi hoàn tất gửi tệp đính kèm lớn, khắc phục lỗi HTTP 405 trên nút kẹp giấy.

## [0.0.8] - 2026-11-04

### Added (Tính năng mới & Cập nhật)
- **Hoàn thiện đồng bộ dữ liệu di động Android (`src/history_sync.rs`):** Hỗ trợ giải nén và giải mã toàn diện các tệp container `.db.crypt` (RSA 2048 + AES-256-CBC + LZMA2) từ bản sao lưu Zalo Android.
- **Nghiên cứu & Bắt tay Sync v2 iOS (Curve25519 / X25519 Double-Ratchet):** Triển khai giao thức bắt tay trao đổi khóa phiên cho việc đồng bộ từ thiết bị Apple iOS.
- **Quản lý đa tài khoản (Multi-Profile) (`src/account_manager.rs`):** Cho phép lưu trữ và chuyển đổi nhanh giữa nhiều tài khoản Zalo trên cùng một máy, lưu cơ sở dữ liệu tách biệt theo định danh (`accounts/<uin>_history.db`).
- **Xem Nhật ký mạng xã hội Zalo (`src/feed.rs`, `src/bin/view_feed.rs`):** Hỗ trợ nạp và đọc bảng tin nhật ký bạn bè từ cache SQLite cục bộ.

### Fixed (Sửa lỗi)
- **Khóa ô nhập liệu khi bị chặn chat trong nhóm ([#97](https://github.com/ctdoteam1110/ctdozlite/issues/97)):** Nhận diện cờ `posting_restricted` từ cấu hình nhóm Zalo và vô hiệu hóa thanh soạn thảo đối với thành viên không có quyền nhắn tin.
- **Tối ưu hóa khởi động với lịch sử chat lớn ([#44](https://github.com/ctdoteam1110/ctdozlite/issues/44)):** Tăng tốc mở ứng dụng bằng cơ chế nạp lịch sử phân trang `PRELOAD_MSGS` thay vì nạp toàn bộ tin nhắn vào RAM.

## [0.0.7] - 2026-11-03

### Fixed (Sửa lỗi & Tinh chỉnh giao diện)
- **Sửa lỗi gõ tiếng Việt Telex trên macOS bị nuốt dấu cách & lỗi phím Enter ([#71](https://github.com/ctdoteam1110/ctdozlite/issues/71)):** Bảo toàn ký tự khoảng trắng (`" "`) trong sự kiện `Event::Ime(ImeEvent::Commit(" "))` của macOS NSTextInputClient/egui trước đây bị lọc nhầm bởi `.is_empty()`, đồng thời gắn cờ `ime_Preedit_active` để phím `Enter` chỉ xác nhận từ đang gõ dấu thay vì gửi tin nhắn sớm.
- **Hiển thị trực tiếp ảnh trong hội thoại & Trình xem ảnh tích hợp ([#72](https://github.com/ctdoteam1110/ctdozlite/issues/72)):** Phóng to khung hiển thị ảnh trực tiếp trong bong bóng chat lên tối đa `340x320`, tự động ẩn chuỗi URL CDN thô (`https://...zdn.vn/...`) khi đã có ảnh đính kèm, và chuyển toàn bộ thao tác bấm vào ảnh hoặc liên kết ảnh sang mở `ImageViewerModal` ngay trong ứng dụng (hỗ trợ phóng to/thu nhỏ, sao chép ảnh vào clipboard và lưu về máy).
- **Tự động kích hoạt đồng bộ tin nhắn từ điện thoại ngay khi đăng nhập ([#73](https://github.com/ctdoteam1110/ctdozlite/issues/73)):** Tự động khởi chạy đồng bộ lịch sử đám mây kèm gửi gói yêu cầu WebSocket `cmd 12000` & `cmd 12888` tới điện thoại ngay sau khi đăng nhập QR/Mật khẩu hoặc khôi phục phiên mà không cần bấm nút thủ công.
- **Tải tệp đính kèm trực tiếp & giữ nguyên tên file gốc của người gửi ([#74](https://github.com/ctdoteam1110/ctdozlite/issues/74)):** Sửa thứ tự trích xuất tên tệp trong `parse_chat_message` (`src/chat.rs`) ưu tiên trường `title` trước `href` để không bao giờ lấy nhầm đường dẫn endpoint `renewMedia.doc` của Zalo CDN, đồng thời tải trực tiếp tệp về thư mục `Downloads` với đúng tên gốc của người gửi thay vì mở trình duyệt ngoài.

## [0.0.6] - 2026-11-02

### Added (Tính năng mới & Cập nhật)
- **Trang Danh bạ chuẩn Zalo PC (`src/gui/modals.rs`, `src/gui/workspace.rs`):** Hợp nhất hai nút Cá nhân và Nhóm chat trên thanh điều hướng trái thành mục **Danh bạ** duy nhất với 4 phân mục chuẩn Zalo Desktop (*Danh sách bạn bè*, *Danh sách nhóm và cộng đồng*, *Lời mời kết bạn*, *Lời mời vào nhóm và cộng đồng*), hỗ trợ gom nhóm theo chữ cái đầu (`A–Z`), tìm kiếm và lọc nhanh.
- **Khôi phục gói nhãn dán động "Mèo Ami Bụng Bự (GIF)" (`src/gui/stickers.rs`, `src/assets/ami_fat_cat/`):** Tích hợp bộ 15 sticker GIF động trong bảng chọn nhãn dán và hỗ trợ gửi/hiển thị trực tiếp dưới dạng ảnh động mượt mà.
- **Hiển thị văn bản định dạng giàu (Markdown & Zalo RTF `textProperties`) (`src/chat.rs`, `src/gui/messages.rs`):** Giải mã định dạng `styles` trong `params.textProperties` của Zalo PC/Mobile và hỗ trợ hiển thị trực tiếp **in đậm**, *in nghiêng*, ~~gạch ngang~~, `inline code`, khối mã (`code block`), trích dẫn (`blockquote`), tiêu đề và danh sách ngay trong bong bóng tin nhắn.
- **Đồng bộ trạng thái tắt thông báo (Mute) hai chiều ([#67](https://github.com/ctdoteam1110/ctdozlite/issues/67)), Đồng bộ tin nhắn từ điện thoại ([#68](https://github.com/ctdoteam1110/ctdozlite/issues/68)) & Hỗ trợ Đa tài khoản / Proxy ([#64](https://github.com/ctdoteam1110/ctdozlite/issues/64)).**

### Fixed (Sửa lỗi & Tinh chỉnh giao diện)
- **Cải tiến khung nhập tin nhắn đa dòng (`src/gui/workspace.rs`):** Tự động mở rộng chiều cao từ 1 lên tối đa 5 dòng (`34px` → `114px`) khi nhấn `Shift + Enter`, từ dòng thứ 6 trở đi tự cuộn dọc bên trong và khóa cứng chiều ngang để không bao giờ đẩy lệch cụm nút `🕒` và `Gửi`.
- **Thiết kế lại hộp thoại Hẹn giờ gửi tin nhắn & Nhắc hẹn (`src/gui/modals.rs`):** Chuyển sang thẻ nổi bo góc hiện đại có màn mờ nền (`backdrop`), lưới 4 nút chọn nhanh cân đối và băng thông báo vàng hổ phách độ tương phản cao.

## [0.0.5] - 2026-11-02

### Fixed (Sửa lỗi)
- **Sửa triệt để lỗi gộp nhầm/nhảy sai hội thoại khi tên Zalo trùng nhau ([#63](https://github.com/ctdoteam1110/ctdozlite/issues/63)):** Tách biệt hoàn toàn định danh theo `thread_id` thực tế trong `threads_represent_same_contact` (`src/chat.rs`), khôi phục lựa chọn hội thoại theo `t.id == prev.id` (`src/gui.rs`), giới hạn ánh xạ tên trong SQLite `hydrate_threads` (`src/store.rs`) chỉ cho `"Cloud của tôi"`, và ưu tiên `partner_id` khi tra cứu avatar cuộc gọi.
- **Sửa lỗi khởi tạo & kiểm tra gói cài đặt macOS native helpers ([#58](https://github.com/ctdoteam1110/ctdozlite/issues/58), [#20](https://github.com/ctdoteam1110/ctdozlite/issues/20)):** Xác thực kiến trúc `arm64`/`x86_64`/Universal và chữ ký ad-hoc cho `ctdozlite.app` trước khi đóng gói `.dmg`.
- **Mạng & Failover đa máy chủ tự động ([#27](https://github.com/ctdoteam1110/ctdozlite/issues/27)):** Tự động chuyển tiếp qua danh sách máy chủ dự phòng cho từng dịch vụ (`friend`, `group`, `group_poll`, `chat`, `msg`, `conversation`, `profile`) khi gặp lỗi kết nối hoặc HTTP 404/5xx.
- **Làm mới hội thoại minh bạch ([#23](https://github.com/ctdoteam1110/ctdozlite/issues/23)):** Đổi nhãn nút "Đã đồng bộ đám mây" trong hồ sơ thành "Làm mới hội thoại" và hiển thị chính xác số hội thoại/tin nhắn mới hoặc thông báo lỗi mạng thực tế.

### Added (Tính năng mới & Cập nhật)
- **Tự động kiểm tra, thông báo & cập nhật trực tiếp qua GitHub Releases API:** Tự động truy vấn `https://api.github.com/repos/ctdoteam1110/ctdozlite/releases/latest` khi khởi động và định kỳ mỗi 30 phút, gửi thông báo hệ điều hành (macOS Notification Center / Linux `notify-send` / Windows PowerShell toast), hiển thị thanh thông báo nổi và hộp thoại chi tiết danh sách sửa lỗi/cập nhật kèm nút **Cập nhật tự động ngay** (xác thực `SHA256SUMS.txt` + chữ ký số).
- **Hẹn giờ gửi tin nhắn & Nhắc hẹn ([#38](https://github.com/ctdoteam1110/ctdozlite/issues/38), [#24](https://github.com/ctdoteam1110/ctdozlite/issues/24)):** Bổ sung bảng `scheduled_messages` trong SQLite, bộ điều phối nền kiểm tra mỗi 2 giây trong worker, nút `🕒` cạnh ô nhập tin nhắn và hộp thoại quản lý tin nhắn hẹn giờ (`+15m`, `+1h`, `18:00 hôm nay`, `08:00 ngày mai` hoặc `YYYY-MM-DD HH:MM`).
- **REST API, MCP Server & Auto-Reply ([#35](https://github.com/ctdoteam1110/ctdozlite/issues/35)):**
  - Máy chủ MCP chuẩn JSON-RPC 2.0 qua `--mcp` (stdio) và `POST /api/v1/mcp` (`list_threads`, `get_messages`, `search_messages`, `send_message`, `schedule_message`, `list_scheduled`, `cancel_scheduled`, `list_autoreply_rules`).
  - Máy chủ HTTP REST API cục bộ (`127.0.0.1`) qua `--api-port <PORT>` hoặc `CTDOZLITE_API_PORT`.
  - Bộ quy tắc tự động trả lời (`autoreply_rules.json`) quản lý trực tiếp trong **Cài đặt → Tiện ích**.
- **Nhập (Migrate) lịch sử tin nhắn từ thư mục/máy khác ([#9](https://github.com/ctdoteam1110/ctdozlite/issues/9)):** Hỗ trợ gộp dữ liệu lịch sử từ file `.db`/`.sqlite`, `.json`/`.jsonl` hoặc toàn bộ thư mục vào `zalo_history.db` một cách idempotent.
- **Tùy biến Theme & Sticker Pack bằng JSON ([#7](https://github.com/ctdoteam1110/ctdozlite/issues/7)):** Hỗ trợ xuất/nạp bảng màu `theme.json` và bộ nhãn dán `custom_stickers.json` ngay trong **Cài đặt → Giao diện**.
- **Menu chuột phải quản lý hội thoại ([#5](https://github.com/ctdoteam1110/ctdozlite/issues/5), [#24](https://github.com/ctdoteam1110/ctdozlite/issues/24)):** Hỗ trợ Ghim/Bỏ ghim, Tắt/Bật thông báo, Ẩn/Hiện hội thoại, Hẹn giờ gửi tin nhắn và Xoá lịch sử cuộc trò chuyện.
- **Đóng gói cục bộ ([#15](https://github.com/ctdoteam1110/ctdozlite/issues/15)), Phân loại Release Notes ([#14](https://github.com/ctdoteam1110/ctdozlite/issues/14)) & Tối ưu CI ([#17](https://github.com/ctdoteam1110/ctdozlite/issues/17), [#13](https://github.com/ctdoteam1110/ctdozlite/issues/13), [#12](https://github.com/ctdoteam1110/ctdozlite/issues/12), [#10](https://github.com/ctdoteam1110/ctdozlite/issues/10)):** Thêm `packaging/package-local.sh`, `packaging/package-local.ps1`, `.github/release.yml`, manifest `latest.json` kèm SHA-256 và bộ test giao thức giải mã (`#19`, `#16`).

## [0.0.3] - 2026-04-19

### Added
- Đồng bộ lịch sử phân trang có lưu trạng thái (`src/history_sync.rs`) và giải mã gói sao lưu lịch sử Zalo PC (`src/history_backup.rs`).
- Trình tự cập nhật an toàn có kiểm tra mã băm SHA-256 và chữ ký tệp thực thi (`src/updater.rs`).
- Hỗ trợ tùy chọn `--data-dir` và biến môi trường `CTDOZLITE_DATA_DIR` (`src/paths.rs`).
