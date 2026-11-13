//! Voice controls; the microphone and encoder stay on the native capture thread.
use super::*;
static NEXT_REQUEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
#[derive(Default)]
pub(super) struct State {
    pub generation: u64,
    pub account: String,
    pub busy: bool,
    pub recording: bool,
    pub seconds: u64,
    pub peak: f32,
    target: String,
}
impl ZaloGuiApp {
    pub(super) fn render_voice_controls(&mut self, ui: &mut egui::Ui) {
        if self.voice.account != self.user_uin {
            self.voice = State::default();
        }
        if self.voice.busy {
            ui.label(format!(
                "Ghi âm {}s · {}",
                self.voice.seconds, self.voice.target
            ));
            ui.add(egui::ProgressBar::new(self.voice.peak.clamp(0.0, 1.0)).desired_width(70.0));
            if self.voice.recording && ui.button("Dừng và gửi").clicked() {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::VoiceStop {
                        generation: self.voice.generation,
                        cancel: false,
                    });
                }
                self.voice.recording = false;
            }
            if ui.button("Hủy ghi âm").clicked() {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::VoiceStop {
                        generation: self.voice.generation,
                        cancel: true,
                    });
                }
            }
        } else if ui
            .button("Ghi âm")
            .on_hover_text("Thu tối đa 5 phút; chỉ gửi khi bấm Dừng và gửi")
            .clicked()
        {
            if self.active_call.is_some() {
                self.show_toast("Kết thúc cuộc gọi trước khi ghi âm tin nhắn.");
                return;
            }
            if !crate::video_tools::available() {
                self.install_video_tools();
                self.show_toast("Đang chuẩn bị FFmpeg. Khi hoàn tất, bấm Ghi âm để thu.");
                return;
            }
            if let Some(thread) = self.threads.get(self.selected_thread_index) {
                self.voice.generation =
                    NEXT_REQUEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.voice.account = self.user_uin.clone();
                self.voice.busy = true;
                self.voice.target = thread.name.clone();
                if let Some(tx) = WORKER_TX.get() {
                    if tx
                        .send(WorkerCmd::VoiceStart {
                            account: self.user_uin.clone(),
                            generation: self.voice.generation,
                            thread: thread.id.clone(),
                            is_group: thread.is_group,
                        })
                        .is_err()
                    {
                        self.voice.busy = false;
                    }
                } else {
                    self.voice.busy = false;
                    self.show_toast("Tác vụ nền chưa sẵn sàng để ghi âm.");
                }
            }
        }
    }
}
