//! Video-tool setup controls; returns actions for the app to send to its worker.

use super::*;

pub(super) enum Action {
    Install,
    Refresh,
    Cancel,
}

pub(super) fn render(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    status: &str,
    progress: Option<f32>,
    available: bool,
) -> Option<Action> {
    let mut action = None;
    ui.label(
        RichText::new("Công cụ gọi video")
            .color(pal.text_primary)
            .size(13.5)
            .strong(),
    );
    ui.label(RichText::new(status).color(pal.text_secondary).size(12.0));
    if let Some(progress) = progress {
        ui.add(egui::ProgressBar::new(progress.clamp(0.0, 1.0)).animate(true));
        if ui.button("Dừng cài đặt").clicked() {
            action = Some(Action::Cancel);
        }
    } else {
        if !available {
            ui.label(
                RichText::new(if cfg!(target_os = "macos") {
                    "Tải FFmpeg đã xác minh vào thư mục dữ liệu của app; không cần cài Homebrew."
                } else {
                    "Cài FFmpeg qua trình quản lý gói của hệ điều hành."
                })
                .color(pal.text_secondary)
                .size(12.0),
            );
        }
        ui.horizontal_wrapped(|ui| {
            if !available && ui.button("Cài công cụ gọi video").clicked() {
                action = Some(Action::Install);
            }
            if ui.small_button("Kiểm tra lại").clicked() {
                action = Some(Action::Refresh);
            }
        });
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_controls_fit_narrow_settings_in_both_themes_and_all_states() {
        for pal in [ThemePalette::light(), ThemePalette::dark()] {
            for width in [280.0, 640.0] {
                for (progress, ready) in [(None, false), (Some(0.42), false), (None, true)] {
                    let ctx = egui::Context::default();
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(width, 400.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                render(
                                    ui,
                                    &pal,
                                    "Đang kiểm tra công cụ gọi video và kết nối đến nguồn tải…",
                                    progress,
                                    ready,
                                );
                                assert!(
                                    ui.min_rect().width() <= width,
                                    "controls must wrap inside the viewport"
                                );
                            });
                        },
                    );
                    assert!(!output.shapes.is_empty());
                }
            }
        }
    }
}
