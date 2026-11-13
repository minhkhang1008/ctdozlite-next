# Công cụ gọi video

Đã triển khai trong mã nguồn, chưa phát hành/cài đè app đang dùng.

FFmpeg phục vụ thu camera, mã hóa H.264, giải mã hình cuộc gọi và phát video đính
kèm ngay trong hội thoại. Luồng hiện tại
không gọi `ffprobe` hoặc `ffplay`, nên không cần cài thêm hai chương trình đó.

Nếu bấm **Gọi Video** hoặc **Bật Camera** mà còn thiếu FFmpeg, app mở Cài đặt chung
và bắt đầu chuẩn bị công cụ. Có tiến độ, nút dừng và thông báo lỗi để thử lại.
Cũng có thể vào **Cài đặt → Cài đặt chung → Công cụ gọi video** và bấm cài trước.
Cài xong có thể gọi lại hoặc bật camera, không cần khởi động lại app.

- **macOS Apple Silicon/Intel:** tải FFmpeg 9.0.2 vào `tools/ffmpeg` dưới thư mục
  dữ liệu của app. Không cần Homebrew hoặc quyền quản trị. App kiểm tra SHA-256
  trước khi cho chạy, thử khởi động rồi mới kích hoạt; không thay FFmpeg hệ thống.
- **Windows:** dùng WinGet, gói `Gyan.FFmpeg` từ nguồn `winget`. Cần WinGet có sẵn.
- **Linux:** dùng `pkexec /usr/bin/apt-get install -y ffmpeg`; hỗ trợ môi trường
  Debian/Ubuntu có apt-get và PolicyKit. Hệ điều hành có thể yêu cầu xác thực quyền
  cài đặt. Các trình quản lý gói Linux khác chưa được tích hợp.

App dò cả đường dẫn Homebrew chuẩn khi khởi chạy từ Finder, không chỉ `PATH` của
Terminal. Việc quét lại và mọi tiến trình/tải file đều chạy nền. Phần giao diện chỉ
đọc kết quả đã lưu trong bộ nhớ. Trình tải công cụ dùng HTTP client riêng, không
mang cookie/token Zalo; chỉ theo redirect HTTPS tới đúng nhà phát hành/mirror đã
cho phép. Hủy trình quản lý gói có thể để hệ điều hành tiếp tục hoàn tất thao tác
đang chạy; app không hứa tự hoàn tác các thay đổi của trình quản lý gói.

## Nguồn và xác minh

FFmpeg chỉ phát hành mã nguồn và dẫn tới các bên cung cấp binary ở
[trang tải chính thức](https://ffmpeg.org/download.html).

| Kiến trúc macOS | Nguồn | SHA-256 của file FFmpeg sau giải nén |
| --- | --- | --- |
| Apple Silicon | [OSXExperts](https://www.osxexperts.net/), `ffmpeg902arm.zip` | `6deb2860c3dc0c08e0f96bab119196c024103046fa3cc7ef72e3a77797d8553c` |
| Intel | [Evermeet](https://evermeet.cx/ffmpeg/), `ffmpeg-9.0.2.zip` | `a45b462cf91ed89148ae218c4577e30896485d7a6792c3673bcf5f823fa01b63` |

Hash Apple Silicon đã đối chiếu với hash công bố của nhà cung cấp; cả hai hash đã
được tính lại từ bản tải HTTPS trong lần triển khai này. Hash Intel là hash của
bản tải đã kiểm tra, không được mô tả là checksum công bố riêng của nhà cung cấp.
Nguồn ARM liên kết mã nguồn build; thông tin bản build/GPL nằm trên các trang
nhà cung cấp. App tải trực tiếp từ nhà cung cấp khi cần, không commit binary vào repo.

Giới hạn tải ZIP 128 MiB, file giải nén 192 MiB. Chỉ đọc entry `ffmpeg`, không dùng
tên đường dẫn trong ZIP để ghi file. Các file mới thuộc thư mục app có quyền riêng
trên Unix; file thực thi được đặt `0700`. Hash sai hoặc chạy thử thất bại không
được kích hoạt thành công cụ của app.

Đã kiểm tra bằng test offline: archive/hash/path, cài vào thư mục tạm, quyền file,
giữ bản cũ khi hash sai và bố cục các trạng thái ở hai theme/cửa sổ hẹp. Binary
Apple Silicon thật đã qua roundtrip mã hóa/giải mã một khung H.264 tổng hợp.
Chưa xác minh installer trên Windows/Linux, giao diện app đang cài hoặc cuộc gọi
video với thiết bị/người dùng thật. Cài FFmpeg không thay thế quyền camera/mic của
hệ điều hành và không bảo đảm mọi luồng VoIP Zalo đều đã hoạt động.
