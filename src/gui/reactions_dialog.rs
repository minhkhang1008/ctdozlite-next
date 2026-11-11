//! "Who reacted" dialog opened by clicking a message's reaction pill.
//!
//! The list is built only from the reaction ledger (`ReactionSummary::reactor_ids`), so every name
//! shown is a sender Zalo really reported. Counts that no known reactor explains (the part of a
//! history snapshot that has no per-user event) are listed as "Chưa rõ người thả" instead of
//! being attributed to anybody.

use super::*;

/// One person (or the unattributed remainder) in the dialog list.
const DIALOG_SIZE: Vec2 = Vec2::new(440.0, 350.0);
const DIALOG_ROUNDING: f32 = 8.0;
const HEADER_H: f32 = 52.0;
const LEFT_COL_W: f32 = 120.0;
const TAB_ROW_H: f32 = 36.0;
const REACTOR_ROW_H: f32 = 56.0;
const REACTOR_AVATAR_R: f32 = 20.0;

/// Paint an emoji with its Zalo texture when loaded, otherwise as text.
fn paint_emoji(
    painter: &egui::Painter,
    textures: &HashMap<String, TextureHandle>,
    emoji: &str,
    rect: Rect,
) {
    match textures.get(emoji_to_tex_key(emoji)) {
        Some(tex) => {
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                emoji,
                FontId::proportional(rect.height() * 0.8),
                Color32::WHITE,
            );
        }
    }
}

struct ReactorRow {
    name: String,
    avatar: Option<String>,
    unknown: bool,
    /// (emoji, count) pairs, most used first.
    icons: Vec<(String, u64)>,
}

impl ZaloGuiApp {
    /// Resolve a reactor id to a display name and avatar from data already loaded in the app.
    fn reactor_identity(&self, thread_idx: usize, id: &str) -> (String, Option<String>) {
        if id == "0" || (!self.user_uin.is_empty() && id == self.user_uin) {
            let name = if self.user_name.is_empty() {
                "Bạn".to_string()
            } else {
                self.user_name.clone()
            };
            return (name, self.user_avatar.clone());
        }
        if let Some(thread) = self.threads.get(thread_idx) {
            let member = self
                .group_members_map
                .get(&thread.id)
                .into_iter()
                .flatten()
                .chain(thread.presentation.members.iter())
                .find(|m| m.id == id && !m.name.is_empty());
            if let Some(m) = member {
                return (m.name.clone(), m.avatar.clone());
            }
        }
        if let Some(t) = self.threads.iter().find(|t| !t.is_group && t.id == id) {
            return (t.name.clone(), t.avatar.clone());
        }
        ("Người dùng Zalo".to_string(), None)
    }

    pub(super) fn render_reactions_dialog(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let Some((thread_id, msg_id)) = self.reactions_dialog.clone() else {
            return;
        };
        let Some(thread_idx) = self.threads.iter().position(|t| t.id == thread_id) else {
            self.reactions_dialog = None;
            return;
        };
        let Some(msg) = self.threads[thread_idx]
            .messages
            .iter()
            .find(|m| m.id == msg_id)
            .cloned()
        else {
            self.reactions_dialog = None;
            return;
        };

        let mut summaries: Vec<crate::reactions::ReactionSummary> = msg
            .reaction_summaries
            .iter()
            .filter(|s| s.count > 0)
            .cloned()
            .collect();
        summaries.sort_by(|a, b| b.count.cmp(&a.count));
        let total: u64 = summaries.iter().map(|s| s.count).sum();
        let tab = self
            .reactions_tab
            .filter(|kind| summaries.iter().any(|s| s.r_type == Some(*kind)));

        // Build rows first: names come from `self`, which the window closure borrows mutably.
        let mut rows: Vec<ReactorRow> = Vec::new();
        let mut index_by_id: HashMap<String, usize> = HashMap::new();
        let mut unknown_icons: Vec<(String, u64)> = Vec::new();
        for s in summaries
            .iter()
            .filter(|s| tab.is_none() || s.r_type == tab)
        {
            let mut known = 0u64;
            for id in &s.reactor_ids {
                let n = s.reactor_counts.get(id).copied().unwrap_or(1);
                known += n;
                let idx = *index_by_id.entry(id.clone()).or_insert_with(|| {
                    let (name, avatar) = self.reactor_identity(thread_idx, id);
                    rows.push(ReactorRow {
                        name,
                        avatar,
                        unknown: false,
                        icons: Vec::new(),
                    });
                    rows.len() - 1
                });
                rows[idx].icons.push((s.emoji.clone(), n));
            }
            if s.count > known {
                unknown_icons.push((s.emoji.clone(), s.count - known));
            }
        }
        if !unknown_icons.is_empty() {
            rows.push(ReactorRow {
                name: "Chưa rõ người thả".to_string(),
                avatar: None,
                unknown: true,
                icons: unknown_icons,
            });
        }

        let mut open = !ctx.input(|i| i.key_pressed(Key::Escape));
        let mut next_tab = tab;

        // Tabs: "Tất cả" plus one per reaction kind, most used first (Zalo PC layout).
        let mut tabs: Vec<(Option<i32>, Option<String>, u64)> = vec![(None, None, total)];
        tabs.extend(
            summaries
                .iter()
                .filter_map(|s| s.r_type.map(|k| (Some(k), Some(s.emoji.clone()), s.count))),
        );

        let screen = ctx.screen_rect();
        let mut backdrop_clicked_pos: Option<Pos2> = None;
        egui::Area::new(Id::new("reactions_modal_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    r,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, if pal.is_dark { 140 } else { 90 }),
                );
                if resp.clicked() {
                    backdrop_clicked_pos = resp.interact_pointer_pos();
                }
            });

        let card_size = Vec2::new(
            DIALOG_SIZE.x.min(screen.width() - 32.0).max(280.0),
            DIALOG_SIZE.y.min(screen.height() - 32.0).max(220.0),
        );
        let right_bg = pal.bg_elevated;
        let left_bg = if pal.is_dark {
            pal.bg_sidebar
        } else {
            pal.bg_hover
        };
        let tab_selected_bg = if pal.is_dark {
            pal.bg_hover
        } else {
            Color32::from_rgb(0xE5, 0xE7, 0xEB)
        };
        let card_area = egui::Area::new(Id::new("reactions_modal_card"))
            .order(egui::Order::Tooltip)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(right_bg)
                    .rounding(Rounding::same(DIALOG_ROUNDING))
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, 8.0),
                        blur: 24.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(if pal.is_dark { 120 } else { 40 }),
                    })
                    .show(ui, |ui| {
                        let (card, _) = ui.allocate_exact_size(card_size, Sense::hover());
                        let painter = ui.painter().clone();

                        // Header: title + close button, then a divider.
                        let header =
                            Rect::from_min_size(card.min, Vec2::new(card.width(), HEADER_H));
                        painter.text(
                            Pos2::new(header.left() + 18.0, header.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Biểu cảm",
                            FontId::proportional(17.0),
                            pal.text_primary,
                        );
                        let close_rect = Rect::from_center_size(
                            Pos2::new(header.right() - 26.0, header.center().y),
                            Vec2::splat(32.0),
                        );
                        let close = ui
                            .interact(close_rect, Id::new("reactions_modal_close"), Sense::click())
                            .on_hover_cursor(CursorIcon::PointingHand);
                        if close.hovered() {
                            painter.rect_filled(close_rect, Rounding::same(6.0), pal.bg_hover);
                        }
                        painter.text(
                            close_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            lucide::X,
                            FontId::proportional(20.0),
                            pal.text_primary,
                        );
                        if close.clicked() {
                            open = false;
                        }
                        painter.hline(
                            header.x_range(),
                            header.bottom(),
                            Stroke::new(1.0_f32, pal.border),
                        );

                        // Left column: reaction tabs.
                        let body_top = header.bottom() + 1.0;
                        let left = Rect::from_min_max(
                            Pos2::new(card.left(), body_top),
                            Pos2::new(card.left() + LEFT_COL_W, card.bottom()),
                        );
                        painter.rect_filled(
                            left,
                            Rounding {
                                sw: DIALOG_ROUNDING,
                                ..Rounding::ZERO
                            },
                            left_bg,
                        );
                        for (i, (kind, emoji, count)) in tabs.iter().enumerate() {
                            let row = Rect::from_min_size(
                                Pos2::new(left.left(), left.top() + 8.0 + i as f32 * TAB_ROW_H),
                                Vec2::new(LEFT_COL_W, TAB_ROW_H),
                            );
                            if row.bottom() > left.bottom() {
                                break;
                            }
                            let resp = ui
                                .interact(row, Id::new(("reactions_tab", i)), Sense::click())
                                .on_hover_cursor(CursorIcon::PointingHand);
                            if tab == *kind {
                                painter.rect_filled(row, Rounding::ZERO, tab_selected_bg);
                            } else if resp.hovered() {
                                painter.rect_filled(
                                    row,
                                    Rounding::ZERO,
                                    tab_selected_bg.gamma_multiply(0.6),
                                );
                            }
                            if resp.clicked() {
                                next_tab = *kind;
                            }
                            let label_pos = Pos2::new(row.left() + 16.0, row.center().y);
                            match emoji {
                                None => {
                                    painter.text(
                                        label_pos,
                                        egui::Align2::LEFT_CENTER,
                                        "Tất cả",
                                        FontId::proportional(15.0),
                                        pal.text_primary,
                                    );
                                }
                                Some(e) => paint_emoji(
                                    &painter,
                                    &self.emoji_textures,
                                    e,
                                    Rect::from_min_size(
                                        Pos2::new(label_pos.x, row.center().y - 11.0),
                                        Vec2::splat(22.0),
                                    ),
                                ),
                            }
                            painter.text(
                                Pos2::new(row.right() - 16.0, row.center().y),
                                egui::Align2::RIGHT_CENTER,
                                count.to_string(),
                                FontId::proportional(15.0),
                                pal.text_primary,
                            );
                        }

                        // Right column: who reacted.
                        let right = Rect::from_min_max(Pos2::new(left.right(), body_top), card.max);
                        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right), |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("reactions_modal_list")
                                .max_height(right.height())
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.add_space(8.0);
                                    if rows.is_empty() {
                                        ui.add_space(12.0);
                                        ui.horizontal(|ui| {
                                            ui.add_space(16.0);
                                            ui.label(
                                                RichText::new("Chưa có dữ liệu biểu cảm.")
                                                    .color(pal.text_muted)
                                                    .size(14.0),
                                            );
                                        });
                                    }
                                    for row in &rows {
                                        self.paint_reactor_row(ui, row, pal);
                                    }
                                });
                        });
                    });
            });
        if let Some(pos) = backdrop_clicked_pos {
            if !card_area.response.rect.contains(pos) {
                open = false;
            }
        }
        self.reactions_tab = next_tab;
        if !open {
            self.reactions_dialog = None;
        }
    }

    /// One reactor line: avatar, name (ellipsized) and that person's emoji on the right.
    fn paint_reactor_row(&mut self, ui: &mut egui::Ui, row: &ReactorRow, pal: &ThemePalette) {
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), REACTOR_ROW_H),
            Sense::hover(),
        );
        let painter = ui.painter().clone();
        if resp.hovered() {
            painter.rect_filled(rect, Rounding::ZERO, pal.bg_hover);
        }
        let av_center = Pos2::new(rect.left() + 16.0 + REACTOR_AVATAR_R, rect.center().y);
        if row.unknown {
            painter.circle_filled(av_center, REACTOR_AVATAR_R, pal.bg_input);
            painter.text(
                av_center,
                egui::Align2::CENTER_CENTER,
                "?",
                FontId::proportional(17.0),
                pal.text_muted,
            );
        } else {
            paint_user_avatar(
                &painter,
                av_center,
                REACTOR_AVATAR_R,
                &row.name,
                false,
                row.avatar.as_deref(),
                &self.sticker_textures,
                &mut self.loading_stickers,
            );
        }

        // Emoji block, laid out right to left: [emoji count] [emoji count] ...
        let mut x = rect.right() - 16.0;
        for (emoji, n) in &row.icons {
            let count =
                painter.layout_no_wrap(n.to_string(), FontId::proportional(15.0), pal.text_primary);
            x -= count.size().x;
            painter.galley(
                Pos2::new(x, rect.center().y - count.size().y / 2.0),
                count,
                pal.text_primary,
            );
            x -= 6.0 + 22.0;
            paint_emoji(
                &painter,
                &self.emoji_textures,
                emoji,
                Rect::from_min_size(Pos2::new(x, rect.center().y - 11.0), Vec2::splat(22.0)),
            );
            x -= 10.0;
        }

        let name_left = av_center.x + REACTOR_AVATAR_R + 12.0;
        let name_w = (x - name_left - 8.0).max(20.0);
        let mut job = egui::text::LayoutJob::single_section(
            row.name.clone(),
            egui::TextFormat::simple(
                FontId::proportional(15.0),
                if row.unknown {
                    pal.text_muted
                } else {
                    pal.text_primary
                },
            ),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(name_w);
        let name = painter.layout_job(job);
        painter.galley(
            Pos2::new(name_left, rect.center().y - name.size().y / 2.0),
            name,
            pal.text_primary,
        );
    }
}
