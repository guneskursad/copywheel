//! Drawing the wheel and its search and command card.

use super::shapes::*;
use super::theme::Palette;
use super::wheel::{SCROLL_TO_ROW, SHORTCUT_NUMBERS, SegAction, Src, shortcut_digit};
use super::*;

const HIT_RADIUS: f32 = 142.0;
const HIT_MARGIN: f32 = 13.0;
const R_OUT: f32 = HIT_RADIUS + HIT_MARGIN;
const R_IN: f32 = 56.0;
const CARD_GAP: f32 = 18.0;
const CARD_INSET: f32 = 14.0;
const SEARCH_HEIGHT: f32 = 36.0;
const ROW: f32 = 44.0;
const ROW_GAP: f32 = 6.0;
const HEADER_ROW: f32 = 24.0;
const ROW_CONTROL: f32 = 24.0;
const BADGE_WIDTH: f32 = 19.0;
const BADGE_HEIGHT: f32 = 18.0;

const FRAME_INSET: f32 = 6.0;
const SECTOR_INSET: f32 = 18.0;
const TRACK_INSET: f32 = 8.0;
const TRACK_OUTSET: f32 = 22.0;
const BADGE_INSET: f32 = 11.0;
const BADGE_SIDE_CLEARANCE: f32 = 16.0;
const CENTER_ARC_HALF_SPAN: f32 = 35.0 * PI / 180.0;
const SELECTION_SECONDS: f64 = 0.120;

#[derive(Clone, Copy)]
struct SelectionMotion {
    opened: f64,
    changed: f64,
    selected: usize,
    from_angle: f32,
    to_angle: f32,
    from_strengths: [f32; PER_PAGE],
}

impl SelectionMotion {
    fn update(
        ctx: &egui::Context,
        opened: f64,
        now: f64,
        selected: usize,
        target: f32,
    ) -> (f32, [f32; PER_PAGE]) {
        let id = Id::new("wheel_selection_motion");
        let mut motion = ctx.data(|data| data.get_temp::<Self>(id))
            .filter(|motion| motion.opened == opened)
            .unwrap_or_else(|| {
                let mut strengths = [0.0; PER_PAGE];
                strengths[selected] = 1.0;
                Self {
                    opened,
                    changed: now - SELECTION_SECONDS,
                    selected,
                    from_angle: target,
                    to_angle: target,
                    from_strengths: strengths,
                }
            });

        let progress = ((now - motion.changed) / SELECTION_SECONDS).clamp(0.0, 1.0) as f32;
        let angle = egui::lerp(motion.from_angle..=motion.to_angle, progress);
        let strengths = std::array::from_fn(|i| {
            egui::lerp(motion.from_strengths[i]..=f32::from(i == motion.selected), progress)
        });
        let turn = (target - motion.to_angle + PI).rem_euclid(TAU) - PI;
        if motion.selected != selected || turn.abs() > 0.0001 {
            motion.selected = selected;
            motion.from_angle = (angle + PI).rem_euclid(TAU) - PI;
            motion.to_angle = motion.from_angle + (target - motion.from_angle + PI).rem_euclid(TAU) - PI;
            motion.from_strengths = strengths;
            motion.changed = now;
        }
        if now - motion.changed < SELECTION_SECONDS {
            ctx.request_repaint();
        }
        ctx.data_mut(|data| data.insert_temp(id, motion));
        (angle, strengths)
    }
}

fn shortcut_badge(colors: &Palette, painter: &egui::Painter, center: Pos2, text: &str, alpha: f32) {
    let color = with_alpha(colors.command_text, alpha);
    let label = painter.layout_no_wrap(text.to_owned(), FontId::proportional(11.0), color);
    let badge = Rect::from_center_size(center, vec2((label.size().x + 10.0).max(BADGE_WIDTH), BADGE_HEIGHT));
    let corner = CornerRadius::same(5);
    let shadow_opacity = if colors.dark_mode { alpha } else { 0.32 * alpha };
    painter.rect_filled(badge.translate(vec2(0.0, 1.5)), corner, with_alpha(colors.badge_shadow, shadow_opacity));
    painter.rect_filled(badge, corner, with_alpha(colors.command_bg, alpha));
    painter.galley(center - label.size() / 2.0, label, color);
}

impl App {
    pub(super) fn draw_wheel(&mut self, ctx: &egui::Context, numbered_row: Option<usize>) {
        let colors = palette(ctx);
        let (segs, pages) = self.wheel_segments();
        let n = segs.len();
        if self.wheel.sel >= n {
            self.wheel.sel = 0;
            self.wheel.item = None;
        }
        let now = ctx.input(|i| i.time);
        let t = ((now - self.wheel.opened) / 0.16).clamp(0.0, 1.0) as f32;
        if t < 1.0 {
            ctx.request_repaint();
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let scale = 0.9 + 0.1 * ease;
        let alpha = ease;
        let show_shortcuts = ctx.input(|i| i.modifiers.mac_cmd && !i.modifiers.alt && !i.modifiers.shift && !i.modifiers.ctrl)
            || (self.shot.is_some() && std::env::var("COPYWHEEL_START").as_deref() == Ok("wheel-shortcuts"));
        let show_ring_numbers = show_shortcuts && !self.wheel.number_panel;
        let show_panel_numbers = show_shortcuts && self.wheel.number_panel;

        let mut paste_row: Option<usize> = None;
        let mut copy_row: Option<usize> = None;
        let mut delete_row: Option<usize> = None;
        let mut edit_row: Option<usize> = None;
        // Drag and drop of favorites: where a release would put the dragged one.
        let mut drop_to: Option<(u64, Option<u64>)> = None;
        let mut slice_drop: Option<u64> = None;
        let released = ctx.input(|i| i.pointer.any_released());
        let drag_id = self.wheel_drag;
        let reveal_row = ctx.data_mut(|data| data.remove_temp::<bool>(Id::new(SCROLL_TO_ROW))).unwrap_or(false);
        let mut single_copy = None;
        let mut clicked_seg: Option<usize> = None;
        let mut back = false;
        let mut close = false;
        let at_root = matches!(self.wheel.level, Level::Root);

        let mut items = Vec::new();
        let mut searching = !self.wheel_query.trim().is_empty();

        let prompt_open = self.pending_snippet.is_some();
        egui::CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            if prompt_open {
                ui.disable();
                ctx.memory_mut(|memory| memory.surrender_focus(super::wheel::search_id()));
            }
            let rect = ui.max_rect();
            // The wheel sits in the middle; its list card opens beside the chosen piece.
            let c = pos2(rect.center().x, rect.center().y - 18.0);
            let r_out = R_OUT * scale;
            let r_in = R_IN * scale;
            let step = TAU / n as f32;
            let mid_of = |i: usize| -PI / 2.0 + i as f32 * step;
            let area = ui.interact(rect, Id::new("wheel_area"), Sense::click());
            let painter = ui.painter().clone();

            // ---- list card geometry: one place on each side of the wheel ----
            let card_w = (c.x - rect.left() - 14.0 - R_OUT - CARD_GAP).max(200.0);
            let card_h = rect.height() - 28.0;
            let card_right = Rect::from_min_size(pos2(c.x + R_OUT + CARD_GAP, rect.top() + 14.0), vec2(card_w, card_h));
            let card_left = Rect::from_min_size(pos2(c.x - R_OUT - CARD_GAP - card_w, rect.top() + 14.0), vec2(card_w, card_h));
            // Pieces on the right open it on the right, pieces on the left on the left;
            // the top piece opens it on the right and the bottom piece on the left.
            let opens_right = |i: usize| {
                let m = mid_of(i);
                if m.cos().abs() < 0.2 { m.sin() < 0.0 } else { m.cos() > 0.0 }
            };

            // ---- pointer on the wheel ----
            let pointer = if self.shot.is_some() || prompt_open { None } else { ctx.input(|i| i.pointer.hover_pos()) };
            let moved = ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO);

            let mut hover_idx = None;
            let mut in_center = false;
            if let Some(p) = pointer.filter(|p| !card_right.contains(*p) && !card_left.contains(*p)) {
                let d = p - c;
                let dist = d.length();
                if dist >= r_in && dist <= HIT_RADIUS * scale + HIT_MARGIN {
                    let a = d.y.atan2(d.x) + PI / 2.0 + step / 2.0;
                    hover_idx = Some(((a.rem_euclid(TAU) / step) as usize) % n);
                } else if dist < r_in {
                    in_center = true;
                }
            }
            if let (Some(h), true) = (hover_idx, drag_id.is_some()) {
                slice_drop = segs[h].action.group_id();
            }
            if let Some(h) = hover_idx
                && moved && !searching && !show_shortcuts && !self.wheel.number_panel
                && (h != self.wheel.sel || self.wheel.item.is_some())
            {
                self.wheel.sel = h;
                self.wheel.item = None;
            }
            // Fades out on one side and back in on the other when the side changes.
            let side = ctx.animate_bool_with_time(Id::new("wheel_card_side"), opens_right(self.wheel.sel), 0.16);
            let card = if side > 0.5 { card_right } else { card_left };
            let card_alpha = alpha * (2.0 * side - 1.0).abs();

            let sel = self.wheel.sel;
            let seg = &segs[sel];
            items = self.wheel_displayed_items(seg);
            let (angle, strengths) = SelectionMotion::update(ctx, self.wheel.opened, now, sel, mid_of(sel));

            let frame_radius = r_out - FRAME_INSET;
            let outer = r_out - SECTOR_INSET;
            let inner = r_in + TRACK_OUTSET;
            let frame_line = colors.line.gamma_multiply(0.70 * alpha);
            painter.circle_filled(c, frame_radius, colors.bg.gamma_multiply(alpha));
            for radius in [r_out, frame_radius] {
                painter.circle_stroke(c, radius, Stroke::new(1.0_f32, frame_line));
            }
            for i in -4..=4 {
                let d = egui::Vec2::angled(-PI / 2.0 + i as f32 * PI / 16.0);
                painter.line_segment(
                    [c + d * (r_out + 3.0), c + d * (r_out + 9.0)],
                    Stroke::new(1.0_f32, colors.line.gamma_multiply(alpha)),
                );
            }
            for i in 0..n {
                let boundary = mid_of(i) + step / 2.0;
                painter.circle_filled(
                    c + egui::Vec2::angled(boundary) * r_out,
                    1.5,
                    colors.accent.gamma_multiply(0.65 * alpha),
                );
            }
            painter.circle_filled(c + vec2(0.0, r_out), 2.5, colors.accent.gamma_multiply(alpha));

            let track_width = TRACK_OUTSET - TRACK_INSET;
            painter.circle_stroke(
                c,
                inner - track_width / 2.0,
                Stroke::new(track_width, colors.sector_track.gamma_multiply(alpha)),
            );
            let sectors: Vec<_> = (0..n).map(|i| {
                let mid = mid_of(i);
                Sector::new(c, inner, outer, mid - step / 2.0, mid + step / 2.0)
            }).collect();
            for (i, sector) in sectors.iter().enumerate() {
                let base = if hover_idx == Some(i) && i != sel { colors.hover } else { colors.sector_fill };
                let fill = mix(base, colors.accent, colors.sector_selected_opacity * strengths[i]);
                sector.fill(&painter, fill.gamma_multiply(alpha));
                sector.stroke(&painter, Stroke::new(1.0_f32, colors.sector_line.gamma_multiply(alpha)));
            }
            for (sector, strength) in sectors.iter().zip(strengths) {
                if strength > 0.0 {
                    sector.glow(&painter, colors.accent, alpha * strength);
                }
            }

            let badge_radius = outer - BADGE_INSET;
            let badge_side_angle = (BADGE_SIDE_CLEARANCE / badge_radius).asin();
            let badges: Vec<_> = segs.iter().enumerate().map(|(i, seg)| {
                if matches!(seg.action, SegAction::Nothing) {
                    return None;
                }
                let angle = mid_of(i) + step / 2.0 - badge_side_angle;
                let center = c + egui::Vec2::angled(angle) * badge_radius;
                let strength = strengths[i];
                let color = if show_shortcuts {
                    colors.text.gamma_multiply(alpha)
                } else {
                    mix(colors.text, colors.accent, strength)
                        .gamma_multiply(alpha * egui::lerp(0.35..=1.0, strength))
                };
                let galley = painter.layout_no_wrap(shortcut_digit(i).to_string(), FontId::proportional(12.0), color);
                Some((Rect::from_center_size(center, galley.size()), galley, color))
            }).collect();

            let label_font = FontId::proportional(15.0);
            let half_height = ui.fonts(|fonts| fonts.row_height(&label_font)) / 2.0;
            for (i, seg) in segs.iter().enumerate() {
                let mid = mid_of(i);
                let offset = egui::Vec2::angled(mid) * ((outer + inner) / 2.0);
                let center = c + offset;
                let mut width = sector_label_width(offset, half_height, inner, outer, mid, step);
                for (badge, _, _) in badges.iter().flatten() {
                    let badge = badge.expand(2.0);
                    if center.y + half_height > badge.top() && center.y - half_height < badge.bottom() {
                        let room = (center.x - badge.center().x).abs() - badge.width() / 2.0;
                        width = width.min(2.0 * room);
                    }
                }
                let base_opacity = if hover_idx == Some(i) { 1.0 } else { 0.85 };
                let color = mix(colors.text, colors.sector_selected_text, strengths[i])
                    .gamma_multiply(alpha * egui::lerp(base_opacity..=1.0, strengths[i]));
                let label = elided(ui, &seg.label, label_font.clone(), color, (width - 4.0).max(0.0), 1);
                let label_rect = Rect::from_center_size(center, label.size());
                if badges.iter().flatten().any(|(badge, _, _)| badge.expand(1.0).intersects(label_rect)) {
                    continue;
                }
                let pos = label_rect.min;
                painter.galley(pos, label.clone(), color);
                // The bundled proportional face has one weight. Offset a second pass for emphasis.
                painter.galley_with_override_text_color(
                    pos + vec2(0.45, 0.0),
                    label,
                    color.gamma_multiply(egui::lerp(0.25..=0.85, strengths[i])),
                );
            }
            for (rect, badge, color) in badges.into_iter().flatten() {
                painter.galley(rect.min, badge, color);
            }

            painter.circle(
                c,
                r_in,
                colors.wheel_disc.gamma_multiply(alpha),
                Stroke::new(1.0_f32, colors.line.gamma_multiply(alpha)),
            );
            painter.add(egui::Shape::line(
                arc_points(c, r_in - 6.0, angle - CENTER_ARC_HALF_SPAN..=angle + CENTER_ARC_HALF_SPAN),
                Stroke::new(1.5_f32, colors.accent.gamma_multiply(0.50 * alpha)),
            ));
            let title_color = colors.text.gamma_multiply(alpha);
            let title = elided(ui, &seg.label, FontId::proportional(20.0), title_color, r_in * 1.55, 1);
            let title_pos = c - title.size() / 2.0 - vec2(0.0, 7.0);
            painter.galley(title_pos, title.clone(), title_color);
            painter.galley_with_override_text_color(title_pos + vec2(0.4, 0.0), title, title_color.gamma_multiply(0.25));

            let count = match seg.action {
                SegAction::Paste(_) => 1,
                _ => seg.items.iter().filter(|item| !matches!(item.src, Src::Header(_))).count(),
            };
            let dim = colors.dim.gamma_multiply(alpha);
            painter.text(
                c + vec2(0.0, 13.0),
                Align2::CENTER_CENTER,
                format!("{count} {}", if count == 1 { "item" } else { "items" }),
                FontId::proportional(12.0),
                dim,
            );

            let foot = if !at_root {
                self.wheel_title()
            } else if pages > 1 {
                format!("Page {}/{}", self.wheel.page.min(pages - 1) + 1, pages)
            } else {
                String::new()
            };
            if !foot.is_empty() {
                let g = elided(ui, &foot, FontId::proportional(10.0), dim, r_out * 1.45, 1);
                painter.galley(pos2(c.x - g.size().x / 2.0, c.y - r_out - g.size().y - 13.0), g, dim);
            }

            // ---- search bar + command cards beside the chosen piece ----
            let wheel_alpha = alpha;
            let alpha = card_alpha;
            let accent_soft = colors.wheel_selection(alpha);
            let single = !searching && matches!(segs[sel].action, SegAction::Paste(_));
            let content_height: f32 = items.iter().map(|item| {
                if matches!(item.src, Src::Header(_)) { HEADER_ROW } else { ROW }
            }).sum();
            let content_height = content_height + items.len().saturating_sub(1) as f32 * ROW_GAP;
            let list_top = CARD_INSET + SEARCH_HEIGHT + 26.0;
            let wanted_height = if single { card.height() } else { (list_top + content_height + CARD_INSET).max(160.0) };
            let card = Rect::from_min_size(card.min, vec2(card.width(), wanted_height.min(card.height())));
            if area.clicked() {
                if let Some(h) = hover_idx {
                    clicked_seg = Some(h);
                } else if in_center {
                    back = true;
                } else if pointer.is_some_and(|p| !card.contains(p)) {
                    close = true;
                }
            }
            card_shadow(colors, &painter, card, 16, alpha, 0.55);
            painter.rect_filled(card, CornerRadius::same(16), with_alpha(colors.list_bg, alpha));
            painter.rect_stroke(card, CornerRadius::same(16), Stroke::new(1.0_f32, with_alpha(colors.list_line, alpha)), StrokeKind::Inside);

            let inner = card.shrink(CARD_INSET);
            let search_rect = Rect::from_min_size(inner.min, vec2(inner.width(), SEARCH_HEIGHT));
            let search_id = super::wheel::search_id();
            let search_focused = ctx.memory(|m| m.has_focus(search_id));
            let copy_selected = !prompt_open && super::wheel::take_wheel_copy(ctx, search_id, search_focused);
            let (search_edge, border_width) = if search_focused { (colors.accent, 1.5_f32) } else { (colors.list_line, 1.0) };
            painter.rect_filled(search_rect, CornerRadius::same(11), with_alpha(colors.bg2, alpha));
            painter.rect_stroke(search_rect, CornerRadius::same(11), Stroke::new(border_width, with_alpha(search_edge, alpha)), StrokeKind::Inside);

            let (up, down, enter) = if !prompt_open && (searching || search_focused) {
                ctx.input_mut(|i| {
                    (
                        i.consume_key(Modifiers::NONE, Key::ArrowUp),
                        i.consume_key(Modifiers::NONE, Key::ArrowDown),
                        i.consume_key(Modifiers::NONE, Key::Enter),
                    )
                })
            } else {
                (false, false, false)
            };
            let mut scroll_to_selection = reveal_row;
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(search_rect.shrink2(vec2(10.0, 7.0))), |ui| {
                ui.set_opacity(alpha);
                ui.horizontal(|ui| {
                    icon_label(ui, Icon::Search, 15.0, if search_focused { colors.wheel_action_hover } else { colors.dim });
                    let w = (ui.available_width() - 42.0).max(60.0);
                    let r = ui.add(
                        TextEdit::singleline(&mut self.wheel_query)
                            .id(search_id)
                            .hint_text(RichText::new("Search commands or favorites…").color(colors.dim))
                            .frame(false)
                            .text_color(colors.text)
                            .font(FontId::proportional(13.0))
                            .desired_width(w),
                    );
                    if (r.clicked() || r.changed()) && !self.wheel.number_panel {
                        self.wheel.number_panel = true;
                        ctx.request_repaint();
                    }
                    if r.changed() {
                        ctx.request_repaint();
                        items = self.wheel_displayed_items(&segs[sel]);
                        searching = !self.wheel_query.trim().is_empty();
                        self.wheel.item = if searching { super::wheel::step_item(&items, None, 1) } else { None };
                        scroll_to_selection = searching;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        Frame::NONE
                            .fill(colors.list_bg)
                            .stroke(Stroke::new(1.0_f32, colors.list_line))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.label(RichText::new("⌘K").size(10.5).color(colors.dim));
                            });
                    });
                });
            });

            if searching {
                if down || up {
                    let direction = if down { 1 } else { -1 };
                    self.wheel.item = super::wheel::step_item(&items, self.wheel.item, direction);
                    scroll_to_selection = true;
                }
                if enter {
                    copy_row = self.wheel.item.filter(|&row| row < items.len())
                        .or_else(|| super::wheel::step_item(&items, None, 1));
                }
            }

            let header_y = search_rect.bottom() + 14.0;
            let single = !searching && matches!(segs[sel].action, SegAction::Paste(_));
            if copy_selected {
                copy_row = self.wheel.item.filter(|&row| {
                    items.get(row).is_some_and(|item| !matches!(item.src, Src::Header(_)))
                });
            }
            let mut copy_single = single && (numbered_row == Some(0) || copy_selected);
            let command_count = items.iter().filter(|item| !matches!(item.src, Src::Header(_))).count() + usize::from(single);
            let count = if searching { format!("{command_count} results") } else { format!("{command_count} items") };
            let count = painter.layout_no_wrap(count, FontId::proportional(11.5), with_alpha(colors.dim, alpha));
            let count_x = inner.right() - count.size().x;
            let heading = if searching {
                "Search results".to_owned()
            } else if single {
                self.wheel_title()
            } else {
                segs[sel].label.clone()
            };
            let heading = elided(ui, &heading, FontId::proportional(15.0), with_alpha(colors.text, alpha), count_x - inner.left() - 10.0, 1);
            let heading_color = with_alpha(colors.text, alpha);
            strong_galley(&painter, pos2(inner.left(), header_y - heading.size().y / 2.0), heading, heading_color);
            painter.galley(pos2(count_x, header_y - count.size().y / 2.0), count, with_alpha(colors.dim, alpha));

            let list_rect = Rect::from_min_max(pos2(inner.left(), card.top() + list_top), inner.max);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(list_rect), |ui| {
                ui.set_opacity(alpha);
                if single {
                    let seg = &segs[sel];
                    Frame::NONE
                        .fill(colors.bg2)
                        .stroke(Stroke::new(1.0f32, colors.line))
                        .corner_radius(CornerRadius::same(12))
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&seg.label).strong().size(13.5).color(colors.text));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let label = if show_panel_numbers { "⌘1  Copy" } else { "Copy" };
                                    if small_button(ui, label).clicked() {
                                        copy_single = true;
                                    }
                                });
                            });
                            ui.add_space(6.0);
                            Frame::NONE.fill(colors.command_bg).corner_radius(CornerRadius::same(7)).inner_margin(Margin::same(8)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(RichText::new(&seg.detail).monospace().size(11.5).color(colors.command_text));
                            });
                        });
                } else if items.is_empty() {
                    let empty = if searching { "No matching items." } else { "No items yet." };
                    let label_color = with_alpha(colors.dim, alpha);
                    let label = elided(ui, empty, FontId::proportional(12.0), label_color, list_rect.width() - 20.0, 1);
                    painter.with_clip_rect(list_rect).galley(list_rect.center() - label.size() / 2.0, label, label_color);
                } else {
                    ui.set_clip_rect(ui.clip_rect().intersect(list_rect));
                    let mut scroll = ScrollArea::vertical().auto_shrink([false, false]).id_salt("wheel_cards");
                    if std::mem::take(&mut self.wheel.reset_card_scroll) {
                        scroll = scroll.vertical_scroll_offset(0.0);
                    }
                    scroll.show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let mut visible_index = 0;
                        for (r, it) in items.iter().enumerate() {
                            if r > 0 {
                                ui.add_space(ROW_GAP);
                            }
                            if let Src::Header(hgid) = it.src {
                                let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_ROW), Sense::hover());
                                let hp = ui.painter_at(hr);
                                if drag_id.is_some() && pointer.is_some_and(|p| hr.contains(p) && list_rect.contains(p)) {
                                    hp.rect_filled(hr, CornerRadius::same(6), with_alpha(colors.accent, 0.14 * alpha));
                                    drop_to = Some((hgid, None));
                                }
                                let label_color = with_alpha(colors.wheel_action_hover, alpha);
                                let g = elided(ui, &it.title, FontId::proportional(11.5), label_color, hr.width() - 8.0, 1);
                                hp.galley(pos2(hr.left() + 4.0, hr.bottom() - g.size().y - 3.0), g, label_color);
                                hp.line_segment(
                                    [pos2(hr.left() + 4.0, hr.bottom() - 1.0), pos2(hr.right() - 4.0, hr.bottom() - 1.0)],
                                    Stroke::new(1.0f32, with_alpha(colors.line, alpha)),
                                );
                                continue;
                            }
                            let (rr, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click_and_drag());
                            let visible = ui.clip_rect().contains_rect(rr);
                            let row_key = if visible && visible_index < SHORTCUT_NUMBERS.len() {
                                let index = visible_index;
                                visible_index += 1;
                                Some(index)
                            } else { None };
                            if row_key.is_some() && row_key == numbered_row {
                                copy_row = Some(r);
                            }
                            if resp.drag_started() {
                                self.wheel_drag = Some(it.src);
                            }
                            let hovered = pointer.is_some() && ui.rect_contains_pointer(rr) && drag_id.is_none();
                            if hovered && moved && !scroll_to_selection && !show_shortcuts && self.wheel.item != Some(r) {
                                self.wheel.item = Some(r);
                                ctx.request_repaint();
                            }
                            let on = self.wheel.item == Some(r);
                            if on && scroll_to_selection && !visible {
                                resp.scroll_to_me(None);
                            }
                            if resp.clicked() {
                                paste_row = Some(r);
                            }
                            let p = ui.painter_at(rr);
                            if on {
                                let face = rr.shrink2(vec2(0.0, 1.0));
                                p.rect_filled(face, CornerRadius::same(7), accent_soft);
                                let bar = Rect::from_min_size(rr.min + vec2(2.0, 7.0), vec2(3.0, ROW - 14.0));
                                p.rect_filled(bar, CornerRadius::same(2), with_alpha(colors.accent, alpha));
                            } else {
                                let fill = if hovered { colors.row_hover } else { colors.row_bg };
                                p.rect(
                                    rr,
                                    CornerRadius::same(10),
                                    with_alpha(fill, alpha),
                                    Stroke::new(1.0_f32, with_alpha(colors.row_line, alpha)),
                                    StrokeKind::Inside,
                                );
                            }
                            if let Src::Fav(fid) = it.src
                                && drag_id.is_some() && drag_id != Some(it.src)
                                && pointer.is_some_and(|p| rr.contains(p) && list_rect.contains(p))
                                && let Some(favorite) = self.store.favorites.iter().find(|f| f.id == fid)
                            {
                                drop_to = Some((favorite.group, Some(fid)));
                                ui.painter().line_segment(
                                    [pos2(rr.left() + 6.0, rr.top() + 1.0), pos2(rr.right() - 6.0, rr.top() + 1.0)],
                                    Stroke::new(2.5f32, with_alpha(colors.accent, alpha)),
                                );
                            }

                            let mut text_right = rr.right() - 6.0;
                            let mut hovered_btn = false;
                            if on || hovered {
                                let edit_tip = if matches!(it.src, Src::Clip(_)) { "Edit and add to favorites" } else { "Edit" };
                                let armed = self.wheel_del_armed == Some(it.src);
                                let delete_tip = if armed { "Click again to delete" } else { "Delete" };
                                let danger = colors.danger;
                                for (icon, tip, clicked, is_armed, tint) in [
                                    (Icon::Copy, "Copy", &mut copy_row, false, colors.wheel_action_hover),
                                    (Icon::Pencil, edit_tip, &mut edit_row, false, colors.wheel_action_hover),
                                    (Icon::Trash, delete_tip, &mut delete_row, armed, danger),
                                ].into_iter().rev() {
                                    let rect = Rect::from_center_size(
                                        pos2(text_right - ROW_CONTROL / 2.0, rr.center().y),
                                        vec2(ROW_CONTROL, ROW_CONTROL),
                                    );
                                    let button = ui.interact(rect, resp.id.with(("action", icon)), Sense::click());
                                    if button.clicked() {
                                        *clicked = Some(r);
                                    }
                                    let color = if is_armed {
                                        p.rect_filled(rect, CornerRadius::same(7), with_alpha(danger, alpha));
                                        Color32::WHITE
                                    } else if button.hovered() {
                                        p.rect_filled(rect, CornerRadius::same(7), with_alpha(colors.hover, alpha));
                                        tint
                                    } else {
                                        colors.dim
                                    };
                                    paint_icon(&p, icon, Rect::from_center_size(rect.center(), vec2(14.0, 14.0)), with_alpha(color, alpha));
                                    hovered_btn |= button.hovered();
                                    button.on_hover_text(tip);
                                    text_right = rect.left();
                                    if icon == Icon::Trash {
                                        text_right -= DELETE_ICON_GAP;
                                    }
                                }
                                text_right -= 4.0;
                            }

                            let key_label = row_key.filter(|_| show_panel_numbers).map(shortcut_digit);
                            let inset = if let Some(digit) = key_label {
                                shortcut_badge(colors, &p, pos2(rr.left() + 21.0, rr.center().y), &format!("⌘{digit}"), alpha);
                                38.0
                            } else {
                                9.0
                            };
                            let command_left = rr.left() + inset;
                            let title_color = with_alpha(colors.dim, alpha);
                            let title_left = command_left + 6.0;
                            let title = elided(ui, &it.title, FontId::proportional(11.5), title_color, (text_right - title_left).max(1.0), 1);
                            p.galley(pos2(title_left, rr.top() + 3.0), title, title_color);

                            let command = Rect::from_min_max(pos2(command_left, rr.top() + 19.0), pos2(text_right, rr.bottom() - 3.0));
                            if on {
                                p.rect_filled(command, CornerRadius::same(5), with_alpha(colors.command_bg, alpha));
                            }
                            let command_color = with_alpha(if on { colors.command_text } else { colors.text }, alpha);
                            let cg = elided(ui, &it.detail, FontId::monospace(12.0), command_color, (command.width() - 12.0).max(1.0), 1);
                            let command_y = command.center().y - cg.size().y / 2.0;
                            let truncated = cg.elided;
                            p.galley(pos2(command.left() + 6.0, command_y), cg, command_color);
                            // Keep short commands visible and reveal longer ones only from their preview.
                            if !hovered_btn && !resp.is_pointer_button_down_on() && drag_id.is_none() && !show_shortcuts
                                && let Some(Content::Text { text }) = self.source_content(it.src)
                                && (text != &it.detail || truncated)
                            {
                                let preview = ui.interact(command, resp.id.with("command_preview"), Sense::hover());
                                preview.on_hover_ui(|ui| {
                                    ui.set_max_width(480.0);
                                    ui.label(RichText::new(text).monospace().size(11.5));
                                });
                            }
                        }
                    });
                }
            });
            if copy_single && let SegAction::Paste(source) = segs[sel].action {
                single_copy = Some(source);
            }

            // ---- key hints under the wheel ----
            let alpha = wheel_alpha;
            let hint = if show_panel_numbers {
                "⌘ + number Copy   Esc Wheel"
            } else if show_ring_numbers && pages > 1 {
                "⌘ + number Segment   ⌘J Next page"
            } else if show_ring_numbers {
                "⌘ + number Segment"
            } else if searching {
                "↑ ↓ Select  ↵ Copy  Esc Clear"
            } else {
                match (self.wheel.level, self.wheel.item.is_some()) {
                    (Level::Root, false) => "⬅ ➡ Select  ⌘K Search  1–9 Group  Esc Close",
                    (Level::Root, true) => "⬆ ⬇ Select  ↩ Paste  ⌘K Search  Esc Back",
                    _ => "⬅ ➡ Select  ↩ Paste  1–9 Quick select  Esc Back",
                }
            };
            let g = painter.layout_no_wrap(hint.into(), FontId::proportional(10.5), with_alpha(colors.dim, alpha));
            let pr = Rect::from_center_size(pos2(c.x, c.y + R_OUT + 26.0), g.size() + vec2(22.0, 12.0));
            card_shadow(colors, &painter, pr, 12, alpha, 0.25);
            painter.rect_filled(pr, CornerRadius::same(12), with_alpha(colors.bg2, alpha));
            painter.rect_stroke(pr, CornerRadius::same(12), Stroke::new(1.0f32, with_alpha(colors.line, alpha)), StrokeKind::Inside);
            painter.galley(pr.center() - g.size() / 2.0, g, with_alpha(colors.dim, alpha));

            // Pin: keeps the wheel open when another app (screenshot tool, etc.) takes focus.
            let pin_rect = Rect::from_center_size(pos2(pr.right() + 24.0, pr.center().y), vec2(30.0, 30.0));
            let pin_resp = ui.interact(pin_rect, Id::new("wheel_pin"), Sense::click());
            if pin_resp.clicked() {
                self.pinned = !self.pinned;
            }
            let (pfill, pcol) = if self.pinned {
                (colors.accent, colors.on_accent)
            } else if pin_resp.hovered() {
                (colors.bg2, colors.text)
            } else {
                (colors.bg2, colors.dim)
            };
            card_shadow(colors, &painter, pin_rect, 15, alpha, 0.25);
            painter.rect_filled(pin_rect, CornerRadius::same(15), with_alpha(pfill, alpha));
            painter.rect_stroke(pin_rect, CornerRadius::same(15), Stroke::new(2.0_f32, with_alpha(colors.accent, alpha)), StrokeKind::Inside);
            paint_icon(&painter, Icon::Pin, Rect::from_center_size(pin_rect.center(), vec2(15.0, 15.0)), with_alpha(pcol, alpha));
            pin_resp.on_hover_text("Pin the wheel to keep it open when switching apps");
        });

        if let Some(fav) = self.wheel_drag {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
                let title = match fav {
                    super::wheel::Src::Fav(id) => self.store.favorites.iter().find(|f| f.id == id).map(|f| f.title.clone()),
                    super::wheel::Src::Clip(id) => self.store.history.iter().find(|c| c.id == id).map(|c| c.content.preview(28)),
                    super::wheel::Src::Header(_) => None,
                }
                .unwrap_or_default();
                let lp = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("wheel_drag_ghost")));
                let g = lp.layout_no_wrap(title, FontId::proportional(12.0), colors.on_accent);
                let r = Rect::from_min_size(p + vec2(12.0, 8.0), g.size() + vec2(18.0, 10.0));
                lp.rect_filled(r, CornerRadius::same(8), colors.accent);
                lp.galley(r.min + vec2(9.0, 5.0), g, colors.on_accent);
            }
            if released && let Some((group, before)) = drop_to.or(slice_drop.map(|g| (g, None))) {
                self.wheel_drop(fav, group, before);
            }
            if released || !ctx.input(|i| i.pointer.any_down()) {
                self.wheel_drag = None;
            }
        }

        if let Some(item) = copy_row.and_then(|r| items.get(r)) {
            self.copy_wheel(item.src);
        } else if let Some(source) = single_copy {
            self.copy_wheel(source);
        } else if let Some(item) = edit_row.and_then(|r| items.get(r)) {
            self.wheel_edit_row(ctx, item.src);
        } else if let Some(item) = delete_row.and_then(|r| items.get(r)) {
            self.wheel_delete_row(item.src, true);
        } else if let Some(r) = paste_row {
            if let Some(item) = items.get(r) {
                self.paste_wheel(item.src);
            }
        } else if let Some(i) = clicked_seg {
            self.wheel.sel = i;
            self.wheel_activate(ctx, i);
        } else if back {
            self.wheel_back(ctx);
        } else if close {
            self.hide();
        }
    }
}
