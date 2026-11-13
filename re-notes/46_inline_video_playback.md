# 46 — Video đính kèm phát trong ứng dụng

Ngày kiểm tra source: 2026-11-13.

## Hiện trạng và bằng chứng

- Payload đã được parser nhận diện sẵn qua `chat.video`, `video` hoặc mã `4`, nhưng nhánh
  `fileName/fileSize` chạy trước và đổi `link_src` thành `Tập tin Zalo`. Fixture tổng hợp trong
  `chat::send_identity_tests::parse_chat_video_preserves_video_kind_and_extension` tái hiện lỗi
  này. Không dùng capture người dùng và không ghi cookie/URL ký thật vào repo.
- Tài liệu Zalo công khai xác nhận Zalo PC phân loại riêng **Ảnh/Video** và **Files**, có kho
  Ảnh/Video trong thông tin hội thoại. Tài liệu không công bố format payload, chiến lược buffer
  hay API player nội bộ:
  <https://help.zalo.me/huong-dan/chuyen-muc/nhan-tin-va-goi/nhan-tin/cac-tien-ich-co-ban-khi-nhan-tin-tren-zalo-may-tinh/>
- Tài liệu Media3 chính thức khuyến nghị chỉ chuẩn bị player khi UI ở foreground và giải phóng
  player để trả tài nguyên decoder hữu hạn. Đây là nguyên tắc vòng đời tham khảo, không phải bằng
  chứng Zalo dùng Media3:
  <https://developer.android.com/media/implement/playback-app>
- FFmpeg hỗ trợ HTTP seek/range, timeout và reconnect, nhưng ctdozlite không đưa URL tin nhắn
  thẳng cho tiến trình. Worker kiểm tra allowlist Zalo, tải theo chunk xuống file cache riêng tư,
  rồi decoder chỉ đọc file local:
  <https://ffmpeg.org/ffmpeg-protocols.html#http>

## Thiết kế đã chọn

1. Bubble chỉ tải poster nhỏ qua worker. Không tự phát và không tự tải video khi cuộn lịch sử.
2. Bấm phát mới tải video. File tạm/destination thuộc cache theo tài khoản, quyền `0700/0600`,
   tải nguyên tử, timeout 10 phút và giới hạn 512 MiB. Cache cũ bị prune theo mtime.
3. FFmpeg chạy ngoài UI thread. Một cờ `frame_pending` chỉ cho một frame RGBA 960×540 chờ UI
   (khoảng 2 MiB); frame sau vẫn được đọc nhưng không xếp thêm event nếu UI chưa nhận frame cũ.
4. Audio PCM dùng ring buffer tối đa hai giây. Bắt đầu video khác hoặc đóng modal đặt cờ dừng,
   hủy download hiện tại và kill/reap hai tiến trình decoder.
5. ID lưu trữ dài, không có đuôi và trông như token không còn được hiện như tên file; UI ghi
   `Video Zalo`. Tên thật có đuôi vẫn được giữ nguyên.

## Chưa xác minh

- Chưa có capture video thật đã khử nhạy cảm để kiểm tra mọi biến thể payload Zalo.
- Chưa kiểm tra trực quan/âm thanh với session Zalo thật, video dọc, video dài, codec HEVC hoặc
  URL đã hết hạn. Fixture parser và kiểm tra FFmpeg local không thay thế các bước này.
