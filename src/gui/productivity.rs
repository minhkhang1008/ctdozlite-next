//! Account-local productivity UI; all disk work is ordered through the worker.
use super::*;
use crate::productivity::{Action, Draft, Snapshot, Update};
#[derive(Default)]
pub(super) struct State {
    account: String,
    requested: bool,
    loaded: bool,
    bound: String,
    pending_jump: Option<(String, String)>,
    drafts: HashMap<String, Draft>,
    pub snapshot: Snapshot,
    pub open: bool,
    pub tab: usize,
    query: String,
    generation: u64,
    search_pending: Option<std::time::Instant>,
    results: Vec<crate::store::StoredMessage>,
    last_tick: Option<std::time::Instant>,
    reminder_target: Option<(String, ChatMessage)>,
    pub quitting: bool,
    pub status: String,
    pending: HashMap<String, Draft>,
    note_edits: HashMap<String, String>,
    fired: Vec<crate::productivity::Reminder>,
}
impl ZaloGuiApp {
    pub(super) fn local_action(&self, action: Action) -> bool {
        WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Productivity {
                account: self.user_uin.clone(),
                action,
            })
            .is_ok()
        })
    }
    pub(super) fn flush_local_draft(&self) {
        if !self.user_uin.is_empty() {
            self.local_action(Action::Draft(self.current_draft()));
        }
    }
    pub(super) fn local_draft_preview(&self, thread: &str) -> Option<String> {
        self.productivity
            .drafts
            .get(thread)
            .filter(|d| !d.text.is_empty() || d.reply.is_some())
            .map(|d| format!("Nháp: {}", d.text.chars().take(80).collect::<String>()))
    }
    fn current_draft(&self) -> Draft {
        Draft {
            thread_id: self.productivity.bound.clone(),
            text: self.input_message.clone(),
            reply: self.replying_to.clone(),
            auto_mention: self
                .reply_auto_mention
                .as_ref()
                .map(|p| (p.uid.clone(), p.name.clone())),
            mentions: self
                .composer_mentions
                .iter()
                .map(|p| (p.uid.clone(), p.name.clone()))
                .collect(),
        }
    }
    fn store_current_draft(&mut self, draft: Draft) {
        if draft.thread_id.is_empty() {
            return;
        }
        let changed = self
            .productivity
            .drafts
            .get(&draft.thread_id)
            .map(|d| serde_json::to_string(d).ok())
            != Some(serde_json::to_string(&draft).ok());
        self.productivity
            .drafts
            .insert(draft.thread_id.clone(), draft.clone());
        if changed {
            if self.productivity.loaded {
                self.local_action(Action::Draft(draft));
            } else {
                self.productivity
                    .pending
                    .insert(draft.thread_id.clone(), draft);
            }
        }
    }
    fn restore_draft(&mut self, thread: String) {
        self.productivity.bound = thread.clone();
        let draft = self
            .productivity
            .drafts
            .get(&thread)
            .cloned()
            .unwrap_or_default();
        self.input_message = draft.text;
        self.replying_to = draft.reply;
        self.composer_mentions = draft
            .mentions
            .into_iter()
            .map(|(uid, name)| crate::chat::MentionPick { uid, name })
            .collect();
        self.reply_auto_mention = draft
            .auto_mention
            .map(|(uid, name)| crate::chat::MentionPick { uid, name });
    }
    pub(super) fn local_begin(&mut self, ctx: &egui::Context) -> Draft {
        if self.productivity.account != self.user_uin {
            self.productivity = State {
                account: self.user_uin.clone(),
                ..Default::default()
            };
            self.input_message.clear();
            self.cancel_reply();
            self.composer_mentions.clear();
        }
        if !self.productivity.requested {
            self.productivity.requested = self.local_action(Action::Load);
        }
        if self
            .productivity
            .last_tick
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(2))
        {
            self.local_action(Action::Tick);
            self.productivity.last_tick = Some(std::time::Instant::now());
        }
        ctx.request_repaint_after(Duration::from_secs(2));
        let selected = self
            .threads
            .get(self.selected_thread_index)
            .map(|t| t.id.clone())
            .unwrap_or_default();
        if self.productivity.bound != selected {
            self.store_current_draft(self.current_draft());
            self.restore_draft(selected);
        }
        self.current_draft()
    }
    pub(super) fn local_end(&mut self, before: Draft) {
        let selected = self
            .threads
            .get(self.selected_thread_index)
            .map(|t| t.id.clone())
            .unwrap_or_default();
        if selected != self.productivity.bound {
            self.productivity.pending_jump = None;
            self.store_current_draft(before);
            self.restore_draft(selected);
        } else if self.productivity.loaded
            || !self.input_message.is_empty()
            || self
                .productivity
                .drafts
                .contains_key(&self.productivity.bound)
        {
            self.store_current_draft(self.current_draft());
        }
    }
    pub(super) fn local_update(&mut self, account: String, update: Result<Update, String>) {
        if account != self.user_uin {
            return;
        }
        match update {
            Ok(Update::Located { thread, reference }) => {
                if self.productivity.pending_jump.as_ref()
                    != Some(&(thread.id.clone(), reference.clone()))
                {
                    return;
                }
                self.productivity.pending_jump = None;
                self.store_current_draft(self.current_draft());
                let thread_id = thread.id.clone();
                if let Some(existing) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                    crate::chat::merge_thread_into(existing, thread, true);
                } else {
                    self.threads.push(thread);
                }
                if let Some(index) = self.threads.iter().position(|t| t.id == thread_id) {
                    self.selected_thread_index = index;
                    self.restore_draft(thread_id.clone());
                }
                self.filter_starred_only = false;
                self.in_chat_search_open = false;
                self.in_chat_search_query.clear();
                self.quote_jump = Some(QuoteJump {
                    thread_id,
                    reference,
                    highlight_until: std::time::Instant::now() + Duration::from_secs(5),
                    scroll_pending: true,
                });
            }
            Ok(Update::Snapshot(snapshot)) => {
                let first = !self.productivity.loaded;
                for draft in &snapshot.drafts {
                    self.productivity
                        .drafts
                        .entry(draft.thread_id.clone())
                        .or_insert_with(|| draft.clone());
                }
                self.productivity.snapshot = snapshot;
                self.productivity.loaded = true;
                if first {
                    for draft in std::mem::take(&mut self.productivity.pending).into_values() {
                        self.local_action(Action::Draft(draft));
                    }
                }
                if first && self.input_message.is_empty() {
                    self.restore_draft(self.productivity.bound.clone());
                }
            }
            Ok(Update::Found {
                generation,
                messages,
            }) if generation == self.productivity.generation => {
                self.productivity.results = messages
            }
            Ok(Update::Due(items)) if !items.is_empty() => {
                for item in &items {
                    self.show_toast(format!("Nhắc hẹn: {}", item.preview));
                    if self.settings.enable_notifications {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::PostNativeNotification {
                                title: "Nhắc hẹn tin nhắn".into(),
                                body: item.preview.clone(),
                                avatar_url: None,
                            });
                        }
                    }
                }
                self.productivity.fired.extend(items);
                self.productivity.open = true;
                self.productivity.tab = 1;
                self.local_action(Action::Load);
            }
            Ok(Update::QuitReady) => std::process::exit(0),
            Err(error) => {
                self.productivity.quitting = false;
                self.productivity.status = error.clone();
                self.show_toast(error);
            }
            _ => {}
        }
    }
    pub(super) fn request_local_quit(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        if !self.productivity.quitting {
            self.productivity.quitting = true;
            if !self.local_action(Action::Quit(self.current_draft())) {
                self.productivity.quitting = false;
                self.show_toast("Không thể lưu bản nháp: tác vụ nền chưa sẵn sàng.");
            }
        }
    }
    pub(super) fn open_local_reminder(&mut self, thread: String, message: ChatMessage) {
        self.productivity.reminder_target = Some((thread, message));
        self.productivity.tab = 1;
        self.productivity.open = true;
    }
    fn jump_local(&mut self, thread: &str, msg: &str) {
        self.productivity.pending_jump = Some((thread.to_owned(), msg.to_owned()));
        self.local_action(Action::Locate {
            thread: thread.to_owned(),
            msg: msg.to_owned(),
        });
        self.productivity.open = false;
    }
    pub(super) fn render_local_window(&mut self, ctx: &egui::Context) {
        if !self.productivity.open {
            return;
        }
        let mut open = true;
        egui::Window::new("Tin nhắn của bạn").open(&mut open).default_width(600.0).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| { for (tab,label) in [(0,"Đã lưu"),(1,"Nhắc hẹn"),(2,"Tìm nâng cao")] { ui.selectable_value(&mut self.productivity.tab,tab,label); } });
            ui.label(&self.productivity.status);
            egui::ScrollArea::vertical().max_height(500.0).show(ui,|ui| match self.productivity.tab {
                0 => {
                    let items=self.productivity.snapshot.saved.clone();
                    if items.is_empty() { ui.label("Chọn Đánh dấu sao trên tin nhắn để lưu tại đây."); }
                    for item in items { ui.group(|ui| {
                        ui.label(format!("{} · {}",item.message.sender_name, item.message.thread_id));
                        ui.label(&item.message.text);
                        let mut note=self.productivity.note_edits.entry(item.message.msg_id.clone()).or_insert_with(||item.note.clone()).clone();
                        let response=ui.text_edit_multiline(&mut note);
                        if response.changed() { self.productivity.note_edits.insert(item.message.msg_id.clone(),note.clone()); }
                        if response.lost_focus() && note!=item.note { self.local_action(Action::Save { thread:item.message.thread_id.clone(),msg:item.message.msg_id.clone(),note:Some(note) }); }
                        ui.horizontal(|ui| {
                            if ui.button("Đến hội thoại").clicked() { self.jump_local(&item.message.thread_id,&item.message.msg_id); }
                            if ui.button("Bỏ lưu").clicked() {
                                for thread in &mut self.threads { for message in &mut thread.messages { if message.id==item.message.msg_id { message.is_starred=false; } } }
                                self.persist_threads_async();
                                self.local_action(Action::Save { thread:item.message.thread_id,msg:item.message.msg_id,note:None }); }
                        });
                    }); }
                }
                1 => {
                    if let Some((thread,msg))=self.productivity.reminder_target.clone() {
                        ui.label(format!("Nhắc riêng bạn: {}",msg.text));
                        ui.horizontal_wrapped(|ui| { for (seconds,label) in [(900,"Sau 15 phút"),(3600,"Sau 1 giờ"),(86400,"Ngày mai")] {
                            if ui.button(label).clicked() { self.local_action(Action::Remind { thread:thread.clone(),msg:msg.id.clone(),due:crate::login::unix_now() as i64+seconds,preview:msg.text.clone() }); self.productivity.reminder_target=None; }
                        } });
                    }
                    let mut items=self.productivity.snapshot.reminders.clone();
                    items.extend(self.productivity.fired.clone());
                    if items.is_empty() { ui.label("Chưa có nhắc hẹn. Chọn Nhắc hẹn trong menu tin nhắn."); }
                    for item in items { ui.group(|ui| {
                        ui.label(&item.preview); ui.label(crate::clock::message_time(item.due_at*1000));
                        ui.horizontal(|ui| { if ui.button("Đến hội thoại").clicked() { self.jump_local(&item.thread_id,&item.msg_id); } if ui.button("Hủy nhắc").clicked() { self.productivity.fired.retain(|r|r.id!=item.id); self.local_action(Action::RemoveReminder(item.id)); } });
                    }); }
                }
                _ => {
                    ui.label("from:tên hoặc ID · in:hội thoại · has:image/file/voice/link · before:YYYY-MM-DD · after:YYYY-MM-DD · is:saved · is:pinned · #tag · \"cụm từ\"");
                    ui.label("Tối đa 200 kết quả mới nhất; thêm bộ lọc để thu hẹp.");
                    let changed=ui.text_edit_singleline(&mut self.productivity.query).changed();
                    if changed {self.productivity.generation+=1;self.productivity.search_pending=Some(std::time::Instant::now());}
                    if ui.button("Tìm trong lịch sử đã lưu").clicked() || self.productivity.search_pending.is_some_and(|t|t.elapsed()>=Duration::from_millis(250)) {
                        self.productivity.search_pending=None;
                        self.local_action(Action::Search{generation:self.productivity.generation,query:self.productivity.query.clone()});
                    }
                    if self.productivity.search_pending.is_some(){ctx.request_repaint_after(Duration::from_millis(250));}
                    for message in self.productivity.results.clone() { if ui.button(format!("{} · {}",message.sender_name,message.text)).clicked() { self.jump_local(&message.thread_id,&message.msg_id); } }
                }
            });
        });
        self.productivity.open &= open;
    }
}
