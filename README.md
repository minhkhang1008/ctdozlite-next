# ctdozlite-next

Client Zalo desktop gọn nhẹ viết bằng Rust, là phiên bản tự phát triển từ
[ctdozlite](https://github.com/ctdoteam1110/ctdozlite) theo trải nghiệm và nhu cầu cá nhân,
tập trung vào **macOS**.

- Không Electron, không Chromium, không Node.js: một file thực thi duy nhất, ít RAM.
- Giao diện `egui` theo phong cách Zalo PC; lịch sử tin nhắn lưu cục bộ bằng SQLite.

> [!NOTE]
> Dự án cá nhân, phi thương mại, không liên quan tới VNG hay Zalo. Bản thân người dùng tự chịu trách nhiệm khi sử dụng.

## Khác gì so với bản gốc

**Riêng tư & bảo mật**
- Tắt REST API/MCP: app không mở cổng mạng cục bộ.
- Tự cập nhật từ Releases của repo này: kiểm tra SHA-256 rồi thay app tại chỗ (macOS) và mở lại.
- Token phiên (`zpw_sek`, cookie) chỉ được gửi tới domain của Zalo (`*.zalo.me`, `*.zaloapp.com`, `*.zadn.vn`, `*.zdn.vn`).
- `session.json` và cơ sở dữ liệu tin nhắn chỉ chủ máy đọc được (quyền `0600`).

**Trải nghiệm**
- Thông báo tin nhắn mới qua Notification Center của macOS.
- Dán ảnh bằng `⌘V`; kéo-thả tệp vào vùng **Gửi nhanh** hoặc **Xem trước khi gửi**.
- Nhấp đúp vào tin nhắn để trả lời, tự tag người gửi; `Esc` để thoát.
- Tag thành viên bằng `@`; tìm và gửi GIF bằng `@gif từ khoá`.
- Xem ai đã thả cảm xúc; emoji màu; giao diện sáng/tối theo Zalo PC.
- Lịch sử tin nhắn không còn mất khi tắt app hoặc đăng nhập lại.

Danh sách đầy đủ: [CHANGELOG](CHANGELOG.md).

## Cài đặt

**Tải bản dựng sẵn:** vào [Releases](https://github.com/minhkhang1008/ctdozlite-next/releases), tải
`ctdozlite-<phiên bản>-macos-arm64.dmg` (Apple Silicon) hoặc `-x86_64` (Intel), kéo app vào
Applications. App chưa được Apple công chứng, nên lần đầu mở hãy chuột phải → **Open**.

**Tự build:** cần Rust 1.75+, Xcode Command Line Tools và CMake.

```bash
git clone https://github.com/minhkhang1008/ctdozlite-next.git && cd ctdozlite-next
export CMAKE_POLICY_VERSION_MINIMUM=3.5      # nếu dùng CMake 4.x
cargo run --release -- --gui                  # chạy thử
bash packaging/package-local.sh               # đóng gói .dmg vào dist/
```

Chi tiết: [Cài đặt & phát hành](docs/CAI_DAT.md).

## Sử dụng nhanh

| Lệnh | Tác dụng |
|---|---|
| `ctdozlite --gui` | Mở giao diện đồ hoạ (mặc định) |
| `ctdozlite --tui --qr` | Giao diện terminal, đăng nhập bằng QR |
| `ctdozlite --data-dir <thư mục>` | Dùng thư mục dữ liệu riêng (mỗi tài khoản một thư mục) |
| `ctdozlite --dev` | Bật Dev Inspector để xem log mạng khi gỡ lỗi |

Dữ liệu (phiên đăng nhập, lịch sử, cài đặt) nằm ở `~/Library/Application Support/ctdozlite`
khi chạy bản `.app`, hoặc ở thư mục `--data-dir` nếu bạn chỉ định. Không chia sẻ `session.json`
cho bất kỳ ai.

## Tài liệu

- [Tính năng & giới hạn](docs/TINH_NANG.md)
- [Hướng dẫn sử dụng](docs/HUONG_DAN_SU_DUNG.md)
- [Cài đặt & phát hành](docs/CAI_DAT.md)
- [Xử lý sự cố](docs/KHAC_PHUC_SU_CO.md)
- Cho người phát triển: [AGENTS.md](AGENTS.md), [kiến trúc](docs/KIEN_TRUC_HE_THONG.md),
  [quy chuẩn](docs/standards/), ghi chú giao thức trong [`re-notes/`](re-notes/)

## Ghi công & giấy phép

Xây dựng trên mã nguồn [ctdozlite](https://github.com/ctdoteam1110/ctdozlite) của nhóm
ctdoteam1110 và các cộng tác viên. Phát hành theo giấy phép **MIT hoặc Apache-2.0**, tuỳ bạn chọn
([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).

Định dạng giao thức được tìm hiểu qua phân tích client Zalo công khai để viết lại độc lập bằng
Rust; repo không phân phối lại mã nguồn hay tài nguyên của Zalo.
