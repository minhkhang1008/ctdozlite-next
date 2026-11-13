//! User-triggered document text viewer; no network or extraction runs on the UI thread.
use super::*;
static NEXT_REQUEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
#[derive(Default)]
pub(super) struct State {
    pub generation: u64,
    pub loading: bool,
    pub text: String,
    title: String,
    account: String,
    open: bool,
}
impl ZaloGuiApp {
    pub(super) fn open_doc_preview(&mut self, url: String, name: String) {
        self.doc_preview.generation =
            NEXT_REQUEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.doc_preview.loading = true;
        self.doc_preview.text.clear();
        self.doc_preview.title = name.clone();
        self.doc_preview.open = true;
        self.doc_preview.account = self.user_uin.clone();
        if let Some(tx) = WORKER_TX.get() {
            if tx
                .send(WorkerCmd::FetchDocPreview {
                    account: self.user_uin.clone(),
                    generation: self.doc_preview.generation,
                    url,
                    name,
                })
                .is_err()
            {
                self.doc_preview.loading = false;
                self.doc_preview.text = "Tác vụ nền chưa sẵn sàng.".into();
            }
        } else {
            self.doc_preview.loading = false;
            self.doc_preview.text = "Tác vụ nền chưa sẵn sàng.".into();
        }
    }
    pub(super) fn render_doc_preview(&mut self, ctx: &egui::Context) {
        if self.doc_preview.account != self.user_uin {
            self.doc_preview.open = false;
            self.doc_preview.text.clear();
        }
        if !self.doc_preview.open {
            return;
        }
        egui::Window::new(format!("Xem trước văn bản: {}", self.doc_preview.title))
            .open(&mut self.doc_preview.open)
            .default_width(650.0)
            .show(ctx, |ui| {
                if self.doc_preview.loading {
                    ui.spinner();
                    ui.label("Đang tải và đọc tài liệu…");
                } else {
                    if ui.button("Sao chép nội dung").clicked() {
                        ui.output_mut(|o| o.copied_text = self.doc_preview.text.clone());
                    }
                    egui::ScrollArea::both().max_height(600.0).show(ui, |ui| {
                        ui.add(
                            egui::Label::new(&self.doc_preview.text)
                                .selectable(true)
                                .wrap(),
                        );
                    });
                }
            });
    }
}
