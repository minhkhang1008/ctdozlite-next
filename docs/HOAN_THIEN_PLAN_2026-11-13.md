# Hoàn thiện các nhóm còn lại — 2026-11-13

Đây là trạng thái mã nguồn hiện tại, chưa phải bản đã phát hành hoặc cài đè app.
Các cải tiến được chuyển chọn lọc từ upstream v0.1.1, giữ các bản sửa và bảo mật của fork.

## Cách dùng

- **Đồng bộ điện thoại:** lần đầu chọn Android hoặc iPhone trong Cài đặt. Hai nút
  đồng bộ dùng chung thao tác. Android dùng luồng sao lưu native; iPhone dùng Sync V2.
  Khi thiếu runtime, app tự tải từ máy chủ tài nguyên công khai của Zalo, kiểm tra
  SHA-256 và khả năng chạy trước khi gửi yêu cầu điện thoại. Không cần cài Zalo PC.
  Lựa chọn điện thoại lưu theo thư mục dữ liệu/bản cài đặt, có thể đổi trong Cài đặt.
- **Bản nháp:** đổi chat để giữ từng bản nháp, quote và mention. Sidebar hiện Nháp.
  Đóng app chờ worker lưu xong; khi đăng xuất/chuyển tài khoản cũng lưu trước khi đổi
  dữ liệu. Bản nháp không gửi lên máy chủ.
- **Đã lưu:** dùng Đánh dấu sao trong menu tin nhắn. Mở Đã lưu ở thanh công cụ dưới
  cùng để thêm ghi chú, bỏ lưu hoặc đến đúng tin nhắn cũ. Đây là dữ liệu trên máy.
- **Nhắc hẹn riêng:** menu tin → Nhắc hẹn → chọn 15 phút/1 giờ/ngày mai. Quản lý ở
  thanh Nhắc hẹn. Đến hạn sẽ hiện thông báo và giữ mục vừa nhắc trong phiên hiện tại.
  App cần mở và đăng nhập; khi mở lại, các nhắc chưa xử lý đã đến hạn được nhận tiếp.
  Tính năng này không gửi tin vào hội thoại. Nút 🕒 hẹn gửi tin vẫn là thao tác riêng.
- **Tìm nâng cao:** chỉ tìm lịch sử đã có trong SQLite. Ví dụ:
  `"họp tuần" from:Minh has:file after:2026-11-01`.
  Hỗ trợ `from:`, `in:`, `has:image/file/voice/link`, `before:`, `after:`,
  `is:saved`/`is:starred`, `is:pinned`, cụm từ có dấu ngoặc kép và `#tag`.
  Giới hạn 200 kết quả mới nhất; thêm bộ lọc để thu hẹp. Bấm kết quả để tải ngữ cảnh
  cục bộ và nhảy đến tin, kể cả tin nằm ngoài phần lịch sử đang tải trên màn hình.
- **Tài liệu:** bấm Xem trước nội dung trên PDF/DOCX/XLSX/PPTX/TXT/CSV/MD/LOG/RTF.
  Chỉ trích xuất văn bản, không tái tạo trang/bảng tính như Office; tài liệu dài có
  giới hạn. PDF scan cần OCR bên ngoài; định dạng OLE cũ hoặc không hỗ trợ vẫn tải/mở
  bằng ứng dụng khác. Không có request tự động cho tệp/link trong tin nhận được.
- **Ghi âm:** chọn hội thoại → Ghi âm ở thanh dưới cùng. Nếu thiếu FFmpeg, app chuẩn
  bị công cụ; sau đó bấm Ghi âm để bắt đầu. Thu tối đa 5 phút, Dừng và gửi mới gửi;
  Hủy bỏ bản thu. Đích gửi cố định từ lúc bắt đầu, không đổi theo chat đang xem.
  Khi đổi tài khoản hoặc bắt đầu/nhận cuộc gọi, bản thu được hủy. Tin thoại nhận
  được được nhận diện riêng, giữ thời lượng; Nghe mở bằng ứng dụng mặc định.
- **Markdown:** khối mã có nút sao chép, bảng và danh sách đánh số. Cài đặt → Tin nhắn
  cho phép tắt định dạng để xem nguyên văn hoặc đổi kiểu `*một sao*`. Renderer
  mention/emoji, quote và font riêng của fork được giữ.

## Giới hạn và quyền riêng tư

Dữ liệu nháp/Đã lưu/nhắc hẹn dùng database của tài khoản đang hoạt động. Migration
thêm bảng/cột, không xóa tin nhắn; giữ được bản nháp dạng text, ghi chú và nhắc hẹn
nếu đã có schema upstream. Quote/mention đầy đủ được lưu cho bản nháp tạo bởi bản này.
Những cột tham chiếu quote cũ trong schema upstream được giữ, không tự dựng nội dung
quote khi thiếu dữ liệu gốc.

Tài liệu: tải tối đa 10 MiB; Office tối đa 2.000 mục, 8 MiB/mục và 32 MiB tổng
ngân sách giải nén thực tế, văn bản xem trước khoảng 20.000 ký tự, 50 hàng/sheet.
PDF chạy tiến trình con, timeout 12 giây; trên Unix đặt giới hạn CPU/data qua shell
hệ thống với đường dẫn truyền bằng positional argument. Giới hạn tài nguyên của
Windows chưa được kiểm chứng. Không ghi tài liệu nhận được vào cache đĩa.

Runtime Sync V2 tải không cookie/token, giới hạn 2 MiB, không nhận redirect, xác minh
hash trước khi ghi atomically vào `.sync-runtime` (file 0600, thư mục 0700). Module
không đi vào Git/gói phát hành. Override WASM chủ động của người dùng vẫn được giữ;
module tự tìm từ một bản Zalo khác chỉ dùng nếu trùng bản pin, nếu không tải bản đã
kiểm chứng vào cache riêng. Cache hỏng không làm thay đổi bản Zalo đã cài.

FFmpeg/macOS và âm thanh tạm tiếp tục dùng thư mục/tệp riêng tư; âm thanh được xóa
khi hoàn tất hoặc thất bại. Không tự retry gửi ghi âm vì có thể gây gửi trùng nếu
máy chủ đã nhận nhưng phản hồi mất. Kiểm tra hội thoại trước khi ghi lại/gửi lại.

Giữ allowlist Zalo, trần history 256/512 MiB, REST/MCP tắt, updater chỉ nguồn fork
và xác minh SHA-256. Không bật lại Feed, dịch tin qua dịch vụ bên ngoài hay scripting.

## Bằng chứng và phần cần kiểm tra thật

Các module liên quan đã có test lưu/đọc lại, isolation, migration schema cũ, SQL
operators/injection/literal wildcard, atomic Saved update, reminder claim một lần,
nhảy đến tin cũ với ngữ cảnh giới hạn, ZIP expansion, PDF giả lập, Markdown parser,
resample và AAC encode/decode. WASM thực được nạp/kiểm tra, FFmpeg ARM64 thực chạy
với âm thanh giả lập; đây không phải cuộc gọi hoặc tin nhắn thật.

Cần chạy bản mới với **đúng thư mục dữ liệu đang dùng**, rồi kiểm tra Android và
một iPhone thực sự nhận yêu cầu, nhập được tin cũ và không nhầm hội thoại. Kiểm tra
hủy/từ chối/thử lại, đổi tài khoản, quyền micro, hai thiết bị âm thanh, nghe bản ghi
ở điện thoại, cuộc gọi đến/đi và khởi động lại GUI. Không thay các kiểm tra này bằng
kết quả unit test. Agent chưa build release, phát hành hoặc thay app đang mở.
