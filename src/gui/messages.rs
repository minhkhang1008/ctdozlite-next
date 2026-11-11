use super::*;

// ---------------------------------------------------------------------------
// Chat Bubble & Sticker Widget Rendering
// ---------------------------------------------------------------------------

pub(super) fn is_sticker_message(msg: &ChatMessage) -> Option<(u64, u64)> {
    if let Some(sid) = msg.sticker_id {
        if sid > 0 {
            return Some((sid, msg.sticker_cate_id.unwrap_or(0)));
        }
    }
    let check_ami = |s: &str| -> Option<(u64, u64)> {
        let pos = s.find("ami_fat_cat_")?;
        let num_str = s.get(pos + 12..pos + 14)?;
        let idx = num_str.parse::<u64>().ok()?;
        if (1..=15).contains(&idx) {
            Some((999000 + idx, AMI_FAT_CAT_CATE_ID))
        } else {
            None
        }
    };
    if let Some(hit) = check_ami(&msg.text)
        .or_else(|| msg.link_desc.as_deref().and_then(check_ami))
        .or_else(|| msg.link_title.as_deref().and_then(check_ami))
        .or_else(|| msg.link_url.as_deref().and_then(check_ami))
        .or_else(|| msg.link_thumb.as_deref().and_then(check_ami))
    {
        return Some(hit);
    }
    if msg.text.trim() == "[Nhãn dán]" {
        return Some((18009, 10130));
    }
    None
}

pub(super) fn render_sticker_widget(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    sticker_textures: &HashMap<String, TextureHandle>,
    loading_stickers: &mut HashSet<String>,
) -> bool {
    let Some((sid, cid)) = is_sticker_message(msg) else {
        return false;
    };

    let key = format!("stk_{cid}_{sid}");
    let url = if cid == 0 && (1..=30).contains(&sid) {
        format!("https://stc-chat.zdn.vn/images/stickers/default/thumb/{sid}.png")
    } else {
        format!("https://api.zaloapp.com/api/emoticon/sticker/webpc?eid={sid}&size=130")
    };

    if !sticker_textures.contains_key(&key) && loading_stickers.insert(key.clone()) {
        if let Some(tx) = WORKER_TX.get() {
            let _ = tx.send(WorkerCmd::FetchStickerImage {
                key: key.clone(),
                url,
            });
        }
    }

    if let Some(tex) = sticker_textures.get(&key) {
        ui.image((tex.id(), Vec2::new(116.0, 116.0)));
        true
    } else {
        false
    }
}

#[derive(Debug, Clone)]
pub(super) struct CallCardView {
    pub(super) title: String,
    pub(super) status_badge: String,
    pub(super) reason: Option<String>,
    pub(super) is_video: bool,
    pub(super) is_missed_or_failed: bool,
}

/// Detect whether `msg` is a voice/video call log entry and parse its structured metadata.
pub(super) fn extract_call_card_view(msg: &ChatMessage) -> Option<CallCardView> {
    let raw = msg.text.trim();
    let is_call_id = msg.id.starts_with("call_");
    let is_call_text = raw == "Cuộc gọi"
        || raw == "Cuộc gọi thoại"
        || raw == "Cuộc gọi video"
        || raw.starts_with("📞")
        || raw.starts_with("📹")
        || raw.starts_with("Cuộc gọi đi")
        || raw.starts_with("Cuộc gọi đến")
        || raw.starts_with("Cuộc gọi video");
    if !is_call_id && !is_call_text {
        return None;
    }

    let lower = raw.to_lowercase();
    let is_video = lower.contains("video") || raw.starts_with("📹");
    let is_incoming = lower.contains("gọi đến") || (!lower.contains("gọi đi") && !msg.is_outgoing);

    // Parse optional duration `(Ns)`
    let secs_opt: Option<u64> = raw.find('(').and_then(|open| {
        let rest = &raw[open + 1..];
        let close = rest.find("s)")?;
        rest[..close].trim().parse::<u64>().ok()
    });

    // Parse optional failure/rejection reason after `—` or `-`
    let raw_reason = raw
        .split_once(" — ")
        .or_else(|| raw.split_once(" - "))
        .map(|(_, r)| r.trim().to_string())
        .filter(|r| !r.is_empty());

    let is_missed_or_failed = match secs_opt {
        Some(0) => true,
        Some(_) => raw_reason.is_some(),
        None => raw_reason.is_some() || (!msg.is_outgoing && raw == "Cuộc gọi"),
    };

    let title = if is_video {
        if is_incoming {
            if is_missed_or_failed {
                "Cuộc gọi video nhỡ".to_string()
            } else {
                "Cuộc gọi video đến".to_string()
            }
        } else {
            "Cuộc gọi video đi".to_string()
        }
    } else if is_incoming {
        if is_missed_or_failed {
            "Cuộc gọi thoại nhỡ".to_string()
        } else {
            "Cuộc gọi thoại đến".to_string()
        }
    } else {
        "Cuộc gọi thoại đi".to_string()
    };

    let arrow = if is_incoming { "↙" } else { "↗" };
    let status_badge = match (is_missed_or_failed, secs_opt) {
        // The title already names the missed/failed state; show the reason instead of repeating it.
        (true, _) => {
            let reason_txt = raw_reason
                .as_deref()
                .filter(|r| !r.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    if is_incoming {
                        "Bạn đã bỏ lỡ cuộc gọi này".to_string()
                    } else {
                        "Người nhận không nghe máy".to_string()
                    }
                });
            format!("{arrow} {reason_txt}")
        }
        (false, Some(s)) if s >= 60 => {
            format!("{arrow} Thời lượng: {} phút {:02} giây", s / 60, s % 60)
        }
        (false, Some(s)) => format!("{arrow} Thời lượng: {s} giây"),
        (false, None) => format!("{arrow} Đã kết thúc"),
    };

    // When the call is missed/failed the badge already carries the reason, so only surface a
    // separate reason line for calls that otherwise look successful.
    let reason = if is_missed_or_failed {
        None
    } else {
        raw_reason.map(|r| {
            if r.contains("gián đoạn") || r.contains("từ chối") {
                "Người nhận từ chối hoặc kết nối gián đoạn".to_string()
            } else if r.contains("đang bận") {
                "Người nhận đang bận hoặc từ chối".to_string()
            } else {
                r
            }
        })
    };

    Some(CallCardView {
        title,
        status_badge,
        reason,
        is_video,
        is_missed_or_failed,
    })
}

pub(super) fn render_call_card_widget(
    ui: &mut egui::Ui,
    card: &CallCardView,
    pal: &ThemePalette,
    card_width: f32,
) -> Option<BubbleAction> {
    let mut action = None;
    let accent = if card.is_missed_or_failed {
        Color32::from_rgb(239, 68, 68)
    } else {
        Color32::from_rgb(34, 197, 94)
    };

    ui.set_width(card_width);
    let text_col_w = (card_width - 52.0).max(110.0);
    ui.vertical(|ui| {
        ui.set_width(card_width);
        ui.horizontal(|ui| {
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::hover());
            ui.painter().circle_filled(
                icon_rect.center(),
                20.0,
                accent.linear_multiply(if pal.is_dark { 0.18 } else { 0.14 }),
            );
            ui.painter().circle_stroke(
                icon_rect.center(),
                20.0,
                Stroke::new(1.2_f32, accent.linear_multiply(0.55)),
            );
            if card.is_video {
                draw_icon_video(ui.painter(), icon_rect.center(), 18.0, accent);
            } else {
                draw_icon_phone(ui.painter(), icon_rect.center(), 18.0, accent);
            }

            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.set_width(text_col_w);
                ui.add(
                    egui::Label::new(
                        RichText::new(&card.title)
                            .size(13.8)
                            .color(pal.text_primary)
                            .strong(),
                    )
                    .wrap(),
                );
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(&card.status_badge)
                            .size(11.8)
                            .color(accent)
                            .strong(),
                    )
                    .wrap(),
                );
                if let Some(reason) = &card.reason {
                    ui.add_space(3.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(reason).size(11.2).color(pal.text_secondary),
                        )
                        .wrap(),
                    );
                }
            });
        });
        ui.add_space(8.0);
        let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(card_width, 1.0), Sense::hover());
        ui.painter().line_segment(
            [sep_rect.left_center(), sep_rect.right_center()],
            Stroke::new(1.0_f32, pal.border),
        );
        ui.add_space(6.0);

        let btn_label = if card.is_video {
            format!("{}  Gọi video lại", lucide::VIDEO)
        } else {
            format!("{}  Gọi lại", lucide::PHONE)
        };
        let call_btn = egui::Button::new(
            RichText::new(btn_label)
                .size(12.5)
                .color(Color32::WHITE)
                .strong(),
        )
        .fill(pal.primary)
        .stroke(Stroke::new(1.0_f32, pal.primary_hover))
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(card_width, 30.0));

        if ui
            .add(call_btn)
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            action = Some(BubbleAction::CallAgain {
                is_video: card.is_video,
            });
        }
    });
    action
}

/// Open an `http://` or `https://` URL in the user's default system browser.
pub(super) fn open_external_url(url: &str) {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(
                hwnd: *mut std::ffi::c_void,
                lp_operation: *const u16,
                lp_file: *const u16,
                lp_parameters: *const u16,
                lp_directory: *const u16,
                n_show_cmd: i32,
            ) -> isize;
        }
        let op: Vec<u16> = "open\0".encode_utf16().collect();
        let mut file: Vec<u16> = trimmed.encode_utf16().collect();
        file.push(0);
        unsafe {
            let _ = ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                5, // SW_SHOW
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(trimmed).spawn();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(trimmed).spawn();
    }
}
/// Open a native OS file-picker dialog on a worker thread and return the selected path.
pub(super) fn pick_native_file_dialog(images_only: bool) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct OpenFileNameW {
            l_struct_size: u32,
            hwnd_owner: *mut std::ffi::c_void,
            h_instance: *mut std::ffi::c_void,
            lpstr_filter: *const u16,
            lpstr_custom_filter: *mut u16,
            n_max_cust_filter: u32,
            n_filter_index: u32,
            lpstr_file: *mut u16,
            n_max_file: u32,
            lpstr_file_title: *mut u16,
            n_max_file_title: u32,
            lpstr_initial_dir: *const u16,
            lpstr_title: *const u16,
            flags: u32,
            n_file_offset: u16,
            n_file_extension: u16,
            lpstr_def_ext: *const u16,
            l_cust_data: usize,
            lpfn_hook: *const std::ffi::c_void,
            lp_template_name: *const u16,
            pv_reserved: *mut std::ffi::c_void,
            dw_reserved: u32,
            flags_ex: u32,
        }
        #[link(name = "comdlg32")]
        extern "system" {
            fn GetOpenFileNameW(lpofn: *mut OpenFileNameW) -> i32;
        }
        let filter_raw = if images_only {
            "Image Files (*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp)\0*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp\0All Files (*.*)\0*.*\0\0"
        } else {
            "All Files (*.*)\0*.*\0Video & Media (*.mp4;*.mov;*.mkv;*.mp3;*.wav)\0*.mp4;*.mov;*.mkv;*.mp3;*.wav\0Documents (*.pdf;*.zip;*.docx;*.xlsx;*.txt)\0*.pdf;*.zip;*.docx;*.xlsx;*.txt\0\0"
        };
        let title_raw = if images_only {
            "Chọn hình ảnh để gửi qua Zalo\0"
        } else {
            "Chọn video hoặc tập tin đính kèm để gửi qua Zalo\0"
        };
        let filter_u16: Vec<u16> = filter_raw.encode_utf16().collect();
        let title_u16: Vec<u16> = title_raw.encode_utf16().collect();
        let mut file_buf = vec![0u16; 1024];
        // OFN_EXPLORER (0x00080000) | OFN_FILEMUSTEXIST (0x00001000) | OFN_PATHMUSTEXIST (0x00000800) | OFN_NOCHANGEDIR (0x00000008)
        let mut ofn = OpenFileNameW {
            l_struct_size: std::mem::size_of::<OpenFileNameW>() as u32,
            hwnd_owner: std::ptr::null_mut(),
            h_instance: std::ptr::null_mut(),
            lpstr_filter: filter_u16.as_ptr(),
            lpstr_custom_filter: std::ptr::null_mut(),
            n_max_cust_filter: 0,
            n_filter_index: 1,
            lpstr_file: file_buf.as_mut_ptr(),
            n_max_file: file_buf.len() as u32,
            lpstr_file_title: std::ptr::null_mut(),
            n_max_file_title: 0,
            lpstr_initial_dir: std::ptr::null(),
            lpstr_title: title_u16.as_ptr(),
            flags: 0x0008_0000 | 0x0000_1000 | 0x0000_0800 | 0x0000_0008,
            n_file_offset: 0,
            n_file_extension: 0,
            lpstr_def_ext: std::ptr::null(),
            l_cust_data: 0,
            lpfn_hook: std::ptr::null(),
            lp_template_name: std::ptr::null(),
            pv_reserved: std::ptr::null_mut(),
            dw_reserved: 0,
            flags_ex: 0,
        };
        let ok = unsafe { GetOpenFileNameW(&mut ofn) };
        if ok != 0 {
            let end = file_buf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(file_buf.len());
            let s = String::from_utf16_lossy(&file_buf[..end]);
            if !s.is_empty() {
                Some(std::path::PathBuf::from(s))
            } else {
                None
            }
        } else {
            None
        }
    }
    #[cfg(target_os = "macos")]
    {
        let prompt = if images_only {
            "POSIX path of (choose file with prompt \"Chọn hình ảnh để gửi qua Zalo\" of type {\"public.image\"})"
        } else {
            "POSIX path of (choose file with prompt \"Chọn tập tin đính kèm để gửi qua Zalo\")"
        };
        if let Ok(out) = std::process::Command::new("osascript")
            .args(["-e", prompt])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
        None
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let mut zcmd = std::process::Command::new("zenity");
        zcmd.arg("--file-selection");
        if images_only {
            zcmd.arg("--file-filter=Images | *.png *.jpg *.jpeg *.webp *.gif *.bmp");
        }
        if let Ok(out) = zcmd.output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
        if let Ok(out) = std::process::Command::new("kdialog")
            .arg("--getopenfilename")
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
        None
    }
}

/// Detect `Ctrl+V` / `Shift+Insert` (Windows) or `Cmd+V` (macOS) reliably even when `egui-winit`
/// swallows `Key::V` because the system clipboard contains an image instead of UTF-8 text.
pub(super) fn is_clipboard_paste_triggered(ctx: &egui::Context) -> bool {
    let egui_paste = ctx.input(|i| {
        (i.modifiers.ctrl && i.key_pressed(Key::V))
            || (i.modifiers.shift && i.key_pressed(Key::Insert))
            || i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
    });

    #[cfg(target_os = "windows")]
    {
        #[link(name = "user32")]
        extern "system" {
            fn GetAsyncKeyState(v_key: i32) -> i16;
        }
        static WAS_PASTE_DOWN: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);

        let is_focused = ctx.input(|i| i.focused);
        if !is_focused {
            WAS_PASTE_DOWN.store(false, std::sync::atomic::Ordering::SeqCst);
            return egui_paste;
        }
        let (win32_down, modifier_held) = unsafe {
            let ctrl = (GetAsyncKeyState(0x11) as u16 & 0x8000) != 0; // VK_CONTROL
            let v_key = (GetAsyncKeyState(0x56) as u16 & 0x8000) != 0; // 'V'
            let shift = (GetAsyncKeyState(0x10) as u16 & 0x8000) != 0; // VK_SHIFT
            let ins = (GetAsyncKeyState(0x2D) as u16 & 0x8000) != 0; // VK_INSERT
            ((ctrl && v_key) || (shift && ins), ctrl || shift)
        };
        if modifier_held {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        let prev = WAS_PASTE_DOWN.swap(win32_down, std::sync::atomic::Ordering::SeqCst);
        let edge_triggered = win32_down && !prev;
        return egui_paste || edge_triggered;
    }

    #[cfg(target_os = "macos")]
    {
        // egui-winit drops `Cmd+V` entirely when the clipboard has no text; the NSEvent
        // monitor in `macos_clipboard` records the press instead.
        super::macos_clipboard::install_paste_monitor(ctx);
        let cmd_v = super::macos_clipboard::take_cmd_v();
        egui_paste || cmd_v
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        egui_paste
    }
}

pub(super) fn finalize_clipboard_rgba_image(
    rgba: image::RgbaImage,
) -> Option<(std::path::PathBuf, u32, u32, egui::ColorImage)> {
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    let _ = std::fs::create_dir_all("stickers_cache");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = std::path::PathBuf::from(format!("stickers_cache/clipboard_{now_ms}.png"));
    rgba.save(&path).ok()?;

    let thumb = if width > 240 || height > 240 {
        image::imageops::thumbnail(&rgba, 240, 240)
    } else {
        rgba
    };
    let (tw, th) = thumb.dimensions();
    let col_img =
        egui::ColorImage::from_rgba_unmultiplied([tw as usize, th as usize], thumb.as_raw());
    Some((path, width, height, col_img))
}

/// Read a clipboard image as PNG. macOS: see `macos_clipboard::read_image`; Linux: wl-paste/xclip.
/// On Windows the following formats are supported:
/// 1. Registered `"PNG"` / `"image/png"` streams (Snipping Tool, ShareX, Chrome/Edge, Discord)
/// 2. `CF_HDROP` (`15`) copied image files from Windows Explorer or ShareX
/// 3. `CF_DIB` (`8`) / `CF_DIBV5` (`17`) 24-bit & 32-bit bitmaps (`BI_RGB` & `BI_BITFIELDS`)
pub(super) fn read_clipboard_image_to_png(
) -> Option<(std::path::PathBuf, u32, u32, egui::ColorImage)> {
    #[cfg(target_os = "windows")]
    {
        const CF_DIB: u32 = 8;
        const CF_DIBV5: u32 = 17;
        const CF_HDROP: u32 = 15;
        #[link(name = "user32")]
        extern "system" {
            fn IsClipboardFormatAvailable(format: u32) -> i32;
            fn RegisterClipboardFormatA(lpsz_format: *const u8) -> u32;
            fn OpenClipboard(h_wnd_new_owner: *mut std::ffi::c_void) -> i32;
            fn CloseClipboard() -> i32;
            fn GetClipboardData(u_format: u32) -> *mut std::ffi::c_void;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalLock(h_mem: *mut std::ffi::c_void) -> *const u8;
            fn GlobalUnlock(h_mem: *mut std::ffi::c_void) -> i32;
            fn GlobalSize(h_mem: *mut std::ffi::c_void) -> usize;
        }
        #[link(name = "shell32")]
        extern "system" {
            fn DragQueryFileW(
                h_drop: *mut std::ffi::c_void,
                i_file: u32,
                lpsz_file: *mut u16,
                cch: u32,
            ) -> u32;
        }

        // SAFETY: clipboard handles are inspected and released on this thread; locked data is copied before release.
        unsafe {
            let cf_png = RegisterClipboardFormatA(b"PNG\0".as_ptr());
            let cf_mime_png = RegisterClipboardFormatA(b"image/png\0".as_ptr());
            let has_png = cf_png != 0 && IsClipboardFormatAvailable(cf_png) != 0;
            let has_mime_png = cf_mime_png != 0 && IsClipboardFormatAvailable(cf_mime_png) != 0;
            let has_dib = IsClipboardFormatAvailable(CF_DIB) != 0
                || IsClipboardFormatAvailable(CF_DIBV5) != 0;
            let has_hdrop = IsClipboardFormatAvailable(CF_HDROP) != 0;

            if !has_png && !has_mime_png && !has_dib && !has_hdrop {
                return None;
            }

            // Retry OpenClipboard briefly in case egui-winit/arboard or ShareX is releasing the lock
            let mut opened = false;
            for _ in 0..6 {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    opened = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(8));
            }
            if !opened {
                return None;
            }

            // 1. Try raw PNG stream first (preserves exact RGBA and avoids DIB header quirks)
            for fmt in [cf_png, cf_mime_png] {
                if fmt != 0 && IsClipboardFormatAvailable(fmt) != 0 {
                    let hmem = GetClipboardData(fmt);
                    if !hmem.is_null() {
                        let ptr = GlobalLock(hmem);
                        let len = GlobalSize(hmem);
                        if !ptr.is_null() && len > 8 {
                            let bytes = std::slice::from_raw_parts(ptr, len).to_vec();
                            GlobalUnlock(hmem);
                            if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                                CloseClipboard();
                                return finalize_clipboard_rgba_image(dyn_img.to_rgba8());
                            }
                        } else if !ptr.is_null() {
                            GlobalUnlock(hmem);
                        }
                    }
                }
            }

            // 2. Try CF_HDROP (copied image file in Windows Explorer or ShareX)
            if has_hdrop {
                let hdrop = GetClipboardData(CF_HDROP);
                if !hdrop.is_null() {
                    let mut buf = vec![0u16; 1024];
                    let chars =
                        DragQueryFileW(hdrop, 0, buf.as_mut_ptr(), buf.len() as u32) as usize;
                    if chars > 0 && chars < buf.len() {
                        let file_path =
                            std::path::PathBuf::from(String::from_utf16_lossy(&buf[..chars]));
                        if let Ok(dyn_img) = image::open(&file_path) {
                            CloseClipboard();
                            let rgba = dyn_img.to_rgba8();
                            let (width, height) = rgba.dimensions();
                            let thumb = if width > 240 || height > 240 {
                                image::imageops::thumbnail(&rgba, 240, 240)
                            } else {
                                rgba
                            };
                            let (tw, th) = thumb.dimensions();
                            let col_img = egui::ColorImage::from_rgba_unmultiplied(
                                [tw as usize, th as usize],
                                thumb.as_raw(),
                            );
                            return Some((file_path, width, height, col_img));
                        }
                    }
                }
            }

            // Validate while borrowed, then release handles before pixel decoding.
            for format in [CF_DIBV5, CF_DIB] {
                if IsClipboardFormatAvailable(format) == 0 {
                    continue;
                }
                let handle = GetClipboardData(format);
                if handle.is_null() {
                    continue;
                }
                let len = GlobalSize(handle);
                if !(40..=crate::clipboard_dib::MAX_RAW).contains(&len) {
                    continue;
                }
                let ptr = GlobalLock(handle);
                if ptr.is_null() {
                    continue;
                }
                // SAFETY: GlobalSize bounds this locked allocation; no borrowed data escapes.
                let borrowed = std::slice::from_raw_parts(ptr, len);
                let owned = crate::clipboard_dib::validate_dib(borrowed)
                    .ok()
                    .map(|_| borrowed.to_vec());
                GlobalUnlock(handle);
                if let Some(bytes) = owned {
                    CloseClipboard();
                    return crate::clipboard_dib::decode_dib(&bytes)
                        .ok()
                        .and_then(finalize_clipboard_rgba_image);
                }
            }

            CloseClipboard();
            None
        }
    }
    #[cfg(target_os = "macos")]
    {
        // In-process NSPasteboard read: no osascript subprocess on every paste.
        super::macos_clipboard::read_image().and_then(finalize_clipboard_rgba_image)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Wayland (`wl-paste --type image/png`) or X11 (`xclip -selection clipboard -t image/png -o`)
        for (prog, args) in [
            ("wl-paste", &["--no-newline", "--type", "image/png"][..]),
            (
                "xclip",
                &["-selection", "clipboard", "-t", "image/png", "-o"][..],
            ),
        ] {
            if let Ok(out) = std::process::Command::new(prog).args(args).output() {
                if out.status.success() && out.stdout.len() > 8 {
                    if let Ok(dyn_img) = image::load_from_memory(&out.stdout) {
                        return finalize_clipboard_rgba_image(dyn_img.to_rgba8());
                    }
                }
            }
        }
        None
    }
}

/// Format a URL for clean UI display by stripping tracking query strings (`?_r=1&utm_...`)
/// and truncating to `max_chars`.
pub(super) fn clean_display_url(url: &str, max_chars: usize) -> String {
    let trimmed = url.trim();
    let no_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let base = no_scheme
        .split('?')
        .next()
        .unwrap_or(no_scheme)
        .trim_end_matches('/');
    truncate_chars(base, max_chars).into_owned()
}

/// Extract `(user_comment, url, domain)` from a message when `msg.link_url` is set or when
/// `msg.text` contains an embedded `http://` / `https://` URL.
///
/// Note: `user_comment` only contains extra text typed by the sender *outside* the link's own
/// `link_title` / `link_desc` (so `link_desc` is rendered inside the Link Preview Card rather than
/// duplicated as a large paragraph above the card).
pub(super) fn extract_clean_domain(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("Liên kết")
        .trim()
        .strip_prefix("www.")
        .unwrap_or_else(|| {
            url.split("://")
                .nth(1)
                .unwrap_or(url)
                .split('/')
                .next()
                .unwrap_or("Liên kết")
                .trim()
        })
        .to_string()
}

pub(super) fn extract_message_link_view(msg: &ChatMessage) -> Option<(String, String, String)> {
    if let Some(url) = msg.link_url.as_deref().filter(|u| !u.is_empty()) {
        if crate::chat::is_zalo_photo_cdn_url(url) {
            return None;
        }
        let desc_trim = msg.link_desc.as_deref().unwrap_or("").trim();
        let title_trim = msg.link_title.as_deref().unwrap_or("").trim();
        let url_base = url.split('?').next().unwrap_or(url);

        let mut kept_lines = Vec::new();
        for raw_line in msg.text.lines() {
            let mut line = raw_line.trim().to_string();
            if let Some(pos) = line.find("http://").or_else(|| line.find("https://")) {
                let after = &line[pos..];
                let end = after.find(char::is_whitespace).unwrap_or(after.len());
                let found = &after[..end];
                if found == url || found.starts_with(url_base) {
                    line = format!("{}{}", &line[..pos], &after[end..])
                        .trim()
                        .to_string();
                }
            }
            if line.is_empty()
                || line == url
                || line == title_trim
                || line == desc_trim
                || line == "[Chia sẻ]"
                || line == "[Liên kết]"
                || (!desc_trim.is_empty() && desc_trim.contains(&line))
                || (!title_trim.is_empty() && title_trim.contains(&line))
            {
                continue;
            }
            kept_lines.push(line);
        }
        let user_comment = kept_lines.join("\n").trim().to_string();
        let domain = extract_clean_domain(url);
        return Some((user_comment, url.to_string(), domain));
    }
    let text = msg.text.trim();
    let pos = text.find("https://").or_else(|| text.find("http://"))?;
    let after = &text[pos..];
    let end = after.find(char::is_whitespace).unwrap_or(after.len());
    let url = after[..end].trim_end_matches(|c: char| matches!(c, ')' | ']' | ',' | ';' | '>'));
    if url.len() < 10 || crate::chat::is_zalo_photo_cdn_url(url) {
        return None;
    }
    let caption = format!("{}{}", &text[..pos], &after[end..])
        .trim()
        .to_string();
    let domain = extract_clean_domain(url);
    Some((caption, url.to_string(), domain))
}

/// Convert days since 1970-01-01 into `(year, month, day)` (Howard Hinnant's algorithm).
pub(super) fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
    (year, m, d)
}

/// Normalize a millisecond or second timestamp to milliseconds (`> 0`).
pub(super) fn normalize_ts_ms(ts: i64) -> i64 {
    if ts <= 0 {
        0
    } else if ts < 10_000_000_000 {
        ts * 1000
    } else {
        ts
    }
}

/// Return `true` when a date/time separator pill should be inserted before `curr`.
pub(super) fn should_show_time_divider(prev: Option<&ChatMessage>, curr: &ChatMessage) -> bool {
    let Some(prev) = prev else {
        return true;
    };
    let prev_ms = normalize_ts_ms(prev.ts);
    let curr_ms = normalize_ts_ms(curr.ts);
    if prev_ms > 0 && curr_ms > 0 {
        let tz_offset_sec = 7 * 3600; // UTC+7 (Vietnam)
        let prev_day = (prev_ms / 1000 + tz_offset_sec) / 86_400;
        let curr_day = (curr_ms / 1000 + tz_offset_sec) / 86_400;
        if prev_day != curr_day {
            return true;
        }
        return (curr_ms - prev_ms).abs() >= 30 * 60 * 1000;
    }
    false
}

/// Format the label for a centered date/time separator pill.
pub(super) fn format_time_divider_label(msg: &ChatMessage) -> String {
    let ts_ms = normalize_ts_ms(msg.ts);
    if ts_ms <= 0 {
        return if msg.time.is_empty() {
            "Cuộc trò chuyện".to_string()
        } else {
            format!("Hôm nay • {}", msg.time)
        };
    }
    let tz_offset_sec = 7 * 3600;
    let local_sec = ts_ms / 1000 + tz_offset_sec;
    let msg_day = local_sec.div_euclid(86_400);
    let sec_of_day = local_sec.rem_euclid(86_400);
    let hh = sec_of_day / 3600;
    let mm = (sec_of_day % 3600) / 60;

    let now_sec = (crate::login::unix_now() as i64) + tz_offset_sec;
    let today_day = now_sec.div_euclid(86_400);
    let (y, m, d) = civil_from_days(msg_day);

    if msg_day == today_day {
        format!("Hôm nay • {hh:02}:{mm:02}")
    } else if msg_day == today_day - 1 {
        format!("Hôm qua • {hh:02}:{mm:02}")
    } else {
        format!("{d:02}/{m:02}/{y}")
    }
}

/// Render a centered Zalo-style compact date pill (no side hairlines).
pub(super) fn render_date_divider(ui: &mut egui::Ui, msg: &ChatMessage, pal: &ThemePalette) {
    let label = format_time_divider_label(msg);
    let avail_w = ui.available_width();
    ui.horizontal(|ui| {
        let text_w = ui.fonts(|f| {
            f.layout_no_wrap(
                label.clone(),
                FontId::proportional(11.5),
                pal.text_secondary,
            )
            .size()
            .x
        });
        let pill_w = text_w + 26.0;
        let left_space = ((avail_w - pill_w) * 0.5).max(0.0);
        if left_space > 0.0 {
            ui.add_space(left_space);
        }
        let pill_bg = if pal.is_dark {
            Color32::from_rgba_unmultiplied(30, 34, 41, 235)
        } else {
            Color32::from_rgba_unmultiplied(219, 228, 240, 215)
        };
        egui::Frame::none()
            .fill(pill_bg)
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::symmetric(13.0, 3.5))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(label)
                        .size(11.4)
                        .color(if pal.is_dark {
                            Color32::from_rgb(215, 218, 224)
                        } else {
                            pal.text_secondary
                        })
                        .strong(),
                );
            });
    });
}

/// Detect whether `msg` is a system/event notification rather than a user speech bubble.
pub(super) fn is_system_notice_message(msg: &ChatMessage) -> bool {
    if msg.id.starts_with("sys_") {
        return true;
    }
    let t = msg.text.trim();
    t.starts_with("[Hệ thống]")
        || t.starts_with("[Thông báo]")
        || (t.starts_with("Bạn và ") && t.contains("đã trở thành bạn bè"))
        || t.contains("đã thêm bạn vào nhóm")
        || t.contains("đã tham gia nhóm")
        || t.contains("đã rời khỏi nhóm")
        || t.contains("đã đổi tên nhóm")
}

/// Render a centered system event pill (e.g., friend connected, group event).
pub(super) fn render_system_notice_pill(ui: &mut egui::Ui, msg: &ChatMessage, pal: &ThemePalette) {
    let avail_w = ui.available_width();
    let text = msg.text.trim();
    ui.horizontal(|ui| {
        let text_w = ui.fonts(|f| {
            f.layout_no_wrap(
                text.to_string(),
                FontId::proportional(12.0),
                pal.text_secondary,
            )
            .size()
            .x
        });
        let pill_w = (text_w + 28.0).min(avail_w - 40.0);
        let left_pad = ((avail_w - pill_w) * 0.5).max(0.0);
        if left_pad > 0.0 {
            ui.add_space(left_pad);
        }
        egui::Frame::none()
            .fill(pal.bg_elevated)
            .stroke(Stroke::new(1.0_f32, pal.border))
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::symmetric(12.0, 4.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} {text}", lucide::SPARKLES))
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });
    });
}

/// Return `true` when two adjacent messages belong to the same visual bubble group.
/// Render chat message text with full Markdown support:
/// - Fenced code blocks (` ```lang ... ``` `)
/// - Headings (`#`, `##`, `###`)
/// - Bullet lists (`- `, `* `) and blockquotes (`> `)
/// - Inline `**bold**`, `*italic*`, `~~strikethrough~~`, `` `inline code` ``, and `[link](url)`
fn strip_markdown_syntax_for_width(s: &str) -> String {
    s.replace("```", "")
        .replace("**", "")
        .replace("__", "")
        .replace("~~", "")
        .replace('`', "")
}

/// Zalo tag blue for `@Name` spans, tuned per theme.
pub(super) fn mention_link_color(pal: &ThemePalette) -> Color32 {
    if pal.is_dark {
        Color32::from_rgb(77, 163, 255)
    } else {
        Color32::from_rgb(0, 104, 255)
    }
}

/// Byte offset of the first `@` that starts a word (so `mail@x.vn` is ignored).
fn find_mention_start(s: &str) -> Option<usize> {
    s.match_indices('@').map(|(i, _)| i).find(|&i| {
        s[..i].chars().next_back().map_or(true, |c| {
            c.is_whitespace() || matches!(c, '(' | '[' | '"' | '\'')
        })
    })
}

/// Exact `@Name` substrings tagged in `msg`, read from the wire `mentions` or the archive
/// `meta.mentionsList` (`pos`/`len` are UTF-16 offsets into `msg.text`).
///
/// Returns `None` when there is no usable metadata, so the renderer falls back to guessing;
/// `Some(empty)` means the message is known to carry no tags.
pub(super) fn message_mention_texts(msg: &ChatMessage) -> Option<Vec<String>> {
    let raw = msg.wire_ref.fields.get("mentions").or_else(|| {
        msg.history_source
            .as_ref()
            .and_then(|src| src.pointer("/meta/mentionsList"))
    })?;
    let entries = raw.as_array()?;
    if entries.is_empty() {
        return Some(Vec::new());
    }
    let as_usize = |v: Option<&serde_json::Value>| -> Option<usize> {
        let v = v?;
        v.as_u64()
            .map(|n| n as usize)
            .or_else(|| v.as_str().and_then(|t| t.trim().parse().ok()))
    };
    let units: Vec<u16> = msg.text.encode_utf16().collect();
    let mut out: Vec<String> = Vec::new();
    for m in entries {
        let (Some(pos), Some(len)) = (as_usize(m.get("pos")), as_usize(m.get("len"))) else {
            continue;
        };
        let Some(slice) = units.get(pos..pos.saturating_add(len)) else {
            continue;
        };
        let Ok(tag) = String::from_utf16(slice) else {
            continue;
        };
        if tag.starts_with('@') && tag.len() > 1 && !out.contains(&tag) {
            out.push(tag);
        }
    }
    // Entries we could not map (no pos/len, stale offsets) → let the heuristic try instead.
    (!out.is_empty()).then_some(out)
}

/// Link blue used for bare URLs (no preview card), tuned per theme.
fn url_link_color(pal: &ThemePalette) -> Color32 {
    if pal.is_dark {
        Color32::from_rgb(56, 139, 253)
    } else {
        Color32::from_rgb(30, 136, 229) // #1e88e5
    }
}

/// Prefixes that start a bare (non-Markdown) URL inside message text.
const BARE_URL_PREFIXES: [&str; 5] = [
    "https://",
    "http://",
    "www.",
    "docs.google.com/",
    "drive.google.com/",
];

/// Byte offset of the first bare URL in `s` that begins at a word boundary.
fn find_bare_url_start(s: &str) -> Option<usize> {
    let mut best: Option<usize> = None;
    for prefix in BARE_URL_PREFIXES {
        let mut from = 0;
        while let Some(rel) = s[from..].find(prefix) {
            let i = from + rel;
            let at_boundary = s[..i].chars().next_back().map_or(true, |c| {
                c.is_whitespace() || matches!(c, '(' | '<' | '"' | '\'')
            });
            if at_boundary {
                best = Some(best.map_or(i, |b| b.min(i)));
                break;
            }
            from = i + prefix.len();
        }
    }
    best
}

/// Byte length of the URL at the start of `s`, excluding trailing punctuation.
fn bare_url_len(s: &str) -> usize {
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    s[..end]
        .trim_end_matches(|c: char| {
            matches!(
                c,
                ')' | ']' | ',' | ';' | '>' | '.' | '!' | '?' | '"' | '\''
            )
        })
        .len()
}

/// Add an `https://` scheme to scheme-less URLs such as `docs.google.com/...`.
fn normalize_bare_url(u: &str) -> String {
    if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else {
        format!("https://{u}")
    }
}

/// Font size used for messages that contain only emoji.
pub(super) const EMOJI_ONLY_SIZE: f32 = 36.0;
/// Upper bound on clusters rendered in the large emoji-only style.
const EMOJI_ONLY_MAX: usize = 8;
/// Horizontal gap between emoji in the emoji-only style.
const EMOJI_ONLY_GAP: f32 = 2.0;

fn is_emoji_base(c: char) -> bool {
    matches!(c as u32,
        0x1F000..=0x1FAFF
            | 0x2600..=0x27BF
            | 0x2300..=0x23FF
            | 0x2B00..=0x2BFF
            | 0x2190..=0x21FF
            | 0x3030 | 0x303D | 0x3297 | 0x3299
            | 0x00A9 | 0x00AE | 0x203C | 0x2049 | 0x2122 | 0x2139)
}

fn is_emoji_modifier(c: char) -> bool {
    matches!(c as u32, 0xFE0E | 0xFE0F | 0x20E3 | 0xE0020..=0xE007F)
        || (0x1F3FB..=0x1F3FF).contains(&(c as u32))
}

fn is_regional_indicator(c: char) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&(c as u32))
}

/// Byte ranges of emoji clusters in `text` (ZWJ sequences, modifiers and flag pairs joined).
fn scan_emoji_clusters(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    let mut open = false;
    let mut join_next = false;
    let mut pending_ri = false;
    for (i, c) in text.char_indices() {
        let end = i + c.len_utf8();
        if open && (c == '\u{200D}' || is_emoji_modifier(c)) {
            if let Some(last) = out.last_mut() {
                last.end = end;
            }
            join_next = c == '\u{200D}';
            continue;
        }
        if is_emoji_base(c) {
            let extend = open && (join_next || (pending_ri && is_regional_indicator(c)));
            match out.last_mut() {
                Some(last) if extend => {
                    last.end = end;
                    pending_ri = false;
                }
                _ => {
                    out.push(i..end);
                    pending_ri = is_regional_indicator(c);
                }
            }
            open = true;
            join_next = false;
            continue;
        }
        open = false;
        join_next = false;
        pending_ri = false;
    }
    out
}

/// Split `text` into emoji clusters when it consists only of emoji (whitespace ignored).
/// Returns `None` for any other text or when there are too many emoji.
pub(super) fn emoji_only_clusters(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    let ranges = scan_emoji_clusters(text);
    if ranges.is_empty() || ranges.len() > EMOJI_ONLY_MAX {
        return None;
    }
    let mut cursor = 0;
    for r in &ranges {
        if !text[cursor..r.start].trim().is_empty() {
            return None;
        }
        cursor = r.end;
    }
    if !text[cursor..].trim().is_empty() {
        return None;
    }
    Some(ranges.into_iter().map(|r| text[r].to_string()).collect())
}

/// BMP characters whose default presentation is emoji (Unicode `Emoji_Presentation`).
const BMP_EMOJI_PRESENTATION: &[char] = &[
    '⌚', '⌛', '⏩', '⏪', '⏫', '⏬', '⏰', '⏳', '◽', '◾', '☔', '☕', '♈', '♉', '♊', '♋',
    '♌', '♍', '♎', '♏', '♐', '♑', '♒', '♓', '♿', '⚓', '⚡', '⚪', '⚫', '⚽', '⚾', '⛄',
    '⛅', '⛎', '⛔', '⛪', '⛲', '⛳', '⛵', '⛺', '⛽', '✅', '✊', '✋', '✨', '❌', '❎', '❓',
    '❔', '❕', '❗', '➕', '➖', '➗', '➰', '➿', '⬛', '⬜', '⭐', '⭕',
];

/// Whether an emoji cluster inside running text should be drawn as a colour emoji. Text-style
/// symbols such as `©`, `→` or `✓` stay as text unless followed by VS16.
fn is_inline_emoji_cluster(cluster: &str) -> bool {
    let Some(first) = cluster.chars().next() else {
        return false;
    };
    (first as u32) >= 0x1F000
        || cluster.contains('\u{FE0F}')
        || BMP_EMOJI_PRESENTATION.contains(&first)
}

/// Colour texture for an emoji cluster from the system font, requesting it from the worker
/// on first use. Returns `None` until it arrives (or forever if rasterisation failed).
fn system_emoji_texture<'a>(
    cluster: &str,
    textures: &'a HashMap<String, TextureHandle>,
    loading: &mut HashSet<String>,
) -> Option<&'a TextureHandle> {
    let key = crate::emoji_raster::texture_key(cluster);
    if let Some(tex) = textures.get(&key) {
        return Some(tex);
    }
    if loading.insert(key.clone()) {
        if let Some(tx) = WORKER_TX.get() {
            let _ = tx.send(WorkerCmd::RasterizeEmoji {
                key,
                cluster: cluster.to_string(),
            });
        }
    }
    None
}

/// Rebuild `job` so every inline emoji with a ready colour texture becomes its own transparent
/// placeholder section (first codepoint only, so ZWJ sequences take one cell). Returns the
/// section indices to overdraw with their textures.
fn split_emoji_sections(
    job: egui::text::LayoutJob,
    textures: &HashMap<String, TextureHandle>,
    loading: &mut HashSet<String>,
) -> (egui::text::LayoutJob, Vec<(u32, TextureHandle)>) {
    let mut out = job.clone();
    out.text.clear();
    out.sections.clear();
    let mut marks = Vec::new();
    for section in &job.sections {
        let text = &job.text[section.byte_range.clone()];
        let mut leading = section.leading_space;
        let mut cursor = 0;
        for r in scan_emoji_clusters(text) {
            let cluster = &text[r.clone()];
            if !is_inline_emoji_cluster(cluster) {
                continue;
            }
            let Some(tex) = system_emoji_texture(cluster, textures, loading) else {
                continue;
            };
            if cursor < r.start {
                out.append(&text[cursor..r.start], leading, section.format.clone());
                leading = 0.0;
            }
            let placeholder_len = cluster.chars().next().map_or(0, char::len_utf8);
            let mut fmt = section.format.clone();
            fmt.color = Color32::TRANSPARENT;
            fmt.underline = Stroke::NONE;
            fmt.strikethrough = Stroke::NONE;
            out.append(&cluster[..placeholder_len], leading, fmt);
            leading = 0.0;
            marks.push(((out.sections.len() - 1) as u32, tex.clone()));
            cursor = r.end;
        }
        if cursor < text.len() {
            out.append(&text[cursor..], leading, section.format.clone());
        }
    }
    if marks.is_empty() {
        (job, marks)
    } else {
        (out, marks)
    }
}

/// Paint colour emoji textures over their transparent placeholder glyphs.
fn paint_emoji_marks(
    ui: &egui::Ui,
    galley: &egui::Galley,
    origin: Pos2,
    marks: &[(u32, TextureHandle)],
) {
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    for row in &galley.rows {
        for glyph in &row.glyphs {
            let Some((_, tex)) = marks.iter().find(|(idx, _)| *idx == glyph.section_index) else {
                continue;
            };
            let cell = glyph.logical_rect().translate(origin.to_vec2());
            let side = glyph.font_height.min(cell.height());
            let rect = Rect::from_center_size(cell.center(), Vec2::splat(side));
            ui.painter().image(tex.id(), rect, uv, Color32::WHITE);
        }
    }
}

/// Colour texture key for an emoji that ships as a bundled PNG (reaction set only).
fn emoji_texture_key_exact(e: &str) -> Option<&'static str> {
    match e.trim_end_matches('\u{FE0F}') {
        "👍" => Some("like"),
        "❤" => Some("heart"),
        "😆" => Some("smile"),
        "😮" => Some("surprised"),
        "😢" => Some("cry"),
        "😠" => Some("rage"),
        _ => None,
    }
}

/// Render an emoji-only message large: system colour emoji first, then the bundled reaction
/// PNG, then the monochrome font glyph.
fn render_emoji_only(
    ui: &mut egui::Ui,
    clusters: &[String],
    text_col: Color32,
    sticker_textures: &HashMap<String, TextureHandle>,
    loading_stickers: &mut HashSet<String>,
    emoji_textures: &HashMap<String, TextureHandle>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = EMOJI_ONLY_GAP;
        for e in clusters {
            let tex = system_emoji_texture(e, sticker_textures, loading_stickers)
                .or_else(|| emoji_texture_key_exact(e).and_then(|k| emoji_textures.get(k)));
            match tex {
                Some(tex) => {
                    ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::splat(EMOJI_ONLY_SIZE)));
                }
                None => {
                    ui.label(RichText::new(e).size(EMOJI_ONLY_SIZE).color(text_col));
                }
            }
        }
    });
}

/// Width `render_emoji_only` will use for one cluster (texture cell or glyph advance).
fn emoji_only_cell_width(
    ui: &egui::Ui,
    cluster: &str,
    sticker_textures: &HashMap<String, TextureHandle>,
    emoji_textures: &HashMap<String, TextureHandle>,
) -> f32 {
    let has_tex = sticker_textures.contains_key(&crate::emoji_raster::texture_key(cluster))
        || emoji_texture_key_exact(cluster).is_some_and(|k| emoji_textures.contains_key(k));
    if has_tex {
        EMOJI_ONLY_SIZE
    } else {
        ui.fonts(|f| {
            f.layout_no_wrap(
                cluster.to_string(),
                FontId::proportional(EMOJI_ONLY_SIZE),
                Color32::WHITE,
            )
            .size()
            .x
        })
    }
}

/// Text placed on the clipboard by "Copy tin nhắn": the original URL for link shares
/// (plus the sender's own comment, if any) instead of the preview title/description.
pub(super) fn message_copy_text(msg: &ChatMessage) -> String {
    let is_media = is_sticker_message(msg).is_some()
        || is_zalo_photo_message(msg)
        || is_zalo_file_or_video_message(msg);
    // Without an attached `link_url` the text is what the sender typed (URL included), so it
    // is copied verbatim; only attached link shares carry preview text worth replacing.
    let has_attached_link = msg
        .link_url
        .as_deref()
        .is_some_and(|u| !u.trim().is_empty());
    if !is_media && has_attached_link {
        if let Some((comment, url, _)) = extract_message_link_view(msg) {
            return if comment.is_empty() {
                url
            } else {
                format!("{comment}\n{url}")
            };
        }
    }
    msg.text.clone()
}

fn render_markdown_inline_rich(
    ui: &mut egui::Ui,
    text: &str,
    base_size: f32,
    base_color: Color32,
    pal: &ThemePalette,
    textures: &HashMap<String, TextureHandle>,
    loading: &mut HashSet<String>,
    mentions: Option<&[String]>,
) {
    let mut job = egui::text::LayoutJob::default();
    let mention_fmt = egui::TextFormat {
        font_id: FontId::proportional(base_size),
        color: mention_link_color(pal),
        ..Default::default()
    };
    let mut first_link: Option<String> = None;
    let mut has_bare_url = false;
    let url_col = url_link_color(pal);
    // Hover state from the previous frame drives the hover-only underline on bare URLs.
    let hover_id = Id::new(("md_bare_url_hover", text));
    let url_hovered = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(hover_id))
        .unwrap_or(false);
    let mut rest = text;

    while !rest.is_empty() {
        let next_bold = rest.find("**");
        let next_under = rest.find("__");
        let next_strike = rest.find("~~");
        let next_code = rest.find('`');
        let next_italic = rest.find('*').filter(|&i| rest.get(i..i + 2) != Some("**"));
        let next_link = rest.find('[').and_then(|lb| {
            let after_lb = &rest[lb + 1..];
            let rb = after_lb.find("](")?;
            let after_rb = &after_lb[rb + 2..];
            let rp = after_rb.find(')')?;
            Some((lb, lb + 1 + rb, lb + 1 + rb + 2 + rp))
        });
        let next_url = find_bare_url_start(rest);
        let next_at = find_mention_start(rest);

        let min_pos = [
            next_url,
            next_at,
            next_bold,
            next_under,
            next_strike,
            next_code,
            next_italic,
            next_link.map(|(lb, _, _)| lb),
        ]
        .into_iter()
        .flatten()
        .min();
        let Some(pos) = min_pos else {
            job.append(
                rest,
                0.0,
                egui::TextFormat {
                    font_id: FontId::proportional(base_size),
                    color: base_color,
                    ..Default::default()
                },
            );
            break;
        };

        if pos > 0 {
            job.append(
                &rest[..pos],
                0.0,
                egui::TextFormat {
                    font_id: FontId::proportional(base_size),
                    color: base_color,
                    ..Default::default()
                },
            );
            rest = &rest[pos..];
            continue;
        }

        if next_url == Some(0) {
            let len = bare_url_len(rest);
            if len >= 8 {
                let url_str = &rest[..len];
                if first_link.is_none() {
                    first_link = Some(normalize_bare_url(url_str));
                }
                has_bare_url = true;
                job.append(
                    url_str,
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::proportional(base_size),
                        color: url_col,
                        underline: if url_hovered {
                            Stroke::new(1.0_f32, url_col)
                        } else {
                            Stroke::NONE
                        },
                        ..Default::default()
                    },
                );
                rest = &rest[len..];
                continue;
            }
        }

        if let Some((0, rb_idx, rp_idx)) = next_link {
            let label = &rest[1..rb_idx];
            let url = &rest[rb_idx + 2..rp_idx];
            if first_link.is_none() && (url.starts_with("http://") || url.starts_with("https://")) {
                first_link = Some(url.to_string());
            }
            job.append(
                label.trim_matches('*').trim_matches('`'),
                0.0,
                egui::TextFormat {
                    font_id: FontId::proportional(base_size),
                    color: pal.primary_hover,
                    underline: Stroke::new(1.0_f32, pal.primary_hover),
                    ..Default::default()
                },
            );
            rest = &rest[rp_idx + 1..];
            continue;
        }

        // Tags: exact `@Name` spans from the message's `mentions` metadata win; without
        // metadata, fall back to guessing `@All` / `@Name` from the text.
        if rest.starts_with('@') {
            if let Some(list) = mentions {
                if let Some(tag) = list
                    .iter()
                    .filter(|m| rest.starts_with(m.as_str()))
                    .max_by_key(|m| m.len())
                {
                    job.append(tag, 0.0, mention_fmt.clone());
                    rest = &rest[tag.len()..];
                    continue;
                }
            }
        }
        if rest.starts_with('@') && mentions.is_none() {
            let after = &rest[1..];
            let is_all = after.starts_with("All")
                || after.starts_with("all")
                || after.starts_with("ALL")
                || after.starts_with("Tất cả")
                || after.starts_with("tất cả")
                || after.starts_with("Tat ca")
                || after.starts_with("tat ca");
            if is_all {
                let all_len = if after.starts_with("Tất cả") || after.starts_with("tất cả")
                {
                    "Tất cả".len()
                } else if after.starts_with("Tat ca") || after.starts_with("tat ca") {
                    6
                } else {
                    3
                };
                let next_char = after[all_len..].chars().next();
                if next_char.is_none()
                    || next_char.unwrap().is_whitespace()
                    || matches!(next_char.unwrap(), '.' | ',' | '!' | '?' | ':' | ';')
                {
                    let mention_text = &rest[..1 + all_len];
                    job.append(mention_text, 0.0, mention_fmt.clone());
                    rest = &after[all_len..];
                    continue;
                }
            }

            let mut end_idx = 0;
            let mut word_count = 0;
            let mut prev_is_space = false;
            for (idx, ch) in after.char_indices() {
                if ch == '\n'
                    || ch == '\r'
                    || matches!(
                        ch,
                        ',' | '.'
                            | '!'
                            | '?'
                            | ':'
                            | ';'
                            | '('
                            | ')'
                            | '['
                            | ']'
                            | '"'
                            | '\''
                            | '`'
                            | '<'
                            | '>'
                    )
                {
                    break;
                }
                if ch.is_whitespace() {
                    word_count += 1;
                    if word_count >= 5 || prev_is_space {
                        break;
                    }
                    prev_is_space = true;
                } else {
                    prev_is_space = false;
                }
                end_idx = idx + ch.len_utf8();
            }
            let candidate_name = after[..end_idx].trim_end();
            if !candidate_name.is_empty()
                && candidate_name.chars().next().unwrap().is_alphanumeric()
            {
                let mention_len = 1 + candidate_name.len();
                let mention_str = &rest[..mention_len];
                job.append(mention_str, 0.0, mention_fmt.clone());
                rest = &rest[mention_len..];
                continue;
            }
        }

        if rest.starts_with("**") {
            let after = &rest[2..];
            if let Some(end_b) = after.find("**") {
                job.append(
                    &after[..end_b],
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::proportional(base_size + 0.4),
                        color: if pal.is_dark {
                            Color32::WHITE
                        } else {
                            pal.text_primary
                        },
                        ..Default::default()
                    },
                );
                rest = &after[end_b + 2..];
                continue;
            }
        }

        if rest.starts_with("__") {
            let after = &rest[2..];
            if let Some(end_u) = after.find("__") {
                job.append(
                    &after[..end_u],
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::proportional(base_size),
                        color: base_color,
                        underline: Stroke::new(1.0_f32, base_color),
                        ..Default::default()
                    },
                );
                rest = &after[end_u + 2..];
                continue;
            }
        }

        if rest.starts_with("~~") {
            let after = &rest[2..];
            if let Some(end_s) = after.find("~~") {
                job.append(
                    &after[..end_s],
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::proportional(base_size),
                        color: base_color.linear_multiply(0.75),
                        strikethrough: Stroke::new(1.0_f32, base_color),
                        ..Default::default()
                    },
                );
                rest = &after[end_s + 2..];
                continue;
            }
        }
        if rest.starts_with('*') && !rest.starts_with("**") {
            let after = &rest[1..];
            if let Some(end_i) = after.find('*') {
                if end_i > 0 {
                    job.append(
                        &after[..end_i],
                        0.0,
                        egui::TextFormat {
                            font_id: FontId::proportional(base_size),
                            color: base_color,
                            italics: true,
                            ..Default::default()
                        },
                    );
                    rest = &after[end_i + 1..];
                    continue;
                }
            }
        }

        if rest.starts_with('`') {
            let after = &rest[1..];
            if let Some(end_c) = after.find('`') {
                job.append(
                    &after[..end_c],
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::monospace((base_size - 0.8).max(11.0)),
                        color: pal.primary_hover,
                        background: pal.bg_input,
                        ..Default::default()
                    },
                );
                rest = &after[end_c + 1..];
                continue;
            }
        }

        let mut chars = rest.chars();
        if let Some(ch) = chars.next() {
            let mut buf = [0u8; 4];
            job.append(
                ch.encode_utf8(&mut buf),
                0.0,
                egui::TextFormat {
                    font_id: FontId::proportional(base_size),
                    color: base_color,
                    ..Default::default()
                },
            );
            rest = chars.as_str();
        } else {
            break;
        }
    }

    let (job, emoji_marks) = split_emoji_sections(job, textures, loading);
    let add_label = |ui: &mut egui::Ui, label: egui::Label| {
        if first_link.is_some() {
            ui.add(label.sense(Sense::click()))
        } else {
            ui.add(label)
        }
    };
    let resp = if emoji_marks.is_empty() {
        add_label(ui, egui::Label::new(job).wrap())
    } else {
        // Lay out here (instead of inside `Label`) to know where each placeholder glyph lands.
        let mut job = job;
        job.wrap.max_width = ui.available_width();
        let galley = ui.fonts(|f| f.layout_job(job));
        let resp = add_label(ui, egui::Label::new(galley.clone()));
        paint_emoji_marks(ui, &galley, resp.rect.min, &emoji_marks);
        resp
    };
    if let Some(url) = first_link {
        if has_bare_url && resp.hovered() != url_hovered {
            let hovered = resp.hovered();
            ui.ctx().data_mut(|d| d.insert_temp(hover_id, hovered));
            ui.ctx().request_repaint();
        }
        if resp
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(&url)
            .clicked()
        {
            open_external_url(&url);
        }
    }
}

pub(super) fn render_chat_markdown_block(
    ui: &mut egui::Ui,
    raw: &str,
    text_col: Color32,
    pal: &ThemePalette,
    textures: &HashMap<String, TextureHandle>,
    loading: &mut HashSet<String>,
    mentions: Option<&[String]>,
) {
    let mut in_code_block = false;
    let mut code_lines: Vec<&str> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim_start().starts_with("```") {
            if in_code_block {
                let code_text = code_lines.join("\n");
                code_lines.clear();
                in_code_block = false;
                egui::Frame::none()
                    .fill(pal.bg_input)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(6.0))
                    .inner_margin(Margin::symmetric(10.0, 7.0))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(code_text)
                                    .font(FontId::monospace(12.6))
                                    .color(pal.primary_hover),
                            )
                            .wrap(),
                        );
                    });
            } else {
                in_code_block = true;
            }
            continue;
        }
        if in_code_block {
            code_lines.push(line);
            continue;
        }
        let t_start = trimmed.trim_start();
        if let Some(h3) = t_start.strip_prefix("### ") {
            render_markdown_inline_rich(ui, h3, 15.0, text_col, pal, textures, loading, mentions);
        } else if let Some(h2) = t_start.strip_prefix("## ") {
            render_markdown_inline_rich(ui, h2, 16.0, text_col, pal, textures, loading, mentions);
        } else if let Some(h1) = t_start.strip_prefix("# ") {
            render_markdown_inline_rich(ui, h1, 17.0, text_col, pal, textures, loading, mentions);
        } else if let Some(quote) = t_start.strip_prefix("> ") {
            ui.horizontal(|ui| {
                let (bar_r, _) = ui.allocate_exact_size(Vec2::new(3.0, 16.0), Sense::hover());
                ui.painter()
                    .rect_filled(bar_r, Rounding::same(1.5), pal.primary_hover);
                render_markdown_inline_rich(
                    ui,
                    quote,
                    13.8,
                    text_col.linear_multiply(0.88),
                    pal,
                    textures,
                    loading,
                    mentions,
                );
            });
        } else if let Some(bullet) = t_start
            .strip_prefix("- ")
            .or_else(|| t_start.strip_prefix("* "))
        {
            let bullet_line = format!("• {bullet}");
            render_markdown_inline_rich(
                ui,
                &bullet_line,
                15.0,
                text_col,
                pal,
                textures,
                loading,
                mentions,
            );
        } else {
            render_markdown_inline_rich(
                ui, trimmed, 15.0, text_col, pal, textures, loading, mentions,
            );
        }
    }
    if !code_lines.is_empty() {
        let code_text = code_lines.join("\n");
        egui::Frame::none()
            .fill(pal.bg_input)
            .stroke(Stroke::new(1.0_f32, pal.border))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::symmetric(10.0, 7.0))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(code_text)
                            .font(FontId::monospace(12.6))
                            .color(pal.primary_hover),
                    )
                    .wrap(),
                );
            });
    }
}

pub(super) fn same_message_cluster(a: &ChatMessage, b: &ChatMessage) -> bool {
    if is_system_notice_message(a) || is_system_notice_message(b) {
        return false;
    }
    if a.is_outgoing != b.is_outgoing || a.sender != b.sender {
        return false;
    }
    if should_show_time_divider(Some(a), b) {
        return false;
    }
    let a_ms = normalize_ts_ms(a.ts);
    let b_ms = normalize_ts_ms(b.ts);
    if a_ms > 0 && b_ms > 0 && (b_ms - a_ms).abs() > 10 * 60 * 1000 {
        return false;
    }
    true
}

pub(super) fn render_message_body(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    thread_name: &str,
    pal: &ThemePalette,
    outgoing: bool,
    max_text_width: f32,
    sticker_textures: &HashMap<String, TextureHandle>,
    loading_stickers: &mut HashSet<String>,
    emoji_textures: &HashMap<String, TextureHandle>,
) -> Option<BubbleAction> {
    let mut body_action = None;
    let mention_texts = message_mention_texts(msg);
    let (text_col, meta_col) = if outgoing {
        (pal.bubble_out_text, pal.bubble_out_meta)
    } else {
        (pal.bubble_in_text, pal.bubble_in_meta)
    };

    let trimmed_text = msg.text.trim();
    if trimmed_text == "[Tin nhắn đã thu hồi]" {
        ui.label(
            RichText::new(format!("{} Tin nhắn đã được thu hồi", lucide::BAN))
                .color(meta_col)
                .italics()
                .size(13.8),
        );
        return None;
    }

    if trimmed_text.starts_with("[Đã thu hồi]") {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} Đã thu hồi", lucide::BAN))
                    .color(Color32::from_rgb(239, 68, 68))
                    .italics()
                    .strong()
                    .size(12.0),
            );
        });
        ui.add_space(2.0);
    }

    let sender_label = if msg.is_outgoing {
        "Bạn".to_string()
    } else if !msg.sender.is_empty() && msg.sender != "0" {
        msg.sender.clone()
    } else {
        thread_name.to_string()
    };

    // Dedicated inline Photo Card when the message is an uploaded/received Zalo photo (#72)
    let is_zalo_photo = is_zalo_photo_message(msg);
    if is_zalo_photo {
        if let Some(thumb_url) = photo_primary_url(msg) {
            let key = format!("thumb_{}", crate::crypto::md5_hex(thumb_url));
            if !sticker_textures.contains_key(&key) && loading_stickers.insert(key.clone()) {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::FetchStickerImage {
                        key: key.clone(),
                        url: thumb_url.to_string(),
                    });
                }
            }
            let open_target = msg
                .link_url
                .as_deref()
                .filter(|u| !u.is_empty() && crate::chat::is_zalo_photo_cdn_url(u))
                .unwrap_or(thumb_url)
                .to_string();
            let title_label = msg
                .link_title
                .as_deref()
                .map(str::trim)
                .filter(|t| {
                    !t.is_empty()
                        && !t.starts_with("http://")
                        && !t.starts_with("https://")
                        && !t.ends_with(".zdn.vn")
                        && !t.ends_with(".zadn.vn")
                        && !t.ends_with(".dlfl.vn")
                        && !t.contains("zpcloud")
                        && !t.contains("zpg-r")
                        && !t.contains("photo-stal")
                        && !t.contains("photo-zmp")
                })
                .unwrap_or("Hình ảnh")
                .to_string();

            if let Some(tex) = sticker_textures.get(&key) {
                let [tw, th] = tex.size();
                if tw <= 2 || th <= 2 {
                    // Expired or empty tombstone image on Zalo CDN (e.g. 1x1 jpeg)
                    let ph_w = max_text_width.clamp(180.0, 260.0);
                    let ph_frame = egui::Frame::none()
                        .fill(text_col.linear_multiply(0.05))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .rounding(Rounding::same(12.0))
                        .inner_margin(Margin::symmetric(16.0, 14.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ph_w);
                            ui.vertical_centered(|ui| {
                                let (ir, _) =
                                    ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                                draw_icon_image(
                                    ui.painter(),
                                    ir.center(),
                                    22.0,
                                    text_col.linear_multiply(0.4),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Ảnh đã hết hạn trên Zalo")
                                        .size(12.5)
                                        .color(text_col.linear_multiply(0.6)),
                                );
                            });
                        });
                    if ui
                        .interact(
                            ph_frame.response.rect,
                            Id::new(("photo_exp_click", &msg.id, msg.ts)),
                            Sense::click(),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Bấm để thử mở liên kết ảnh gốc")
                        .clicked()
                    {
                        open_external_url(&open_target);
                    }
                } else {
                    let tw_f = tw as f32;
                    let th_f = th as f32;
                    let max_w = max_text_width.clamp(220.0, 360.0);
                    let max_h = 340.0_f32;
                    let scale = (max_w / tw_f).min(max_h / th_f).min(1.0);
                    let disp_w = (tw_f * scale).clamp(160.0, max_w);
                    let disp_h = (th_f * scale).clamp(120.0, max_h);
                    let img = egui::Image::new(tex)
                        .fit_to_exact_size(Vec2::new(disp_w, disp_h))
                        .rounding(Rounding::same(12.0))
                        .sense(Sense::click());
                    let resp = ui
                        .add(img)
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Bấm để xem ảnh phóng to trong ứng dụng");
                    ui.painter().rect_stroke(
                        resp.rect,
                        Rounding::same(12.0),
                        Stroke::new(
                            if resp.hovered() { 1.6_f32 } else { 1.0_f32 },
                            if resp.hovered() {
                                pal.primary
                            } else {
                                pal.border
                            },
                        ),
                    );
                    if resp.hovered() {
                        ui.painter().rect_filled(
                            resp.rect,
                            Rounding::same(12.0),
                            Color32::from_rgba_unmultiplied(6, 10, 20, 88),
                        );
                        let pill_r =
                            Rect::from_center_size(resp.rect.center(), Vec2::new(108.0, 26.0));
                        ui.painter().rect_filled(
                            pill_r,
                            Rounding::same(13.0),
                            Color32::from_rgba_unmultiplied(15, 23, 42, 225),
                        );
                        ui.painter().text(
                            pill_r.center(),
                            egui::Align2::CENTER_CENTER,
                            "Phóng to ảnh",
                            FontId::proportional(11.5),
                            Color32::WHITE,
                        );
                    }
                    if resp.clicked() {
                        body_action = Some(BubbleAction::OpenImagePopup {
                            tex_key: key,
                            full_url: open_target,
                            title: title_label,
                            sender: sender_label,
                            time: msg.time.clone(),
                        });
                    }
                }
            } else {
                // Loading / Placeholder frame with visual image icon and loading feedback
                let disp_w = max_text_width.clamp(200.0, 280.0);
                let disp_h = 150.0_f32;
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(disp_w, disp_h), Sense::click());
                let is_hover = resp.hovered();
                let bg_col = if is_hover {
                    text_col.linear_multiply(0.09)
                } else {
                    text_col.linear_multiply(0.05)
                };
                let stroke_col = if is_hover { pal.primary } else { pal.border };
                ui.painter().rect_filled(rect, Rounding::same(12.0), bg_col);
                ui.painter().rect_stroke(
                    rect,
                    Rounding::same(12.0),
                    Stroke::new(1.0_f32, stroke_col),
                );

                let icon_center = rect.center() - Vec2::new(0.0, 14.0);
                draw_icon_image(
                    ui.painter(),
                    icon_center,
                    26.0,
                    text_col.linear_multiply(0.45),
                );
                let label_text = if title_label != "Hình ảnh" {
                    title_label.clone()
                } else {
                    "Đang tải hình ảnh...".to_string()
                };
                ui.painter().text(
                    rect.center() + Vec2::new(0.0, 20.0),
                    egui::Align2::CENTER_CENTER,
                    label_text,
                    FontId::proportional(12.5),
                    text_col.linear_multiply(0.7),
                );

                if resp
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text("Đang tải ảnh · Bấm để xem ảnh phóng to")
                    .clicked()
                {
                    body_action = Some(BubbleAction::OpenImagePopup {
                        tex_key: key,
                        full_url: open_target,
                        title: title_label,
                        sender: sender_label,
                        time: msg.time.clone(),
                    });
                }
            }
            if let Some(cap) = extract_photo_user_caption(msg) {
                ui.add_space(5.0);
                render_chat_markdown_block(
                    ui,
                    cap,
                    text_col,
                    pal,
                    sticker_textures,
                    loading_stickers,
                    mention_texts.as_deref(),
                );
            }
            return body_action;
        }
    }
    // Dedicated File / Video Attachment Card (#74)
    let is_zalo_file_or_video = is_zalo_file_or_video_message(msg);
    if is_zalo_file_or_video {
        let is_vid =
            msg.link_src.as_deref() == Some("Video Zalo") || trimmed_text.starts_with("[Video]");
        let url_str = msg.link_url.as_deref().unwrap_or_default();
        let resolved_file_name = crate::chat::resolve_attachment_file_name(
            msg.link_title.as_deref(),
            trimmed_text,
            url_str,
        );
        let title_str = if is_vid {
            msg.link_title
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(trimmed_text)
                .to_string()
        } else {
            resolved_file_name.clone()
        };
        let sub_str = if is_vid {
            "Video đính kèm · Bấm để mở"
        } else {
            "Tập tin Zalo · Bấm để lưu về máy"
        };
        let link_col = if pal.is_dark {
            Color32::from_rgb(97, 175, 239)
        } else {
            pal.primary
        };
        let resp = ui
            .add(
                egui::Label::new(
                    RichText::new(&title_str)
                        .size(13.8)
                        .color(text_col)
                        .strong(),
                )
                .wrap()
                .sense(Sense::click()),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(if is_vid {
                "Bấm để mở video"
            } else {
                "Bấm để tải và lưu tập tin với tên gốc"
            });
        ui.add_space(2.0);
        let dom_resp = ui
            .add(
                egui::Label::new(RichText::new(sub_str).size(12.5).color(link_col))
                    .sense(Sense::click()),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        if resp.clicked() || dom_resp.clicked() {
            if let Some(url) = msg.link_url.as_deref().filter(|u| !u.is_empty()) {
                if is_vid && !url.to_ascii_lowercase().contains("/renewmedia") {
                    open_external_url(url);
                } else {
                    return Some(BubbleAction::DownloadFile {
                        url: url.to_string(),
                        file_name: resolved_file_name,
                    });
                }
            }
        }
        return None;
    }

    if !trimmed_text.starts_with("[Đã thu hồi]") {
        if let Some(clusters) = emoji_only_clusters(trimmed_text) {
            render_emoji_only(
                ui,
                &clusters,
                text_col,
                sticker_textures,
                loading_stickers,
                emoji_textures,
            );
            return None;
        }
    }

    let Some((caption, url, domain)) = extract_message_link_view(msg) else {
        let display_text = if let Some(stripped) = msg.text.strip_prefix("[Đã thu hồi] ") {
            stripped
        } else {
            &msg.text
        };
        render_chat_markdown_block(
            ui,
            display_text,
            text_col,
            pal,
            sticker_textures,
            loading_stickers,
            mention_texts.as_deref(),
        );
        return None;
    };

    let link_col = if pal.is_dark {
        Color32::from_rgb(97, 175, 239) // #61AFEF Zalo/Atom clean link blue
    } else {
        pal.primary
    };

    // 1. Optional user comment above the link
    if !caption.is_empty() && caption != url {
        render_chat_markdown_block(
            ui,
            &caption,
            text_col,
            pal,
            sticker_textures,
            loading_stickers,
            mention_texts.as_deref(),
        );
        ui.add_space(4.0);
    }

    // 2. Optional compact thumbnail directly inside the bubble (no nested card box)
    if let Some(thumb_url) = msg.link_thumb.as_deref().filter(|u| !u.is_empty()) {
        let key = format!("thumb_{}", crate::crypto::md5_hex(thumb_url));
        if !sticker_textures.contains_key(&key)
            && (thumb_url.starts_with("http://") || thumb_url.starts_with("https://"))
            && loading_stickers.insert(key.clone())
        {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchStickerImage {
                    key: key.clone(),
                    url: thumb_url.to_string(),
                });
            }
        }
        if let Some(tex) = sticker_textures.get(&key) {
            let [tw, th] = tex.size();
            let aspect = if tw > 0 && th > 0 {
                (th as f32 / tw as f32).clamp(0.45, 0.85)
            } else {
                0.56
            };
            let thumb_w = max_text_width.min(260.0);
            let img_h = (thumb_w * aspect).min(132.0);
            let img = egui::Image::new(tex)
                .fit_to_exact_size(Vec2::new(thumb_w, img_h))
                .rounding(Rounding::same(6.0))
                .sense(Sense::click());
            if ui
                .add(img)
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(&url)
                .clicked()
            {
                open_external_url(&url);
            }
            ui.add_space(5.0);
        }
    }

    // 2b. No preview at all (no thumbnail, title or description): show the raw URL as a
    // blue link, underlined on hover, instead of a bold title that looks like plain text.
    let has_thumb = msg.link_thumb.as_deref().is_some_and(|u| !u.is_empty());
    let has_title = msg
        .link_title
        .as_deref()
        .map(str::trim)
        .is_some_and(|t| !t.is_empty() && t != domain && t != url);
    let has_desc = msg
        .link_desc
        .as_deref()
        .map(str::trim)
        .is_some_and(|d| !d.is_empty() && d != url);
    if !has_thumb && !has_title && !has_desc {
        let url_col = url_link_color(pal);
        let hover_id = Id::new(("bare_link_hover", &msg.id, msg.ts));
        let was_hovered = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(hover_id))
            .unwrap_or(false);
        let mut rt = RichText::new(truncate_chars(&url, 110))
            .size(13.8)
            .color(url_col);
        if was_hovered {
            rt = rt.underline();
        }
        let resp = ui
            .add(egui::Label::new(rt).wrap().sense(Sense::click()))
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(&url);
        if resp.hovered() != was_hovered {
            let hovered = resp.hovered();
            ui.ctx().data_mut(|d| d.insert_temp(hover_id, hovered));
            ui.ctx().request_repaint();
        }
        if resp.clicked() {
            open_external_url(&url);
        }
        return body_action;
    }

    // 3. Bold Title (exact Zalo PC style: no globe icon, no vertical accent bar, no nested border)
    let header_title = msg
        .link_title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != domain)
        .unwrap_or_else(|| {
            url.strip_prefix("https://")
                .or_else(|| url.strip_prefix("http://"))
                .unwrap_or(&url)
        });
    let title_resp = ui
        .add(
            egui::Label::new(
                RichText::new(truncate_chars(header_title, 95))
                    .size(13.8)
                    .color(text_col)
                    .strong(),
            )
            .wrap()
            .sense(Sense::click()),
        )
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(&url);
    if title_resp.clicked() {
        open_external_url(&url);
    }

    // 4. Optional short description (only if concise and distinct)
    let desc_opt = msg
        .link_desc
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty() && *d != header_title && *d != url && caption.is_empty());
    if let Some(desc) = desc_opt {
        ui.add_space(2.0);
        let d_resp = ui
            .add(
                egui::Label::new(
                    RichText::new(truncate_chars(desc, 110))
                        .size(12.3)
                        .color(meta_col),
                )
                .wrap()
                .sense(Sense::click()),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(&url);
        if d_resp.clicked() {
            open_external_url(&url);
        }
    }

    // 5. Clean Domain line (`docs.google.com`, `drive.google.com`, `tiktok.com`)
    ui.add_space(2.5);
    let dom_resp = ui
        .add(
            egui::Label::new(RichText::new(&domain).size(12.6).color(link_col))
                .sense(Sense::click()),
        )
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(&url);
    if dom_resp.clicked() {
        open_external_url(&url);
    }

    body_action
}

pub(super) fn is_zalo_file_or_video_message(msg: &ChatMessage) -> bool {
    let trimmed = msg.text.trim();
    matches!(
        msg.link_src.as_deref(),
        Some("Tập tin Zalo") | Some("Video Zalo")
    ) || trimmed.starts_with("[Tập tin]")
        || trimmed.starts_with("[Video]")
        || trimmed.starts_with("[Tin nhắn thoại]")
        || msg
            .link_url
            .as_deref()
            .is_some_and(|u| u.to_ascii_lowercase().contains("/renewmedia"))
}

pub(super) fn photo_primary_url(msg: &ChatMessage) -> Option<&str> {
    if let Some(th) = msg
        .link_thumb
        .as_deref()
        .filter(|u| !u.is_empty() && crate::chat::is_zalo_photo_cdn_url(u))
    {
        return Some(th);
    }
    if let Some(lu) = msg
        .link_url
        .as_deref()
        .filter(|u| !u.is_empty() && crate::chat::is_zalo_photo_cdn_url(u))
    {
        return Some(lu);
    }
    let t = msg.text.trim();
    if crate::chat::is_zalo_photo_cdn_url(t) {
        return Some(t);
    }
    if let Some(pos) = t.find("https://").or_else(|| t.find("http://")) {
        let after = &t[pos..];
        let end = after.find(char::is_whitespace).unwrap_or(after.len());
        let candidate =
            after[..end].trim_end_matches(|c: char| matches!(c, ')' | ']' | ',' | ';' | '>'));
        if crate::chat::is_zalo_photo_cdn_url(candidate) {
            return Some(candidate);
        }
    }
    None
}

pub(super) fn is_zalo_photo_message(msg: &ChatMessage) -> bool {
    if is_sticker_message(msg).is_some() || is_zalo_file_or_video_message(msg) {
        return false;
    }
    // If link_url points to a non-image webpage (e.g. YouTube, Google Docs, News), it is a rich link card, not a photo message.
    if let Some(ref lu) = msg.link_url {
        if !lu.is_empty() && !crate::chat::is_zalo_photo_cdn_url(lu) {
            return false;
        }
    }
    let trimmed = msg.text.trim();
    msg.link_src.as_deref() == Some("Hình ảnh Zalo")
        || trimmed == "[Hình ảnh]"
        || trimmed.starts_with("[Hình ảnh] ")
        || msg
            .link_url
            .as_deref()
            .is_some_and(crate::chat::is_zalo_photo_cdn_url)
        || msg
            .link_thumb
            .as_deref()
            .is_some_and(crate::chat::is_zalo_photo_cdn_url)
        || crate::chat::is_zalo_photo_cdn_url(trimmed)
        || photo_primary_url(msg).is_some()
}

pub(super) fn extract_photo_user_caption(msg: &ChatMessage) -> Option<&str> {
    let is_valid_caption = |s: &str| -> bool {
        let t = s.trim();
        if t.is_empty() || t == "[Hình ảnh]" || t == "[GIF]" {
            return false;
        }
        let lower = t.to_ascii_lowercase();
        if [
            ".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp", ".jfif", ".tiff",
        ]
        .iter()
        .any(|ext| lower.ends_with(ext))
        {
            return false;
        }
        !t.starts_with("http://")
            && !t.starts_with("https://")
            && !t.ends_with(".zdn.vn")
            && !t.ends_with(".zadn.vn")
            && !t.ends_with(".dlfl.vn")
            && !t.contains("photo-stal")
            && !t.contains("zpcloud")
            && !t.contains("zpg-r")
            && Some(t) != msg.link_src.as_deref()
    };
    if let Some(desc) = msg.link_desc.as_deref().map(str::trim) {
        if is_valid_caption(desc) {
            return Some(desc);
        }
    }
    let mut trimmed = msg.text.trim();
    if let Some(s) = trimmed.strip_prefix("[Đã thu hồi]") {
        trimmed = s.trim();
    }
    let stripped = trimmed
        .strip_prefix("[Hình ảnh]")
        .map(str::trim)
        .unwrap_or(trimmed);
    if is_valid_caption(stripped) {
        Some(stripped)
    } else {
        None
    }
}

/// Zalo PC `--shadow-bubble`: a 1px soft drop shadow instead of a border.
fn bubble_shadow(fill: Color32) -> egui::epaint::Shadow {
    if fill == Color32::TRANSPARENT {
        return egui::epaint::Shadow::NONE;
    }
    egui::epaint::Shadow {
        offset: Vec2::new(0.0, 1.0),
        blur: 2.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(3, 4, 4, 60),
    }
}

pub(super) fn render_chat_bubble(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    prev_same_sender: bool,
    next_same_sender: bool,
    is_latest_outgoing: bool,
    is_group: bool,
    thread_name: &str,
    sender_avatar: Option<&str>,
    pal: &ThemePalette,
    sticker_textures: &HashMap<String, TextureHandle>,
    loading_stickers: &mut HashSet<String>,
    emoji_textures: &HashMap<String, TextureHandle>,
    idx: usize,
    reactor_name: &dyn Fn(&str) -> String,
) -> Option<BubbleAction> {
    let avail_w = ui.available_width();
    let max_bubble_width = (avail_w * 0.72).clamp(140.0, 520.0);
    let max_text_width = (max_bubble_width - 24.0).max(90.0);
    let has_sticker = is_sticker_message(msg).is_some();
    let call_card_opt = extract_call_card_view(msg);
    let is_call_card = call_card_opt.is_some();
    let is_photo_card = !has_sticker && is_zalo_photo_message(msg);
    let is_file_card = !has_sticker && is_zalo_file_or_video_message(msg);
    let has_link_card =
        !has_sticker && !is_photo_card && !is_file_card && extract_message_link_view(msg).is_some();
    let has_interactive_card = is_call_card || has_link_card || is_photo_card || is_file_card;

    let show_timestamp = !msg.time.is_empty()
        && (!next_same_sender
            || is_latest_outgoing
            || has_interactive_card
            || !msg.reactions.is_empty()
            || msg.is_starred);

    let photo_caption = if is_photo_card {
        extract_photo_user_caption(msg)
    } else {
        None
    };
    let has_photo_caption = photo_caption.is_some();
    let is_frameless_photo = is_photo_card
        && !has_photo_caption
        && msg.reply_to_text.is_none()
        && (msg.is_outgoing || !is_group);

    let sender_is_name = !msg.sender.is_empty()
        && msg.sender != "0"
        && !msg.sender.chars().all(|c| c.is_ascii_digit());
    let show_group_sender = !msg.is_outgoing && !prev_same_sender && is_group && sender_is_name;

    // Pre-measure exact content width for both outgoing and incoming bubbles so every bubble,
    // photo, and call card hugs its content tightly without horizontal overflow or inner/outer frame mismatch.
    let content_width = if has_sticker {
        116.0_f32
    } else if is_call_card {
        max_text_width.min(256.0)
    } else if is_photo_card {
        let thumb_u = photo_primary_url(msg).unwrap_or("");
        let tkey = format!("thumb_{}", crate::crypto::md5_hex(thumb_u));
        let photo_w = if let Some(tex) = sticker_textures.get(&tkey) {
            let [tw, th] = tex.size();
            if tw <= 2 || th <= 2 {
                max_text_width.clamp(180.0, 260.0)
            } else {
                let tw_f = tw as f32;
                let th_f = th as f32;
                let max_w = max_text_width.clamp(220.0, 360.0);
                let max_h = 340.0_f32;
                let scale = (max_w / tw_f).min(max_h / th_f).min(1.0);
                (tw_f * scale).clamp(160.0, max_w).ceil()
            }
        } else {
            max_text_width.clamp(200.0, 280.0)
        };
        if let Some(cap_str) = photo_caption {
            let cap_w = ui.fonts(|f| {
                f.layout(
                    cap_str.to_string(),
                    FontId::proportional(15.2),
                    Color32::WHITE,
                    max_text_width,
                )
                .size()
                .x
            });
            photo_w.max(cap_w + 4.0).min(max_text_width).ceil()
        } else {
            photo_w
        }
    } else if is_file_card || has_link_card {
        let has_loaded_thumb = msg
            .link_thumb
            .as_deref()
            .filter(|u| !u.is_empty())
            .map(|u| sticker_textures.contains_key(&format!("thumb_{}", crate::crypto::md5_hex(u))))
            .unwrap_or(false);
        if has_loaded_thumb {
            max_text_width.min(260.0)
        } else {
            let (cap, url, dom) = extract_message_link_view(msg).unwrap_or_default();
            let title = msg
                .link_title
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    if !url.is_empty() {
                        &url
                    } else {
                        msg.text.trim()
                    }
                });
            let desc = msg.link_desc.as_deref().unwrap_or("");
            let measure = |s: &str, sz: f32| -> f32 {
                if s.is_empty() {
                    0.0
                } else {
                    ui.fonts(|f| {
                        f.layout(
                            truncate_chars(s, 110).into_owned(),
                            FontId::proportional(sz),
                            Color32::WHITE,
                            max_text_width.min(340.0),
                        )
                        .size()
                        .x
                    })
                }
            };
            let w = measure(title, 13.8)
                .max(measure(&cap, 13.8))
                .max(measure(desc, 12.3))
                .max(measure(&dom, 12.6))
                + 8.0;
            w.clamp(135.0, max_text_width.min(340.0)).ceil()
        }
    } else {
        let display_str = if msg.text.trim() == "[Tin nhắn đã thu hồi]" {
            "  Tin nhắn đã được thu hồi".to_string()
        } else if let Some(stripped) = msg.text.strip_prefix("[Đã thu hồi] ") {
            strip_markdown_syntax_for_width(stripped)
        } else {
            strip_markdown_syntax_for_width(&msg.text)
        };
        let emoji_clusters = if msg.text.starts_with("[Đã thu hồi]") {
            None
        } else {
            emoji_only_clusters(msg.text.trim())
        };
        let text_w = if let Some(clusters) = emoji_clusters {
            // Mirrors `render_emoji_only`: texture cells or glyph advances plus gaps.
            let gaps = (clusters.len().saturating_sub(1)) as f32 * EMOJI_ONLY_GAP;
            let cells: f32 = clusters
                .iter()
                .map(|e| emoji_only_cell_width(ui, e, sticker_textures, emoji_textures))
                .sum();
            cells + gaps
        } else {
            ui.fonts(|f| {
                f.layout(
                    display_str,
                    FontId::proportional(15.2),
                    Color32::WHITE,
                    max_text_width,
                )
                .size()
                .x
            })
        };
        let quote_w = match msg.reply_to_text.as_ref() {
            Some(qt) => {
                let q_body_w = ui.fonts(|f| {
                    f.layout(
                        truncate_chars(qt, 80).into_owned(),
                        FontId::proportional(12.0),
                        Color32::WHITE,
                        (max_text_width - 28.0).max(80.0),
                    )
                    .size()
                    .x
                });
                let who = msg
                    .reply_to_sender
                    .as_deref()
                    .filter(|sn| !sn.is_empty() && !sn.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or("Tin nhắn trích dẫn");
                let q_who_w = ui.fonts(|f| {
                    f.layout_no_wrap(who.to_string(), FontId::proportional(11.5), Color32::WHITE)
                        .size()
                        .x
                });
                q_body_w.max(q_who_w) + 30.0
            }
            None => 0.0_f32,
        };
        let sender_w = if show_group_sender {
            ui.fonts(|f| {
                f.layout_no_wrap(
                    truncate_chars(&msg.sender, 22).into_owned(),
                    FontId::proportional(12.0),
                    Color32::WHITE,
                )
                .size()
                .x
            }) + 8.0
        } else {
            0.0_f32
        };
        let marks_w = if !msg.reactions.is_empty() || msg.is_starred {
            78.0_f32
        } else {
            0.0_f32
        };
        let ts_min_w = if show_timestamp && is_latest_outgoing && msg.is_outgoing {
            96.0_f32
        } else {
            44.0_f32
        };
        text_w
            .max(quote_w)
            .max(sender_w)
            .max(marks_w)
            .max(ts_min_w)
            .min(max_text_width)
            .ceil()
            + 4.0
    };

    let h_margin = if has_sticker || is_frameless_photo {
        2.0_f32
    } else {
        12.0_f32
    };
    // Zalo PC bubble: 12px padding all round.
    let v_margin = if has_sticker || is_frameless_photo {
        2.0_f32
    } else {
        12.0_f32
    };
    let total_bubble_w = content_width + h_margin * 2.0;

    if msg.is_outgoing {
        let out_resp = ui.vertical(|ui| {
            let row_resp = ui.horizontal(|ui| {
                let left_pad = (ui.available_width() - total_bubble_w - 6.0).max(0.0);
                if left_pad > 0.0 {
                    ui.add_space(left_pad);
                }

                let fill = if has_sticker || is_frameless_photo {
                    Color32::TRANSPARENT
                } else if let Some(card) = &call_card_opt {
                    if card.is_missed_or_failed {
                        if pal.is_dark {
                            Color32::from_rgb(38, 23, 30)
                        } else {
                            Color32::from_rgb(254, 242, 242)
                        }
                    } else {
                        pal.bg_elevated
                    }
                } else {
                    pal.bubble_out_bg
                };
                let stroke = if has_sticker || is_frameless_photo {
                    Stroke::NONE
                } else if let Some(card) = &call_card_opt {
                    let border_col = if card.is_missed_or_failed {
                        Color32::from_rgb(239, 68, 68).linear_multiply(0.55)
                    } else {
                        Color32::from_rgb(34, 197, 94).linear_multiply(0.55)
                    };
                    Stroke::new(1.2_f32, border_col)
                } else {
                    Stroke::NONE
                };
                let rounding = Rounding::same(6.0);

                let mut card_action = None;
                let mut quote_action = None;
                let mut reaction_action = None;
                let fr = egui::Frame::none()
                    .fill(fill)
                    .stroke(stroke)
                    .rounding(rounding)
                    .shadow(bubble_shadow(fill))
                    .inner_margin(Margin::symmetric(h_margin, v_margin))
                    .show(ui, |ui| {
                        ui.set_min_width(content_width);
                        ui.set_max_width(content_width);
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            quote_action = render_quote_block(ui, msg, pal, true, content_width);
                            if let Some(card) = &call_card_opt {
                                card_action = render_call_card_widget(ui, card, pal, content_width);
                            } else if !render_sticker_widget(
                                ui,
                                msg,
                                sticker_textures,
                                loading_stickers,
                            ) {
                                card_action = render_message_body(
                                    ui,
                                    msg,
                                    thread_name,
                                    pal,
                                    true,
                                    content_width,
                                    sticker_textures,
                                    loading_stickers,
                                    emoji_textures,
                                );
                            }
                            if let Some(act) =
                                render_reactions(ui, msg, pal, emoji_textures, reactor_name)
                            {
                                reaction_action = Some(act);
                            }
                            if show_timestamp && !has_sticker && !is_frameless_photo {
                                ui.add_space(3.5);
                                let (status_str, status_col) = if is_latest_outgoing {
                                    match msg.delivery {
                                        crate::message_model::DeliveryState::Read => {
                                            (format!("{} · Đã xem", msg.time), pal.primary)
                                        }
                                        crate::message_model::DeliveryState::Pending => {
                                            (format!("{} · Đang gửi…", msg.time), pal.text_muted)
                                        }
                                        crate::message_model::DeliveryState::Failed => (
                                            format!("{} · Lỗi gửi", msg.time),
                                            Color32::from_rgb(239, 68, 68),
                                        ),
                                        _ => {
                                            (format!("{} · Đã nhận", msg.time), pal.bubble_out_meta)
                                        }
                                    }
                                } else {
                                    (msg.time.clone(), pal.bubble_out_meta)
                                };
                                ui.label(RichText::new(status_str).size(12.0).color(status_col));
                            }
                        });
                    });
                if fr.response.double_clicked() {
                    return Some(BubbleAction::Reply);
                }
                if let Some(act) = hover_action_bar(ui, &fr.response, msg, idx, pal) {
                    return Some(act);
                }
                reaction_action
                    .or(quote_action)
                    .or(card_action)
                    .or_else(|| {
                        bubble_context_menu(
                            ui,
                            &fr.response,
                            msg,
                            idx,
                            pal,
                            emoji_textures,
                            has_interactive_card,
                            "Bạn",
                        )
                    })
            });

            row_resp.inner
        });
        out_resp.inner
    } else {
        ui.horizontal_top(|ui| {
            let display_sender = if sender_is_name {
                &msg.sender
            } else {
                thread_name
            };

            // Sender Avatar (only on first message of a consecutive group)
            if !prev_same_sender {
                let (av_rect, _) = ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::hover());
                paint_user_avatar(
                    ui.painter(),
                    av_rect.center(),
                    20.0,
                    display_sender,
                    false,
                    sender_avatar,
                    sticker_textures,
                    loading_stickers,
                );
            } else {
                // `allocate_exact_size` above adds `item_spacing.x` after the avatar, but
                // `add_space` does not, so reserve both to keep every incoming bubble's left
                // edge on the same vertical line.
                ui.add_space(40.0 + ui.spacing().item_spacing.x);
            }

            let fill = if has_sticker || is_frameless_photo {
                Color32::TRANSPARENT
            } else if let Some(card) = &call_card_opt {
                if card.is_missed_or_failed {
                    if pal.is_dark {
                        Color32::from_rgb(38, 23, 30)
                    } else {
                        Color32::from_rgb(254, 242, 242)
                    }
                } else {
                    pal.bg_elevated
                }
            } else {
                pal.bubble_in_bg
            };
            let stroke = if has_sticker || is_frameless_photo {
                Stroke::NONE
            } else if let Some(card) = &call_card_opt {
                let border_col = if card.is_missed_or_failed {
                    Color32::from_rgb(239, 68, 68).linear_multiply(0.55)
                } else {
                    Color32::from_rgb(34, 197, 94).linear_multiply(0.55)
                };
                Stroke::new(1.2_f32, border_col)
            } else {
                Stroke::NONE
            };
            let rounding = Rounding::same(6.0);

            ui.vertical(|ui| {
                ui.set_max_width(total_bubble_w);
                let mut card_action = None;
                let mut quote_action = None;
                let mut reaction_action = None;
                let fr = egui::Frame::none()
                    .fill(fill)
                    .stroke(stroke)
                    .rounding(rounding)
                    .shadow(bubble_shadow(fill))
                    .inner_margin(Margin::symmetric(h_margin, v_margin))
                    .show(ui, |ui| {
                        ui.set_min_width(content_width);
                        ui.set_max_width(content_width);
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            if show_group_sender {
                                ui.label(
                                    RichText::new(truncate_chars(display_sender, 22))
                                        .color(pal.text_secondary)
                                        .size(13.0),
                                );
                                ui.add_space(4.0);
                            }
                            quote_action = render_quote_block(ui, msg, pal, false, content_width);
                            if let Some(card) = &call_card_opt {
                                card_action = render_call_card_widget(ui, card, pal, content_width);
                            } else if !render_sticker_widget(
                                ui,
                                msg,
                                sticker_textures,
                                loading_stickers,
                            ) {
                                card_action = render_message_body(
                                    ui,
                                    msg,
                                    thread_name,
                                    pal,
                                    false,
                                    content_width,
                                    sticker_textures,
                                    loading_stickers,
                                    emoji_textures,
                                );
                            }
                            if let Some(act) =
                                render_reactions(ui, msg, pal, emoji_textures, reactor_name)
                            {
                                reaction_action = Some(act);
                            }
                            if show_timestamp && !has_sticker && !is_frameless_photo {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(&msg.time)
                                        .color(pal.bubble_in_meta)
                                        .size(12.0),
                                );
                            }
                        });
                    });
                if fr.response.double_clicked() {
                    return Some(BubbleAction::Reply);
                }
                if let Some(act) = hover_action_bar(ui, &fr.response, msg, idx, pal) {
                    return Some(act);
                }
                reaction_action
                    .or(quote_action)
                    .or(card_action)
                    .or_else(|| {
                        bubble_context_menu(
                            ui,
                            &fr.response,
                            msg,
                            idx,
                            pal,
                            emoji_textures,
                            has_interactive_card,
                            display_sender,
                        )
                    })
            })
            .inner
        })
        .inner
    }
}

#[derive(Clone, PartialEq)]
pub(super) enum BubbleAction {
    Reply,
    #[allow(dead_code)]
    Mention(String),
    JumpToQuoted(String),
    Forward,
    /// Open the "who reacted" dialog for this message.
    ShowReactions,
    React(&'static str),
    TogglePin,
    ToggleStar,
    ScheduleReminder,
    DeleteForMe,
    RecallForEveryone,
    CallAgain {
        is_video: bool,
    },
    OpenImagePopup {
        tex_key: String,
        full_url: String,
        title: String,
        sender: String,
        time: String,
    },
    DownloadFile {
        url: String,
        file_name: String,
    },
}

fn menu_item(ui: &mut egui::Ui, icon: &str, label: &str, text_color: Color32) -> egui::Response {
    let text = RichText::new(format!("{icon}   {label}"))
        .size(13.0)
        .color(text_color);
    let btn = egui::Button::new(text)
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(ui.available_width(), 34.0));
    let response = ui.add(btn);
    response.surrender_focus();
    response
}

/// Diameter of one hover action button (Zalo PC: 28px circles beside the bubble).
const HOVER_BTN: f32 = 28.0;
const HOVER_GAP: f32 = 6.0;

/// Zalo-style actions shown while the pointer is over a bubble: Trả lời, Chia sẻ and "…"
/// (opens the compact message menu). Outgoing bubbles get the bar on their left, incoming on
/// their right; it is bottom-aligned with the bubble.
fn hover_action_bar(
    ui: &mut egui::Ui,
    bubble: &egui::Response,
    msg: &ChatMessage,
    idx: usize,
    pal: &ThemePalette,
) -> Option<BubbleAction> {
    let this_key = format!("{}_{}", msg.id, msg.ts);
    let bar_w = HOVER_BTN * 3.0 + HOVER_GAP * 2.0;
    let rect = bubble.rect;
    let bar_left = if msg.is_outgoing {
        rect.left() - 8.0 - bar_w
    } else {
        rect.right() + 8.0
    };
    let bar_top = (rect.bottom() - HOVER_BTN - 2.0).max(rect.top());
    let bar = Rect::from_min_size(Pos2::new(bar_left, bar_top), Vec2::new(bar_w, HOVER_BTN));

    // Keep the bar while this message's "…" menu is open.
    let menu_open_here = ui.ctx().data(|d| {
        d.get_temp::<(String, Pos2, usize)>(Id::new("active_bubble_context_menu"))
            .is_some_and(|(k, _, _)| k == this_key)
    });
    let pointer = ui.ctx().input(|i| i.pointer.hover_pos());
    let hovering = pointer.is_some_and(|p| {
        ui.clip_rect().contains(p)
            && ui.ctx().layer_id_at(p) == Some(ui.layer_id())
            && (rect.union(bar).expand2(Vec2::new(4.0, 2.0)).contains(p))
    });
    if !hovering && !menu_open_here {
        return None;
    }

    let mut action = None;
    let icons: [(&str, &str); 3] = [
        (lucide::QUOTE, "Trả lời"),
        (lucide::FORWARD, "Chia sẻ"),
        ("", "Thêm"),
    ];
    for (i, (icon, tip)) in icons.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(bar.left() + i as f32 * (HOVER_BTN + HOVER_GAP), bar.top()),
            Vec2::splat(HOVER_BTN),
        );
        let resp = ui
            .interact(
                r,
                Id::new(("bubble_hover_btn", &this_key, i)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(*tip);
        let active = resp.hovered() || (i == 2 && menu_open_here);
        let painter = ui.painter();
        painter.circle_filled(
            r.center(),
            HOVER_BTN / 2.0,
            if active {
                pal.bg_hover
            } else {
                pal.bg_elevated
            },
        );
        painter.circle_stroke(
            r.center(),
            HOVER_BTN / 2.0,
            Stroke::new(1.0_f32, pal.border),
        );
        let col = if active {
            pal.primary_hover
        } else {
            pal.text_secondary
        };
        if icon.is_empty() {
            for dx in [-5.0_f32, 0.0, 5.0] {
                painter.circle_filled(r.center() + Vec2::new(dx, 0.0), 1.6, col);
            }
        } else {
            painter.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                *icon,
                FontId::proportional(14.0),
                col,
            );
        }
        if i == 2 {
            // Let the menu's outside-click check ignore presses on this toggle button.
            ui.ctx()
                .data_mut(|d| d.insert_temp(Id::new(("bubble_more_btn", &this_key)), r));
        }
        if resp.clicked() {
            match i {
                0 => action = Some(BubbleAction::Reply),
                1 => action = Some(BubbleAction::Forward),
                _ => {
                    let now = ui.input(|inp| inp.time);
                    ui.ctx().data_mut(|d| {
                        if menu_open_here {
                            d.remove::<(String, Pos2, usize)>(Id::new(
                                "active_bubble_context_menu",
                            ));
                        } else {
                            d.insert_temp(
                                Id::new("active_bubble_context_menu"),
                                (this_key.clone(), r.left_bottom() + Vec2::new(0.0, 6.0), idx),
                            );
                            d.insert_temp(Id::new("bubble_context_menu_open_time"), now);
                            d.insert_temp(Id::new("bubble_context_menu_compact"), true);
                        }
                    });
                    ui.ctx().request_repaint();
                }
            }
        }
    }
    action
}

/// Shared look of the message menus (right-click, "…" and its flyout).
fn menu_frame(pal: &ThemePalette) -> egui::Frame {
    egui::Frame::none()
        .fill(if pal.is_dark {
            Color32::from_rgb(0x25, 0x28, 0x2D)
        } else {
            Color32::WHITE
        })
        .stroke(Stroke::new(1.0_f32, pal.border))
        .rounding(Rounding::same(8.0))
        .shadow(egui::epaint::Shadow {
            offset: Vec2::new(0.0, 4.0),
            blur: 16.0,
            spread: 0.0,
            color: Color32::from_black_alpha(if pal.is_dark { 130 } else { 45 }),
        })
        .inner_margin(Margin::symmetric(8.0, 8.0))
}

fn style_menu_ui(ui: &mut egui::Ui, pal: &ThemePalette) {
    ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
    ui.spacing_mut().button_padding = Vec2::new(12.0, 8.0);
    let w = &mut ui.style_mut().visuals.widgets;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.hovered.bg_stroke = Stroke::NONE;
    w.active.bg_stroke = Stroke::NONE;
    w.hovered.weak_bg_fill = pal.bg_hover;
}

/// Secondary message actions (reminder, open photo, save file). Shown inline in the right-click
/// menu and in the "Tuỳ chọn khác" flyout of the compact "…" menu.
fn message_extra_menu_items(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    pal: &ThemePalette,
    action: &mut Option<BubbleAction>,
    close_menu: &mut bool,
) {
    // 5b. Hẹn giờ / Nhắc hẹn cho tin nhắn này (#94)
    if menu_item(ui, lucide::BELL, "Hẹn giờ / Nhắc hẹn", pal.text_primary).clicked() {
        *action = Some(BubbleAction::ScheduleReminder);
        *close_menu = true;
    }

    // 6. Nếu là ảnh: Xem ảnh phóng to
    if is_zalo_photo_message(msg) {
        if let Some(thumb_url) = photo_primary_url(msg) {
            if menu_item(ui, lucide::SEARCH, "Xem ảnh phóng to", pal.text_primary).clicked() {
                *action = Some(BubbleAction::OpenImagePopup {
                    tex_key: format!("thumb_{}", crate::crypto::md5_hex(thumb_url)),
                    full_url: msg
                        .link_url
                        .clone()
                        .filter(|u| !u.is_empty())
                        .unwrap_or_else(|| thumb_url.to_string()),
                    title: msg
                        .link_title
                        .clone()
                        .filter(|t| {
                            !t.is_empty() && !t.starts_with("http://") && !t.starts_with("https://")
                        })
                        .unwrap_or_else(|| "Hình ảnh".to_string()),
                    sender: if msg.is_outgoing {
                        "Bạn".to_string()
                    } else {
                        msg.sender.clone()
                    },
                    time: msg.time.clone(),
                });
                *close_menu = true;
            }
        }
    }

    // 7. Nếu là file / video: Lưu tập tin
    if is_zalo_file_or_video_message(msg) {
        if let Some(url) = msg.link_url.as_deref().filter(|u| !u.is_empty()) {
            let resolved_name = crate::chat::resolve_attachment_file_name(
                msg.link_title.as_deref(),
                msg.text.trim(),
                url,
            );
            let label = if resolved_name.len() > 18 {
                format!("Lưu \"{}…\"", &resolved_name[..16])
            } else {
                format!("Lưu \"{resolved_name}\"")
            };
            if menu_item(ui, lucide::PAPERCLIP, &label, pal.text_primary).clicked() {
                *action = Some(BubbleAction::DownloadFile {
                    url: url.to_string(),
                    file_name: resolved_name,
                });
                *close_menu = true;
            }
        }
    }
}

/// Right-click menu on a chat bubble / row: quote/reply, forward/share, copy, pin, star, delete-for-me.
pub(super) fn bubble_context_menu(
    ui: &mut egui::Ui,
    bubble: &egui::Response,
    msg: &ChatMessage,
    idx: usize,
    pal: &ThemePalette,
    emoji_textures: &HashMap<String, TextureHandle>,
    _has_interactive_card: bool,
    _display_sender: &str,
) -> Option<BubbleAction> {
    let ctx_menu_key = Id::new("active_bubble_context_menu");
    let open_time_key = Id::new("bubble_context_menu_open_time");
    let this_key = format!("{}_{}", msg.id, msg.ts);

    let pointer_pos = ui
        .ctx()
        .input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));
    let in_bubble = pointer_pos.map_or(false, |p| bubble.rect.contains(p));

    let secondary_clicked = ui.ctx().input(|i| {
        i.pointer.button_clicked(egui::PointerButton::Secondary)
            || (i.pointer.button_clicked(egui::PointerButton::Primary)
                && (i.modifiers.ctrl || i.modifiers.command))
    });

    if in_bubble && secondary_clicked {
        let click_pos = pointer_pos.unwrap_or(bubble.rect.left_top());
        let current_time = ui.input(|i| i.time);
        ui.ctx().data_mut(|d| {
            d.insert_temp(ctx_menu_key, (this_key.clone(), click_pos, idx));
            d.insert_temp(open_time_key, current_time);
            d.insert_temp(Id::new("bubble_context_menu_compact"), false);
            d.remove::<String>(Id::new("active_hover_reaction_bubble"));
        });
        ui.ctx().request_repaint();
    }

    let active_menu: Option<(String, Pos2, usize)> = ui.ctx().data(|d| d.get_temp(ctx_menu_key));
    let is_this_active = active_menu
        .as_ref()
        .map_or(false, |(k, _, _)| k == &this_key);

    if !is_this_active {
        return None;
    }

    let (_, menu_pos, _) = active_menu.unwrap();
    // Compact = opened from the hover "…" button (Zalo layout); full = right-click.
    let compact: bool = ui
        .ctx()
        .data(|d| d.get_temp(Id::new("bubble_context_menu_compact")))
        .unwrap_or(false);
    let mut action = None;
    let mut close_menu = false;

    let screen = ui.ctx().screen_rect();
    let menu_w = if compact { 230.0_f32 } else { 220.0_f32 };
    let menu_h = if compact { 330.0_f32 } else { 440.0_f32 };
    let pos = Pos2::new(
        menu_pos.x.clamp(
            screen.left() + 8.0,
            (screen.right() - menu_w - 8.0).max(screen.left() + 8.0),
        ),
        menu_pos.y.clamp(
            screen.top() + 8.0,
            (screen.bottom() - menu_h - 8.0).max(screen.top() + 8.0),
        ),
    );

    egui::Area::new(Id::new(("active_ctx_menu_area", idx, msg.ts, &msg.id)))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            menu_frame(pal).show(ui, |ui| {
                ui.set_width(menu_w);
                style_menu_ui(ui, pal);

                // 0. Hàng emoji thả cảm xúc (giống Zalo gốc khi chuột phải)
                if !compact {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        for &(code, key, tooltip) in &REACTION_LIST {
                            let btn_resp = if let Some(tex) = emoji_textures.get(key) {
                                let img =
                                    egui::Image::new(tex).fit_to_exact_size(Vec2::splat(18.0));
                                ui.add(egui::ImageButton::new(img).rounding(Rounding::same(4.0)))
                            } else {
                                ui.add(
                                    egui::Button::new(RichText::new(code).size(15.0))
                                        .fill(Color32::TRANSPARENT)
                                        .rounding(Rounding::same(4.0)),
                                )
                            };
                            let clicked = btn_resp.clicked();
                            btn_resp.on_hover_text(tooltip);
                            if clicked {
                                action = Some(BubbleAction::React(code));
                                close_menu = true;
                            }
                        }
                    });
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // 1. Trả lời (Quote)
                    if menu_item(ui, lucide::QUOTE, "Trả lời", pal.text_primary).clicked() {
                        action = Some(BubbleAction::Reply);
                        close_menu = true;
                    }

                    // 2. Chia sẻ (Forward)
                    if menu_item(ui, lucide::FORWARD, "Chia sẻ", pal.text_primary).clicked() {
                        action = Some(BubbleAction::Forward);
                        close_menu = true;
                    }
                }

                // 3. Copy tin nhắn
                if menu_item(ui, lucide::COPY, "Copy tin nhắn", pal.text_primary).clicked() {
                    ui.ctx().copy_text(message_copy_text(msg));
                    close_menu = true;
                }
                if compact {
                    ui.separator();
                }

                // 4. Ghim tin nhắn
                let pin_label = if msg.is_pinned {
                    "Bỏ ghim tin nhắn"
                } else {
                    "Ghim tin nhắn"
                };
                if menu_item(ui, lucide::PIN, pin_label, pal.text_primary).clicked() {
                    action = Some(BubbleAction::TogglePin);
                    close_menu = true;
                }

                // 5. Đánh dấu tin nhắn
                let star_label = if msg.is_starred {
                    "Bỏ đánh dấu tin nhắn"
                } else {
                    "Đánh dấu tin nhắn"
                };
                if menu_item(ui, lucide::STAR, star_label, pal.text_primary).clicked() {
                    action = Some(BubbleAction::ToggleStar);
                    close_menu = true;
                }

                if !compact {
                    message_extra_menu_items(ui, msg, pal, &mut action, &mut close_menu);
                } else {
                    let more_key = Id::new(("bubble_menu_more", &this_key));
                    let row = ui.add(
                        egui::Button::new(
                            RichText::new(format!("{}   Tuỳ chọn khác", lucide::SLIDERS))
                                .size(13.0)
                                .color(pal.text_primary),
                        )
                        .stroke(Stroke::NONE)
                        .rounding(Rounding::same(7.0))
                        .min_size(Vec2::new(ui.available_width(), 34.0)),
                    );
                    ui.painter().text(
                        Pos2::new(row.rect.right() - 12.0, row.rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        lucide::CHEVRON_RIGHT,
                        FontId::proportional(13.0),
                        pal.text_secondary,
                    );
                    let flyout_prev: Option<Rect> = ui.ctx().data(|d| d.get_temp(more_key));
                    let ptr = ui.ctx().input(|i| i.pointer.hover_pos());
                    let keep = ptr.is_some_and(|p| {
                        row.rect.expand(2.0).contains(p)
                            || flyout_prev.is_some_and(|r| r.expand(6.0).contains(p))
                    });
                    if row.hovered() || row.clicked() || keep {
                        let area = egui::Area::new(Id::new(("bubble_menu_more_area", &this_key)))
                            .order(egui::Order::Foreground)
                            .fixed_pos(row.rect.right_top() + Vec2::new(14.0, -10.0))
                            .show(ui.ctx(), |ui| {
                                menu_frame(pal).show(ui, |ui| {
                                    ui.set_width(210.0);
                                    style_menu_ui(ui, pal);
                                    message_extra_menu_items(
                                        ui,
                                        msg,
                                        pal,
                                        &mut action,
                                        &mut close_menu,
                                    );
                                });
                            });
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(more_key, area.response.rect));
                    } else {
                        ui.ctx().data_mut(|d| d.remove::<Rect>(more_key));
                    }
                }

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 8. Thu hồi tin nhắn (với mọi người)
                if msg.is_outgoing
                    && msg.text.trim() != "[Tin nhắn đã thu hồi]"
                    && !msg.text.starts_with("[Đã thu hồi]")
                {
                    if menu_item(ui, lucide::BAN, "Thu hồi", Color32::from_rgb(249, 115, 22))
                        .clicked()
                    {
                        action = Some(BubbleAction::RecallForEveryone);
                        close_menu = true;
                    }
                }

                // 9. Xóa chỉ ở phía tôi
                if menu_item(
                    ui,
                    lucide::TRASH_2,
                    "Xóa chỉ ở phía tôi",
                    Color32::from_rgb(239, 68, 68),
                )
                .clicked()
                {
                    action = Some(BubbleAction::DeleteForMe);
                    close_menu = true;
                }

                let menu_rect = ui.min_rect();
                let pointer_pos = ui
                    .ctx()
                    .input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));
                let open_time: f64 = ui.ctx().data(|d| d.get_temp(open_time_key)).unwrap_or(0.0);
                if ui.input(|i| i.time) > open_time + 0.05 {
                    let pressed = ui.ctx().input(|i| i.pointer.any_pressed());
                    if pressed {
                        let flyout: Option<Rect> = ui
                            .ctx()
                            .data(|d| d.get_temp(Id::new(("bubble_menu_more", &this_key))));
                        if let Some(pos) = pointer_pos {
                            let more_btn: Option<Rect> = ui
                                .ctx()
                                .data(|d| d.get_temp(Id::new(("bubble_more_btn", &this_key))));
                            if !menu_rect.expand(4.0).contains(pos)
                                && !flyout.is_some_and(|r| r.expand(4.0).contains(pos))
                                && !more_btn.is_some_and(|r| r.contains(pos))
                            {
                                close_menu = true;
                            }
                        }
                    }
                }
                if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                    close_menu = true;
                }
            });
        });

    if close_menu {
        ui.ctx()
            .data_mut(|d| d.remove::<(String, Pos2, usize)>(ctx_menu_key));
        ui.ctx().request_repaint();
    }

    action
}

/// Quoted-message block shown at the top of a reply bubble.
pub(super) fn render_quote_block(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    pal: &ThemePalette,
    outgoing: bool,
    max_w: f32,
) -> Option<BubbleAction> {
    let qt = msg.reply_to_text.as_ref()?;
    let (text_col, meta_col) = if outgoing {
        (pal.bubble_out_text, pal.bubble_out_meta)
    } else {
        (pal.bubble_in_text, pal.bubble_in_meta)
    };
    let bar_col = if outgoing && pal.is_dark {
        Color32::from_rgb(147, 197, 253)
    } else {
        pal.primary
    };
    let bg_col = if outgoing && pal.is_dark {
        Color32::from_rgba_unmultiplied(0, 0, 0, 42)
    } else if outgoing {
        Color32::from_rgba_unmultiplied(255, 255, 255, 150)
    } else {
        text_col.linear_multiply(0.06)
    };
    let inner_col_w = (max_w - 28.0).max(72.0);
    let frame = egui::Frame::none()
        .fill(bg_col)
        .rounding(Rounding::same(7.0))
        .inner_margin(Margin::symmetric(8.0, 5.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 28.0), Sense::hover());
                ui.painter()
                    .rect_filled(bar_rect, Rounding::same(1.5), bar_col);
                ui.vertical(|ui| {
                    ui.set_width(inner_col_w);
                    let who = msg
                        .reply_to_sender
                        .as_deref()
                        .filter(|sn| !sn.is_empty() && !sn.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or("Tin nhắn trích dẫn");
                    ui.label(RichText::new(who).color(bar_col).size(11.5).strong());
                    ui.add(
                        egui::Label::new(
                            RichText::new(truncate_chars(qt, 84))
                                .color(meta_col)
                                .size(12.0),
                        )
                        .wrap(),
                    );
                });
            });
        });
    let reference = crate::message_navigation::message_reply_reference(msg);
    let response = ui.interact(
        frame.response.rect,
        frame.response.id.with("quote_jump"),
        Sense::click(),
    );
    let clicked = response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Đi tới tin nhắn gốc đã tải")
        .clicked();
    ui.add_space(5.0);
    clicked.then(|| BubbleAction::JumpToQuoted(reference.unwrap_or_default()))
}

#[allow(dead_code)]
pub(super) fn render_hover_reaction_bar(
    _ui: &mut egui::Ui,
    _bubble_rect: Rect,
    _msg: &ChatMessage,
    _idx: usize,
    _outgoing: bool,
    _pal: &ThemePalette,
    _emoji_textures: &HashMap<String, TextureHandle>,
) -> Option<BubbleAction> {
    None
}

/// Names listed in the reaction-pill hover tooltip before collapsing into "và N người khác".
const REACTION_TOOLTIP_MAX_NAMES: usize = 8;

/// Lines for the reaction-pill hover tooltip (Zalo PC style: who reacted, one name per line).
///
/// Reactors come only from the ledger (`reactor_ids`), deduplicated across emoji, in order of the
/// most used emoji first. Any count no known reactor explains is folded into "người khác".
pub(super) fn reaction_tooltip_lines(
    summaries: &[crate::reactions::ReactionSummary],
    total: u64,
    reactor_name: &dyn Fn(&str) -> String,
    max_names: usize,
) -> Vec<String> {
    let mut ordered: Vec<&crate::reactions::ReactionSummary> =
        summaries.iter().filter(|s| s.count > 0).collect();
    ordered.sort_by_key(|s| std::cmp::Reverse(s.count));
    let mut ids: Vec<&str> = Vec::new();
    let mut explained = 0u64;
    for s in &ordered {
        for id in &s.reactor_ids {
            explained += s.reactor_counts.get(id).copied().unwrap_or(1);
            if !ids.contains(&id.as_str()) {
                ids.push(id);
            }
        }
    }
    if ids.is_empty() {
        return vec![format!("{total} lượt thả cảm xúc")];
    }
    let mut lines: Vec<String> = ids
        .iter()
        .take(max_names)
        .map(|id| reactor_name(id))
        .collect();
    let hidden_people = ids.len().saturating_sub(max_names) as u64;
    let unexplained = total.saturating_sub(explained);
    let others = hidden_people + unexplained;
    if others > 0 {
        lines.push(format!("và {others} người khác"));
    }
    lines
}

/// Zalo-style tooltip above the reaction pill: solid blue card with a downward arrow.
fn paint_reaction_tooltip(ctx: &egui::Context, anchor: Rect, lines: &[String], pal: &ThemePalette) {
    if lines.is_empty() {
        return;
    }
    const ARROW: f32 = 6.0;
    let fill = if pal.is_dark {
        Color32::from_rgb(0x1A, 0x3F, 0x94)
    } else {
        Color32::from_rgb(0x00, 0x52, 0xCC)
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::new("reaction_pill_tooltip"),
    ));
    let galley = painter.layout(
        lines.join("\n"),
        FontId::proportional(14.0),
        Color32::WHITE,
        f32::INFINITY,
    );
    let padding = Vec2::new(14.0, 9.0);
    let size = galley.size() + padding * 2.0;
    let screen = ctx.screen_rect();
    let mut min = Pos2::new(
        anchor.center().x - size.x / 2.0,
        anchor.top() - ARROW - 2.0 - size.y,
    );
    min.x = min.x.clamp(
        screen.left() + 4.0,
        (screen.right() - size.x - 4.0).max(screen.left() + 4.0),
    );
    min.y = min.y.max(screen.top() + 4.0);
    let card = Rect::from_min_size(min, size);
    painter.rect_filled(card, Rounding::same(6.0), fill);
    let tip_x = anchor
        .center()
        .x
        .clamp(card.left() + 10.0, card.right() - 10.0);
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(tip_x - ARROW, card.bottom()),
            Pos2::new(tip_x + ARROW, card.bottom()),
            Pos2::new(tip_x, card.bottom() + ARROW),
        ],
        fill,
        Stroke::NONE,
    ));
    painter.galley(card.min + padding, galley, Color32::WHITE);
}

pub(super) fn render_reactions(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    pal: &ThemePalette,
    emoji_textures: &HashMap<String, TextureHandle>,
    reactor_name: &dyn Fn(&str) -> String,
) -> Option<BubbleAction> {
    // Pin badge display parked until server-side pinning is fully integrated (avoid showing mock data).
    let has_pin = false;
    let has_star = msg.is_starred;
    let has_react = !msg.reactions.is_empty() || !msg.reaction_summaries.is_empty();

    if !has_pin && !has_star && !has_react {
        return None;
    }

    // Clicking the summary pill opens the "who reacted" dialog.
    let mut action: Option<BubbleAction> = None;
    ui.add_space(3.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);

        if has_star {
            egui::Frame::none()
                .fill(Color32::from_rgb(234, 179, 8).linear_multiply(0.2))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(234, 179, 8)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::symmetric(5.0, 2.0))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(lucide::STAR)
                                .size(11.0)
                                .color(Color32::from_rgb(234, 179, 8))
                                .strong(),
                        )
                        .selectable(false)
                        .wrap_mode(egui::TextWrapMode::Extend),
                    )
                    .on_hover_text("Tin nhắn đã đánh dấu");
                });
        }

        if has_react {
            let mut items: Vec<(String, u64)> = Vec::new();
            if !msg.reaction_summaries.is_empty() {
                for s in &msg.reaction_summaries {
                    if s.count > 0 {
                        if let Some(pos) = items.iter().position(|(e, _)| e == &s.emoji) {
                            items[pos].1 = items[pos].1.saturating_add(s.count);
                        } else {
                            items.push((s.emoji.clone(), s.count));
                        }
                    }
                }
            } else {
                for r in &msg.reactions {
                    if let Some(pos) = items.iter().position(|(e, _)| e == r) {
                        items[pos].1 = items[pos].1.saturating_add(1);
                    } else {
                        items.push((r.clone(), 1));
                    }
                }
            }
            // Most used first, so the pill shows the dominant icons.
            items.sort_by(|a, b| b.1.cmp(&a.1));
            let total: u64 = items.iter().map(|(_, c)| *c).sum();

            if total > 0 {
                // One compact pill like Zalo: up to 3 icons + the total. Separate pills per icon
                // grew wider than a sticker bubble and were clipped at the window edge.
                let fr_resp = egui::Frame::none()
                    .fill(pal.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(11.0))
                    .inner_margin(Margin::symmetric(6.0, 2.5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                            for (emoji, _) in items.iter().take(3) {
                                let key = emoji_to_tex_key(emoji);
                                if let Some(tex) = emoji_textures.get(key) {
                                    ui.add(
                                        egui::Image::new(tex).fit_to_exact_size(Vec2::splat(16.0)),
                                    );
                                } else {
                                    ui.label(RichText::new(emoji).size(13.0));
                                }
                            }
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(total.to_string())
                                    .size(11.5)
                                    .color(pal.text_primary)
                                    .strong(),
                            );
                        });
                    })
                    .response;
                let pill_resp = ui.interact(
                    fr_resp.rect,
                    Id::new(("react_pill", &msg.id, msg.ts)),
                    Sense::click(),
                );
                if pill_resp.clicked() {
                    action = Some(BubbleAction::ShowReactions);
                }
                let pill_resp = pill_resp.on_hover_cursor(CursorIcon::PointingHand);
                if pill_resp.hovered() {
                    let lines = reaction_tooltip_lines(
                        &msg.reaction_summaries,
                        total,
                        reactor_name,
                        REACTION_TOOLTIP_MAX_NAMES,
                    );
                    paint_reaction_tooltip(ui.ctx(), pill_resp.rect, &lines, pal);
                }
            }
        }
    });

    action
}

#[cfg(test)]
mod quote_navigation_tests {
    use super::*;

    #[test]
    fn quote_navigation_bubble_propagates_action_for_both_directions() {
        for is_outgoing in [false, true] {
            let ctx = egui::Context::default();
            let msg = ChatMessage {
                id: "reply-1".into(),
                text: "Reply body".into(),
                sender: "Sender".into(),
                reply_to_id: Some("original-1".into()),
                reply_to_text: Some("Original text".into()),
                is_outgoing,
                ..Default::default()
            };
            let mut position = egui::pos2(30.0, 30.0);
            let mut action = None;
            for frame in 0..4 {
                let mut events = vec![egui::Event::PointerMoved(position)];
                if frame >= 2 {
                    events.push(egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 2,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(600.0, 400.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            action = render_chat_bubble(
                                ui,
                                &msg,
                                false,
                                false,
                                false,
                                false,
                                "Thread",
                                None,
                                &ThemePalette::light(),
                                &HashMap::new(),
                                &mut HashSet::new(),
                                &HashMap::new(),
                                0,
                                &|id: &str| id.to_string(),
                            );
                        });
                    },
                );
                if frame == 0 {
                    position = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.job.text == "Original text" => {
                                Some(text.pos + Vec2::new(4.0, 4.0))
                            }
                            _ => None,
                        })
                        .expect("Quote must be rendered in the bubble");
                }
            }
            assert!(
                matches!(action, Some(BubbleAction::JumpToQuoted(reference)) if reference == "original-1")
            );
        }
    }

    #[test]
    fn quote_navigation_emits_exact_reference_when_clicked() {
        let ctx = egui::Context::default();
        let msg = ChatMessage {
            reply_to_id: Some("00123".into()),
            reply_to_text: Some("Original text".into()),
            ..Default::default()
        };
        let mut action = None;
        for pressed in [None, Some(true), Some(false)] {
            let mut events = vec![egui::Event::PointerMoved(egui::pos2(30.0, 30.0))];
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton {
                    pos: egui::pos2(30.0, 30.0),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        action = render_quote_block(ui, &msg, &ThemePalette::light(), false, 300.0);
                    });
                },
            );
        }
        assert!(
            matches!(action, Some(BubbleAction::JumpToQuoted(reference)) if reference == "00123")
        );
    }

    #[test]
    fn quote_navigation_advertises_click_when_reference_exists() {
        let ctx = egui::Context::default();
        let msg = ChatMessage {
            reply_to_id: Some("original-1".into()),
            reply_to_text: Some("Original text".into()),
            ..Default::default()
        };
        for _ in 0..2 {
            let output = ctx.run(
                egui::RawInput {
                    events: vec![egui::Event::PointerMoved(egui::pos2(30.0, 30.0))],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        render_quote_block(ui, &msg, &ThemePalette::light(), false, 300.0);
                    });
                },
            );
            if output.platform_output.cursor_icon == egui::CursorIcon::PointingHand {
                return;
            }
        }
        panic!("Quote with an exact reference must advertise a clickable target");
    }

    #[test]
    fn bubble_context_menu_opens_on_secondary_click_and_emits_reply_action() {
        let ctx = egui::Context::default();
        let msg = ChatMessage {
            id: "msg-ctx-test".into(),
            text: "Hello right click test".into(),
            sender: "Tester".into(),
            is_outgoing: false,
            ts: 123456789,
            ..Default::default()
        };

        let bubble_rect = Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(120.0, 40.0));
        let click_pos = Pos2::new(60.0, 60.0);
        let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));

        // Frame 1: Pointer moved to bubble
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(click_pos)],
                screen_rect: screen,
                time: Some(1.0),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let bubble_resp =
                        ui.interact(bubble_rect, Id::new("dummy_bubble"), Sense::hover());
                    let _ = bubble_context_menu(
                        ui,
                        &bubble_resp,
                        &msg,
                        0,
                        &ThemePalette::light(),
                        &HashMap::new(),
                        false,
                        "Tester",
                    );
                });
            },
        );

        // Frame 2: Secondary button pressed
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::PointerButton {
                    pos: click_pos,
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
                screen_rect: screen,
                time: Some(1.05),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let bubble_resp =
                        ui.interact(bubble_rect, Id::new("dummy_bubble"), Sense::hover());
                    let _ = bubble_context_menu(
                        ui,
                        &bubble_resp,
                        &msg,
                        0,
                        &ThemePalette::light(),
                        &HashMap::new(),
                        false,
                        "Tester",
                    );
                });
            },
        );

        // Frame 3: Secondary button released -> Context menu is created in ctx.data
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::PointerButton {
                    pos: click_pos,
                    button: egui::PointerButton::Secondary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                screen_rect: screen,
                time: Some(1.1),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let bubble_resp =
                        ui.interact(bubble_rect, Id::new("dummy_bubble"), Sense::hover());
                    let _ = bubble_context_menu(
                        ui,
                        &bubble_resp,
                        &msg,
                        0,
                        &ThemePalette::light(),
                        &HashMap::new(),
                        false,
                        "Tester",
                    );
                });
            },
        );

        // Frame 4: Next frame where Area shapes are painted
        let output = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(1.15),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let bubble_resp =
                        ui.interact(bubble_rect, Id::new("dummy_bubble"), Sense::hover());
                    let _ = bubble_context_menu(
                        ui,
                        &bubble_resp,
                        &msg,
                        0,
                        &ThemePalette::light(),
                        &HashMap::new(),
                        false,
                        "Tester",
                    );
                });
            },
        );

        // Find "Trả lời" menu item position from output shapes
        let mut reply_pos = None;
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                if text.galley.job.text.contains("Trả lời") {
                    reply_pos = Some(text.pos + Vec2::new(10.0, 5.0));
                    break;
                }
            }
        }
        let reply_pos = reply_pos.expect("Context menu should be open with 'Trả lời' text");

        // Frame 5: Move mouse to "Trả lời" and press Primary
        let _ = ctx.run(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(reply_pos),
                    egui::Event::PointerButton {
                        pos: reply_pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                screen_rect: screen,
                time: Some(1.2),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let bubble_resp =
                        ui.interact(bubble_rect, Id::new("dummy_bubble"), Sense::hover());
                    let _ = bubble_context_menu(
                        ui,
                        &bubble_resp,
                        &msg,
                        0,
                        &ThemePalette::light(),
                        &HashMap::new(),
                        false,
                        "Tester",
                    );
                });
            },
        );

        // Frame 6: Release Primary -> Reply action triggered!
        let mut action = None;
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::PointerButton {
                    pos: reply_pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                screen_rect: screen,
                time: Some(1.25),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let bubble_resp =
                        ui.interact(bubble_rect, Id::new("dummy_bubble"), Sense::hover());
                    action = bubble_context_menu(
                        ui,
                        &bubble_resp,
                        &msg,
                        0,
                        &ThemePalette::light(),
                        &HashMap::new(),
                        false,
                        "Tester",
                    );
                });
            },
        );

        assert!(matches!(action, Some(BubbleAction::Reply)));
    }

    #[test]
    fn extract_photo_user_caption_filters_raw_image_filenames() {
        let raw_filename_msg = ChatMessage {
            text: "[Hình ảnh] msedge_uuRZ2mxGXV.png".into(),
            ..Default::default()
        };
        assert_eq!(extract_photo_user_caption(&raw_filename_msg), None);

        let clipboard_msg = ChatMessage {
            text: "[Hình ảnh] clipboard_1730932200000.png".into(),
            ..Default::default()
        };
        assert_eq!(extract_photo_user_caption(&clipboard_msg), None);

        let real_caption_msg = ChatMessage {
            text: "[Hình ảnh] Xem ảnh này nè".into(),
            ..Default::default()
        };
        assert_eq!(
            extract_photo_user_caption(&real_caption_msg),
            Some("Xem ảnh này nè")
        );
    }
    #[test]
    fn emoji_only_clusters_detects_pure_emoji_messages() {
        assert_eq!(emoji_only_clusters("🐧").unwrap(), vec!["🐧"]);
        assert_eq!(emoji_only_clusters(" 🐱 😀 ").unwrap(), vec!["🐱", "😀"]);
        // ZWJ sequence, skin tone, variation selector and flag stay single clusters.
        assert_eq!(emoji_only_clusters("👨‍👩‍👧").unwrap().len(), 1);
        assert_eq!(emoji_only_clusters("👍🏽").unwrap().len(), 1);
        assert_eq!(emoji_only_clusters("❤️").unwrap().len(), 1);
        assert_eq!(emoji_only_clusters("🇻🇳").unwrap().len(), 1);
        assert!(emoji_only_clusters("hi 🐧").is_none());
        assert!(emoji_only_clusters("123").is_none());
        assert!(emoji_only_clusters("").is_none());
        assert!(emoji_only_clusters(&"😀".repeat(EMOJI_ONLY_MAX + 1)).is_none());
        assert_eq!(emoji_texture_key_exact("❤️"), Some("heart"));
        assert_eq!(emoji_texture_key_exact("🐧"), None);
    }

    #[test]
    fn split_emoji_sections_replaces_ready_emoji_with_transparent_placeholders() {
        let ctx = egui::Context::default();
        let tex = ctx.load_texture("t", egui::ColorImage::example(), Default::default());
        let family = "👨\u{200D}👩\u{200D}👧";
        let mut textures = HashMap::new();
        textures.insert(crate::emoji_raster::texture_key("🐧"), tex.clone());
        textures.insert(crate::emoji_raster::texture_key(family), tex);
        let mut loading = HashSet::new();

        let mut job = egui::text::LayoutJob::default();
        job.append(
            &format!("hi 🐧 {family} © 💌"),
            0.0,
            egui::TextFormat::default(),
        );
        let (out, marks) = split_emoji_sections(job, &textures, &mut loading);

        // 🐧 and the family (collapsed to its first codepoint) are placeholders; © stays text;
        // 💌 has no texture yet so it stays a glyph and is requested from the worker.
        assert_eq!(out.text, "hi 🐧 👨 © 💌");
        assert_eq!(marks.len(), 2);
        for (idx, _) in &marks {
            assert_eq!(
                out.sections[*idx as usize].format.color,
                Color32::TRANSPARENT
            );
        }
        assert!(loading.contains(&crate::emoji_raster::texture_key("💌")));
        assert!(!is_inline_emoji_cluster("©"));
        assert!(is_inline_emoji_cluster("❤\u{FE0F}"));
    }

    #[test]
    fn message_mention_texts_maps_utf16_spans_from_wire_metadata() {
        let mut msg = ChatMessage {
            text: "😀 @Nguyễn Hoàng Hải Đăng ơi, @All họp".into(),
            ..Default::default()
        };
        assert_eq!(message_mention_texts(&msg), None, "no metadata → heuristic");

        let all_pos = msg.text[..msg.text.find("@All").unwrap()]
            .encode_utf16()
            .count();
        msg.wire_ref.fields.insert(
            "mentions".into(),
            serde_json::json!([
                {"pos": 3, "len": 22, "uid": "111", "type": 0},
                {"pos": all_pos.to_string(), "len": 4, "uid": "-1", "type": 1},
                {"pos": 999, "len": 4, "uid": "stale"}
            ]),
        );
        assert_eq!(
            message_mention_texts(&msg).unwrap(),
            vec!["@Nguyễn Hoàng Hải Đăng".to_string(), "@All".to_string()]
        );

        msg.wire_ref
            .fields
            .insert("mentions".into(), serde_json::json!([]));
        assert_eq!(message_mention_texts(&msg), Some(Vec::new()));
    }

    #[test]
    fn mention_start_requires_word_boundary() {
        assert_eq!(find_mention_start("hi @An"), Some(3));
        assert_eq!(find_mention_start("@An"), Some(0));
        assert_eq!(find_mention_start("mail@x.vn"), None);
        assert_eq!(find_mention_start("mail@x.vn (@Bo)"), Some(11));
    }

    #[test]
    fn bare_url_detection_handles_scheme_less_and_trailing_punctuation() {
        let s = "xem docs.google.com/document/d/abc, nhé";
        let start = find_bare_url_start(s).unwrap();
        let len = bare_url_len(&s[start..]);
        assert_eq!(&s[start..start + len], "docs.google.com/document/d/abc");
        assert_eq!(
            normalize_bare_url("docs.google.com/x"),
            "https://docs.google.com/x"
        );
        // `www.` inside an `https://` URL must not be reported as a second start.
        assert_eq!(find_bare_url_start("https://www.a.vn"), Some(0));
        assert!(find_bare_url_start("abcwww.x").is_none());
    }

    #[test]
    fn copy_text_prefers_original_url_for_link_shares() {
        let share = ChatMessage {
            text: "Tài liệu họp\nTóm tắt nội dung tài liệu".into(),
            link_url: Some("https://docs.google.com/document/d/abc/edit".into()),
            link_title: Some("Tài liệu họp".into()),
            link_desc: Some("Tóm tắt nội dung tài liệu".into()),
            ..Default::default()
        };
        assert_eq!(
            message_copy_text(&share),
            "https://docs.google.com/document/d/abc/edit"
        );

        let plain = ChatMessage {
            text: "xem https://example.com nhé".into(),
            ..Default::default()
        };
        assert_eq!(message_copy_text(&plain), "xem https://example.com nhé");
    }
}

#[cfg(test)]
mod reaction_tooltip_tests {
    use super::*;
    use crate::reactions::ReactionSummary;

    fn summary(emoji: &str, count: u64, ids: &[&str]) -> ReactionSummary {
        ReactionSummary {
            emoji: emoji.into(),
            count,
            reactor_ids: ids.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn name(id: &str) -> String {
        format!("N{id}")
    }

    #[test]
    fn lists_names_dedup_and_folds_unknown_count() {
        let s = vec![summary("👍", 1, &["2"]), summary("❤️", 3, &["1", "2"])];
        // Most used emoji first; "2" appears once; 4 total - 3 explained = 1 unknown.
        assert_eq!(
            reaction_tooltip_lines(&s, 4, &name, 8),
            vec!["N1", "N2", "và 1 người khác"]
        );
    }

    #[test]
    fn truncates_long_lists_and_handles_no_ledger() {
        let s = vec![summary("❤️", 3, &["1", "2", "3"])];
        assert_eq!(
            reaction_tooltip_lines(&s, 3, &name, 2),
            vec!["N1", "N2", "và 1 người khác"]
        );
        let anon = vec![summary("❤️", 5, &[])];
        assert_eq!(
            reaction_tooltip_lines(&anon, 5, &name, 8),
            vec!["5 lượt thả cảm xúc"]
        );
    }
}
