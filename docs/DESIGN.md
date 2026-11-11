---
name: ctdozlite (ứng dụng desktop egui)
description: Ứng dụng Zalo native gọn nhẹ, tối ưu cho người dùng làm việc cả ngày; nền One Dark, bong bóng chat kiểu Zalo PC.
colors:
  dark-bg-app: "#181A1F"
  dark-bg-sidebar: "#21252B"
  dark-bg-chat-canvas: "#282C34"
  dark-bg-elevated: "#2C313A"
  dark-bg-hover: "#323842"
  dark-bg-selected: "#3A3F4B"
  dark-bg-input: "#1D1F23"
  dark-border: "#343942"
  dark-text-primary: "#D7DAE0"
  dark-text-secondary: "#ABB2BF"
  dark-text-muted: "#767E8E"
  dark-primary: "#528BFF"
  dark-primary-hover: "#61AFEF"
  dark-online: "#98C379"
  dark-danger: "#E06C75"
  dark-warning: "#E5C07B"
  dark-bubble-out-bg: "#25374D"
  dark-bubble-out-text: "#EBF0F6"
  dark-bubble-out-meta: "#9AABC0"
  dark-bubble-in-bg: "#323842"
  dark-bubble-in-border: "#3E4451"
  dark-bubble-in-text: "#D7DAE0"
  dark-bubble-in-meta: "#8E96A5"
  light-bg-app: "#FFFFFF"
  light-bg-sidebar: "#FFFFFF"
  light-bg-chat-canvas: "#EBF0F7"
  light-bg-elevated: "#FFFFFF"
  light-bg-hover: "#F1F5F9"
  light-bg-selected: "#E5F1FF"
  light-bg-input: "#F8FAFC"
  light-border: "#E2E8F0"
  light-text-primary: "#0F172A"
  light-text-secondary: "#475569"
  light-text-muted: "#64748B"
  light-primary: "#0068FF"
  light-primary-hover: "#2563EB"
  light-online: "#059669"
  light-danger: "#DC2626"
  light-warning: "#D97706"
  light-bubble-out-bg: "#E5EFFF"
  light-bubble-out-text: "#0F172A"
  light-bubble-out-meta: "#475C7A"
  light-bubble-in-bg: "#FFFFFF"
  light-bubble-in-border: "#D8E0EC"
  light-bubble-in-text: "#0F172A"
  light-bubble-in-meta: "#64748B"
typography:
  body:
    fontFamily: "Be Vietnam Pro"
    fontSize: "13.5px"
    fontWeight: 400
  caption:
    fontFamily: "Be Vietnam Pro"
    fontSize: "11.5px"
    fontWeight: 400
  label:
    fontFamily: "Be Vietnam Pro"
    fontSize: "12.5px"
    fontWeight: 600
rounded:
  xs: "6px"
  md: "8px"
  pill: "11px"
  panel: "12px"
  hover-bar: "16px"
spacing:
  item-gap: "8px 6px"
  button-padding: "10px 6px"
  bubble-pad-x: "12px"
  bubble-pad-y: "8.5px"
components:
  bubble-in:
    backgroundColor: "{colors.dark-bubble-in-bg}"
    textColor: "{colors.dark-bubble-in-text}"
    rounded: "{rounded.md}"
  bubble-out:
    backgroundColor: "{colors.dark-bubble-out-bg}"
    textColor: "{colors.dark-bubble-out-text}"
    rounded: "{rounded.md}"
  reaction-pill:
    backgroundColor: "{colors.dark-bg-elevated}"
    textColor: "{colors.dark-text-primary}"
    rounded: "{rounded.pill}"
    padding: "6px 2.5px"
---

# Design System: ctdozlite (ứng dụng desktop egui)

> Tài liệu này mô tả **app desktop** (`src/gui/`).
> Giá trị màu, bo góc, khoảng cách ở đây được đọc trực tiếp từ code (`src/gui/theme.rs`, `messages.rs`, `workspace.rs`, `main.rs`). Khi code đổi thì cập nhật file này trong cùng commit.
> **Frontmatter là nguồn chuẩn cho token.** Phần văn xuôi giải thích cách dùng, không lặp lại giá trị khác.

## Overview

**Creative North Star: "Bàn làm việc ban đêm"** *(tên gọi do người viết đề xuất, chỉnh lại nếu bạn có tên khác)*

ctdozlite là công cụ để mở cả ngày, không phải trang để ngắm. Giao diện tối, trầm, ít màu: mặt phẳng xám xanh kiểu One Dark làm nền, một màu xanh duy nhất để chỉ hành động, và bong bóng chat giữ dáng quen thuộc của Zalo PC để người dùng không phải học lại. Bản sáng dùng đúng màu xanh Zalo `#0068FF` trên nền trắng và canvas xanh xám nhạt như Zalo PC.

Mật độ vừa phải, thiên về đọc nhanh: chữ nhỏ (đa số 11.5–13.5px), bo góc nhỏ (6–8px), viền 1px mảnh thay vì bóng đổ nặng. Biểu cảm nằm ở chi tiết chính xác (avatar, pill cảm xúc, thanh hover), không nằm ở trang trí.

**Key Characteristics:**
- Hai chế độ sáng/tối dùng chung một cấu trúc token (`ThemePalette`); không có màu viết cứng ngoài palette, trừ vài màu trạng thái đã ghi trong tài liệu.
- Phân tầng bằng độ sáng nền (app → sidebar → canvas → elevated → hover → selected), không phân tầng bằng bóng đổ.
- Một màu nhấn (`primary`), dùng thưa.
- Mọi chuỗi hiển thị cho người dùng bằng tiếng Việt, font phải hiển thị đủ dấu.
- Cửa sổ không viền hệ điều hành (`with_decorations(false)`), nên app tự chịu trách nhiệm cho thanh tiêu đề và nút điều khiển.

## Colors

Bảng màu xám xanh lạnh, một nhấn xanh dương, màu trạng thái dịu. Nguồn: `ThemePalette::dark()` / `ThemePalette::light()`. Người dùng có thể ghi đè từng màu bằng `theme.json` (`#RRGGBB`), vì vậy **không bao giờ hard-code màu thay cho trường của palette**.

### Primary
- **Xanh hành động** (`dark-primary` / `light-primary`): nút chính, viền focus, avatar đang chọn, trạng thái "Đã xem" của tin gửi đi, viền chọn. Bản sáng là xanh Zalo.
- **Xanh hover** (`*-primary-hover`): trạng thái hover của phần tử dùng primary.

### Neutral
- **Nền app** (`*-bg-app`): thanh điều hướng trái và khung ngoài cùng.
- **Nền sidebar** (`*-bg-sidebar`): danh sách hội thoại, header.
- **Canvas chat** (`*-bg-chat-canvas`): vùng đọc tin nhắn. Bản sáng dùng xanh xám nhạt để bong bóng trắng nổi lên.
- **Nền nổi** (`*-bg-elevated`): modal, popover, pill, thanh hover, thẻ.
- **Hover / Selected** (`*-bg-hover`, `*-bg-selected`): hàng danh sách khi rê chuột và khi đang chọn.
- **Nền ô nhập** (`*-bg-input`): ô tìm kiếm, ô soạn tin; tối hơn hoặc nhạt hơn nền cha để thành "giếng".
- **Viền** (`*-border`): đường chia 1px.
- **Chữ** (`*-text-primary` / `secondary` / `muted`): ba bậc. Muted chỉ dành cho thông tin phụ (giờ, mô tả), phải vẫn đọc được (bản sáng đạt ~4.6:1).

### Trạng thái
- **Trực tuyến** (`*-online`): chấm online.
- **Nguy hiểm** (`*-danger`): xóa, thu hồi, lỗi gửi.
- **Cảnh báo** (`*-warning`): nhấn nhẹ cảnh báo. Màu "Đã lưu" (sao) dùng vàng `#EAB308` riêng trong code.

### Bong bóng chat
- **Tin gửi đi** (`*-bubble-out-*`): xanh navy đậm ở bản tối, xanh nhạt ở bản sáng. Chữ phụ (giờ, trạng thái) dùng `bubble-out-meta`.
- **Tin nhận** (`*-bubble-in-*`): trùng màu hover (tối) hoặc trắng có viền (sáng).
- **Màu avatar chữ cái**: tám màu cố định trong `avatar_color_for`, chọn theo băm tên để cùng một người luôn cùng màu.

### Named Rules
**The Palette-Only Rule.** Màu chỉ lấy từ `ThemePalette`. Cần màu mới thì thêm trường vào palette cho cả hai chế độ, không viết `Color32::from_rgb` tại chỗ.

**The One Voice Rule.** `primary` chỉ xuất hiện ở hành động và trạng thái đang chọn. Không dùng nó làm màu trang trí.

**The Both Modes Rule.** Mọi thay đổi UI phải đẹp ở cả bản tối và bản sáng. Kiểm tra cả hai trước khi báo xong.

## Typography

**Font chính:** Be Vietnam Pro (đóng gói sẵn, bảo đảm đủ dấu tiếng Việt)
**Font biểu tượng:** Lucide (đóng gói sẵn, dùng cho icon dạng glyph)
**Dự phòng:** font hệ thống cho CJK (YaHei, SimSun, Malgun, YuGothic, PingFang, Noto CJK), chỉ nạp khi cần.

**Character:** Sans hình học, gọn, đọc tốt ở cỡ nhỏ và với dấu tiếng Việt chồng. Không dùng font thứ hai cho chữ; icon luôn là Lucide hoặc ảnh emoji đóng gói.

### Hierarchy
Cỡ thực tế đang dùng nhiều nhất (đếm trong `messages.rs`/`workspace.rs`), từ nhỏ đến lớn:
- **Meta / giờ / nhãn nhỏ** (400, 11–11.5px): giờ tin nhắn, trạng thái gửi, số trên pill, nhãn "Đã ghim".
- **Phụ / nhãn** (400–600, 12–12.5px): tiêu đề thẻ, tab, tên người trích dẫn.
- **Nội dung** (400, 13.5px): chữ trong bong bóng và danh sách.
- **Tiêu đề nhỏ** (600, 13.8–15px): tên hội thoại, tiêu đề hộp thoại.

### Named Rules
**The Vietnamese-First Rule.** Chuỗi hiển thị viết bằng tiếng Việt có dấu. Không dùng tiếng Anh cho nhãn, toast, tooltip. Bình luận trong code vẫn bằng tiếng Anh (xem `AGENTS.md`).

**The Real-Icon Rule.** Icon cảm xúc dùng ảnh đóng gói trong `src/assets/emojis/` qua `emoji_to_tex_key`; chỉ rơi về ký tự emoji khi chưa có texture.

## Layout

Bố cục desktop ba cột: **thanh điều hướng trái** (rộng 62px, avatar tài khoản 40px ở trên, hành động ở dưới), **sidebar hội thoại** (rộng 300px cố định), **vùng chat** (phần còn lại: danh sách tin, rồi ô soạn tin).

- **Cửa sổ:** mặc định 1020×700, tối thiểu 480×360. Giao diện phải còn dùng được ở 480px; không được để nội dung bị cắt ở mép phải.
- **Nhịp khoảng cách:** khoảng cách giữa các phần tử 8×6px, đệm nút 10×6px (cấu hình trong `apply_to_ctx`).
- **Bong bóng:** rộng tối đa 72% vùng chat, kẹp trong 140–520px. Đệm trong 12px ngang, 8.5px dọc. Sticker và ảnh không khung dùng đệm 2px và rộng cố định (sticker 116px).
- **Nhóm tin liền nhau:** cùng người gửi thì cách 2.5px; đổi người cách 9px. Avatar 30px chỉ hiện ở tin đầu của cụm.
- **Cuộn:** thanh cuộn đặc rộng 6px.

### Named Rules
**The No-Clip Rule.** Một hàng phần tử (pill, nút, chip) không được rộng hơn bong bóng chứa nó. Gom lại thành một phần tử, hoặc cho xuống dòng. (Lỗi thật đã gặp: bốn pill cảm xúc riêng lẻ rộng hơn bubble sticker và bị cắt ở mép phải.)

**The Narrow-Pane Rule.** Mọi bố cục mới phải được xem ở chiều rộng nhỏ nhất, không chỉ ở 1020px.

## Elevation & Depth

Hệ thống **phẳng, phân tầng bằng tông nền**. Thứ tự từ sâu đến nổi: `bg-app` → `bg-sidebar` → `bg-chat-canvas` → `bg-elevated` → `bg-hover` → `bg-selected`. Viền 1px (`border`) tách các vùng. Bóng đổ chỉ dành cho phần tử **nổi thật sự** nằm trên nội dung.

### Shadow Vocabulary
- **Thanh cảm xúc nổi** (lệch 0×3, mờ 10, đen alpha 110 ở bản tối / 35 ở bản sáng): thanh 👍❤️😆… hiện khi rê chuột lên bong bóng.
- **Nút cuộn xuống** (lệch 0×2, mờ 8, đen alpha 80 / 35): nút tròn 36px ở góc dưới vùng chat.

### Named Rules
**The Flat-By-Default Rule.** Bề mặt phẳng khi nghỉ. Không thêm bóng cho thẻ, pill, hàng danh sách.

**The Floating-Owns-Hover Rule.** Phần tử nổi phía trên nội dung (thanh hover, popup) phải tự giữ trạng thái hover của nó (nhớ vùng của chính nó). Nếu không, bong bóng bên dưới sẽ cướp hover và phần tử biến mất đúng lúc người dùng chạm vào. (Lỗi thật đã gặp với thanh cảm xúc.)

## Shapes

Hình khối mềm nhưng nhỏ. Bo góc theo bậc, đếm từ code: **8px** (bong bóng, nút, ô nhập, mặc định của widget), **6px** (nút nhỏ, nút ảnh, hàng danh sách), **11px** (pill cảm xúc và chip), **12px** (khung panel/modal), **16px** (thanh hover). Avatar luôn tròn. Viền 1px, đổi sang `primary` khi hover/chọn.

### Named Rules
**The Radius-Ladder Rule.** Chỉ dùng các bậc bo góc ở trên. Không phát minh bậc mới (7, 9, 10…) cho một chỗ.

## Components

### Bong bóng tin nhắn
- **Hình:** bo 8px, đệm 12×8.5px.
- **Tin gửi đi:** căn phải, nền `bubble-out-bg`. Dòng cuối có giờ và trạng thái (Đang gửi / Đã nhận / Đã xem / Lỗi gửi) bằng chữ 11px.
- **Tin nhận:** căn trái, nền `bubble-in-bg` kèm viền ở bản sáng, avatar 30px ở đầu cụm.
- **Sticker / ảnh không khung:** nền trong suốt, đệm 2px.
- **Tương tác:** nhấp đúp để trả lời; chuột phải mở menu; rê chuột hiện thanh cảm xúc.

### Pill cảm xúc
- **Một pill duy nhất** dưới tin nhắn, giống Zalo: tối đa 3 icon (loại nhiều nhất trước, ảnh 16px) rồi tổng số (11.5px, đậm). Nền `bg-elevated`, viền 1px `border`, bo 11px, đệm 6×2.5px.
- **Hover:** con trỏ thành bàn tay; tooltip hiện từng loại bằng icon 18px kèm số.
- **Nhấp:** mở hộp thoại "Cảm xúc" (tab "Tất cả" và từng loại; mỗi dòng avatar, tên, icon). Phần số không rõ người thả hiện ở dòng "Chưa rõ người thả", không gán cho ai.
- **Dữ liệu:** số lấy từ sổ cái cảm xúc (`ReactionInbox`), không sửa tại chỗ. Không bao giờ cộng thêm 1 khi người dùng nhấn; chờ sự kiện thật từ Zalo.

### Thanh cảm xúc nổi
- Rộng 236px, cao 32px, bo 16px, đệm 8×4px, có bóng (xem Elevation).
- Sáu cảm xúc của Zalo (👍 ❤️ 😆 😮 😢 😠), icon 20px, theo sau là Trả lời và Chia sẻ. Đặt phía trên bong bóng nếu đủ chỗ, ngược lại ở phía dưới; luôn kẹp trong vùng nhìn thấy.

### Thanh điều hướng trái
- Rộng 62px, avatar tài khoản 40px (viền `primary-hover` 2px khi hover), chấm online có viền cắt khỏi avatar. Hành động phụ dồn xuống đáy.

### Sidebar hội thoại
- Rộng 300px. Hàng danh sách bo 6px; hover `bg-hover`; đang chọn `bg-selected`. Tên đậm hơn khi có tin chưa đọc.

### Avatar
- Tròn. Ảnh thật khi có; không có thì chữ cái đầu trên màu băm theo tên. Nhóm không có ảnh riêng dùng bố cục ghép `multi:`. Cỡ thường gặp: 40px (điều hướng), 34px (danh sách), 30px (trong chat).

### Hộp thoại và popover
- Nền `bg-elevated`, viền 1px, bo 12px. Dùng `egui::Window` neo giữa, không đổi kích thước. Đóng được bằng nút X của cửa sổ.

### Toast
- Thông báo ngắn, tiếng Việt, hiển thị qua `show_toast`; không dùng hộp thoại chặn cho thông tin không cần xác nhận.
- **Popup thông báo tin nhắn mới ở góc màn hình (`zalo_corner_notification_popup`):** Cửa sổ nổi không viền (`with_decorations(false)`, `with_always_on_top()`, `376×148`), nền `bg-elevated`, viền 1px `primary`, bo 12px. Hiển thị avatar tròn 42px (ảnh thật hoặc màu băm theo tên + chấm `accent-online`), tên người gửi (13.5px đậm), nội dung trích trước (12px), nút `Mở chat ↗` (bo 6px), pill số tin chưa đọc (bo 11px), và ô trả lời nhanh trực tiếp. Tự tạm dừng đếm ngược khi rê chuột hoặc đang nhập chữ.

## Do's and Don'ts

### Do:
- **Do** lấy mọi màu từ `ThemePalette` và kiểm tra cả bản sáng lẫn bản tối.
- **Do** gom nhiều chip/pill cùng loại thành một phần tử gọn nếu có nguy cơ rộng hơn bong bóng chứa chúng.
- **Do** xem giao diện ở 480px và ở 1020px trước khi báo xong.
- **Do** để phần tử nổi tự giữ hover của nó và kiểm tra bằng cách đưa chuột vào chính phần tử đó.
- **Do** dùng icon và emoji đóng gói (`src/assets/`), tiếng Việt cho mọi chuỗi hiển thị.
- **Do** để mọi việc nặng (mạng, đĩa, mã hóa) đi qua `WorkerCmd`; giao diện chỉ vẽ và gửi lệnh.
- **Do** hiển thị số liệu thật. Số không có nguồn thì ghi rõ "chưa rõ".

### Don't:
- **Don't** thêm bóng đổ cho thẻ, pill, hàng danh sách; hệ thống phẳng theo mặc định.
- **Don't** dùng `primary` để trang trí.
- **Don't** tạo bậc bo góc hoặc cỡ chữ mới cho một chỗ riêng lẻ.
- **Don't** tự cộng, đoán hay bịa số (cảm xúc, chưa đọc, thành viên). Chỉ hiển thị dữ liệu máy chủ đã xác nhận.
- **Don't** để nội dung bị cắt ở mép phải khi cửa sổ hẹp.
- **Don't** chặn luồng giao diện bằng I/O hoặc đợi mạng.
- **Don't** dùng font khác ngoài Be Vietnam Pro và Lucide cho giao diện.
