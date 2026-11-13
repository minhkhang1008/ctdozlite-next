# Đánh giá tích hợp upstream v0.1.1 — 2026-11-12

## Mốc so sánh

- Fork tại `cb2cd66b72879e3d9382b3c98ce7a1dd2a290399` (v0.2.1 và các commit riêng sau phát hành).
- Upstream `main` và tag `v0.1.1` cùng trỏ tới `7ed340813e398f5fa32f4b9a3f9c168fdac472c4`.
- Tổ tiên chung: `3d9e69074b14cb55c7e4c5e5d6d7728baa4c47af`.
- Upstream có 257 commit tổng cộng; fork thiếu 66 commit upstream, đồng thời có 17 commit riêng.
- Working tree còn các bản sửa đồng bộ/giao diện chưa commit từ tác vụ trước; được giữ nguyên.
- Chỉ fetch và đánh giá; chưa merge, cherry-pick, commit, build hay chạy test upstream.

## Kết luận

Có nhiều thay đổi đáng tích hợp, nhưng nên chuyển từng nhóm bản vá vào fork.
Mô phỏng `git merge-tree --write-tree --name-only HEAD FETCH_HEAD` trả exit 1
với 18 tệp xung đột. Mô phỏng chỉ dùng hai commit, không bao gồm các bản sửa
chưa commit. Không có kết luận rằng bản merge hiện đã chạy được.

## Nhóm nên lấy trước

| Thay đổi | Commit nguồn | Đánh giá |
| --- | --- | --- |
| Tránh panic `clamp` khi cửa sổ hẹp: link card, thanh reaction, tên người gọi | `5e6b98b` | Bản vá nhỏ, có giá trị trực tiếp. Trong fork vẫn còn hai biểu thức clamp thiếu chặn cận trên tại link card và call tile. Chuyển các phần tương ứng, không thay toàn bộ UI. |
| Cuộc gọi đến chỉ báo chuông, chờ bấm Nghe mới chiếm relay/probe UDP | `efe708d` | Có thể tránh app desktop cản điện thoại nhận cuộc gọi. Patch áp được nguyên trạng qua `git apply --check`; chưa kiểm thử cuộc gọi thật, cần rà luồng hủy/timeout/trả lời trước khi tích hợp. |
| Tìm WASM trong `Contents/Resources` của app macOS | `1203b97` | Hữu ích cho bản đóng gói; fork chưa có đường tìm này. Cần chuyển thủ công vì context khác. Việc tìm thấy đường dẫn không chứng minh gói phát hành đã chứa WASM. |
| Sửa định dạng mic, resample, meter | `1889923` và nhóm voice | Có giá trị nhưng phụ thuộc phần thu/gửi voice mới; không cherry-pick độc lập vào audio hiện tại. |

Kiểm tra áp patch trên working tree hiện tại (không ghi file):

| Commit | `git apply --check` |
| --- | --- |
| `efe708d` | exit 0 |
| `5e6b98b` | exit 1: context thanh reaction đã khác |
| `1203b97` | exit 1: context `zproto_host.rs` đã khác |
| `1889923` | exit 1: context `audio.rs` đã khác |
| `7579893`, `b5eeba2` | exit 1: context GUI và vendor chưa có |

Đây là kiểm tra khả năng áp patch, không phải kết quả compile/test.

## Tính năng mới đáng cân nhắc sau

- **Nháp từng hội thoại, nhắc lại tin, hộp Đã lưu, tìm kiếm nâng cao, lịch sử sửa tin, thư mục hội thoại** (`169d115`): thay đổi khoảng 2.500 dòng, liên quan schema SQLite, scheduler và UI. Nên tách tính năng, giữ idempotency và kiểm tra dữ liệu tài khoản.
- **Xem trước tài liệu Office/PDF/text trong app và voice AAC/M4A** (`2b200ed`, `c93137b` cùng chuỗi voice): có ích, nhưng thêm dependency và xử lý dữ liệu không tin cậy. Giới hạn tải 10 MiB chưa giới hạn dung lượng XML giải nén: `doc_preview::read_zip_entry` dùng `read_to_string` không có trần. Cần bổ sung trần giải nén trước khi lấy phần Office.
- **Markdown phong phú hơn** (`2961176`): code block, bảng, danh sách và tùy chọn render; phải giữ các sửa riêng về font/emoji, quote, reaction và bố cục.
- **IME Linux** (`7579893`, `b5eeba2`): có patch egui-winit nhỏ và tài liệu mô tả. Ưu tiên sau nếu bản fork phục vụ macOS; giữ nguyên bản vá clipboard/IME macOS hiện có.
- **Group Workspace**: ghi chú, thống kê, theo dõi từ khóa; phần LLM trong `group_llm.rs` mới là `StubProvider`, chưa có tóm tắt/hỏi đáp AI hoạt động.
- **Dịch tin nhắn** (`7338934`, `b208d1f`): gửi nội dung được chọn tới MyMemory qua HTTP GET; cần thể hiện rõ đích gửi dữ liệu khi đưa vào sản phẩm.
- **Rhai scripting** (`4b9d6ff`): có sandbox giới hạn thao tác, nhưng có thể xếp tin nhắn vào outbox để gửi. Đây là một tính năng tự động hóa riêng, không nên đi kèm đợt vá lỗi nhỏ.

## Sync V2: đáng nghiên cứu, chưa lấy nguyên chuỗi

Upstream có các thay đổi về entropy của WASM, định dạng sync ID base62, định
danh thiết bị, session cleanup và thứ tự các command. Tuy nhiên nhiều commit
trong chuỗi thử rồi đảo hướng giữa V1/V2 và thời điểm gửi 590/592; phải dựa vào
trạng thái cuối cùng và bằng chứng giao thức, không áp lần lượt theo tiêu đề.

Các điểm cần xử lý trước khi chuyển:

1. `history_sync::generate_sync_public_key` ghi private key bằng `std::fs::write`
   vào `sync_rsa_identity.json`, chưa ép `0o600` như quy định của fork.
2. Test `generate_sync_public_key_produces_valid_rsa2048_spki_der_base64` xóa
   `sync_rsa_identity.json` ngay trong working directory. Cần cô lập dữ liệu test
   trước khi chạy bản test này ở nơi có khóa thật.
3. Giữ kiểm tra URL tin cậy, trần tải 256 MiB và giải nén 512 MiB của fork khi
   chuyển mọi đường nhận batch mới; upstream hiện không có các helper/trần này.
4. Có logging sync luôn ghi ra file; cần rà nội dung và quyền truy cập trước
   khi đưa vào bản cá nhân.
5. Upstream vẫn dùng sentinel `9223372036854775808` trong `get_cloud_messages`
   và gọi old mode ngay từ đầu trong `sync_thread`. Vì vậy v0.1.1 chưa mang bản
   sửa recent-first vừa thực hiện trong working tree; không có bằng chứng rằng
   nâng upstream sẽ tự hết lỗi Nhóm & Cloud `-69`.

## Các quy tắc riêng phải giữ

- REST/MCP không được kích hoạt trở lại; upstream vẫn có đường bật server.
- Auto-updater chỉ lấy fork `minhkhang1008/ctdozlite-next` và bắt buộc kiểm tra
  SHA-256. Upstream dùng `GITHUB_REPO = "ctdoteam1110/ctdozlite"`; không thay
  nguồn cập nhật của fork bằng giá trị này.
- Giữ trust allowlist, quyền file/thư mục, giới hạn RAM và không tự tải URL tin
  đến để làm preview.
- Giữ các bản sửa macOS clipboard/IME/emoji, sidebar, notifications, chia sẻ và
  nhãn hội thoại. Không thay file GUI bằng bản upstream để giải conflict.
- Giữ mặc định sáng, lựa chọn theme được lưu và luồng sync chung đang sửa.
- Giữ số phiên bản riêng của fork; upstream v0.1.1 không phải hạ fork xuống v0.1.1.
- Social Feed đã bị bỏ khỏi fork theo yêu cầu trước đó; upstream thêm Feed mới
  không đồng nghĩa nên khôi phục.

## Thứ tự đề xuất

1. Chuyển các sửa clamp và tìm WASM; kiểm thử module GUI/zproto liên quan.
2. Chuyển passive ringing; kiểm tra cuộc gọi thực tế trên desktop và điện thoại.
3. Tách một đợt Sync V2 có test fixture, dữ liệu test riêng và xác minh điện thoại thật.
4. Chọn nháp/tìm kiếm/Đã lưu hoặc preview tài liệu cho đợt tính năng riêng.

Nguồn chính là Git objects đã fetch từ
<https://github.com/ctdoteam1110/ctdozlite> tại SHA nêu trên, không chỉ release notes.
