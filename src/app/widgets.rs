//! Drawing helpers shared by the panel and the wheel.

use super::*;

/// System sans-serif for crisp UI labels and monospace for command previews.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    #[cfg(target_os = "macos")]
    let sans: &[(&str, &str)] = &[
        ("sf", "/System/Library/Fonts/SFNS.ttf"),
        ("sf2", "/System/Library/Fonts/Helvetica.ttc"),
    ];
    #[cfg(target_os = "macos")]
    let monos: &[(&str, &str)] = &[
        ("sfmono", "/System/Library/Fonts/SFNSMono.ttf"),
        ("menlo", "/System/Library/Fonts/Menlo.ttc"),
        (
            "andale",
            "/System/Library/Fonts/Supplemental/Andale Mono.ttf",
        ),
    ];
    #[cfg(windows)]
    let sans: &[(&str, &str)] = &[("segoe", r"C:\Windows\Fonts\segoeui.ttf")];
    #[cfg(windows)]
    let monos: &[(&str, &str)] = &[
        ("cascadia", r"C:\Windows\Fonts\CascadiaMono.ttf"),
        ("consolas", r"C:\Windows\Fonts\consola.ttf"),
    ];
    #[cfg(not(any(target_os = "macos", windows)))]
    let sans: &[(&str, &str)] = &[];
    #[cfg(not(any(target_os = "macos", windows)))]
    let monos: &[(&str, &str)] = &[];

    for (name, path) in sans {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert((*name).into(), Arc::new(egui::FontData::from_owned(bytes)));
            if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                list.insert(0, (*name).into());
            }
            break;
        }
    }
    let mut mono = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    for (name, path) in monos {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert((*name).into(), Arc::new(egui::FontData::from_owned(bytes)));
            mono.insert(0, (*name).into());
            break;
        }
    }
    fonts.families.insert(egui::FontFamily::Monospace, mono);
    ctx.set_fonts(fonts);
}

pub(super) fn setup_style(ctx: &egui::Context) {
    setup_fonts(ctx);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(10.0, 5.0);
        // A solid bar beside the list instead of one floating over it: the floating
        // bar covered the delete/star/edit buttons at the right edge of the rows.
        let mut scroll = egui::style::ScrollStyle::solid();
        scroll.bar_width = 6.0;
        scroll.bar_inner_margin = 4.0;
        s.spacing.scroll = scroll;
        use egui::TextStyle::*;
        s.text_styles.insert(Body, FontId::proportional(13.5));
        s.text_styles.insert(Button, FontId::proportional(13.0));
        s.text_styles.insert(Small, FontId::proportional(11.0));
    });
    theme::setup_visuals(ctx);
}

pub(super) fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub(super) fn elided(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
    rows: usize,
) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(text.to_string(), font, color, width);
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = rows == 1;
    job.wrap.overflow_character = Some('…');
    ui.fonts(|f| f.layout_job(job))
}

pub(super) fn icon_button(ui: &mut Ui, icon: Icon, tip: &str, on: bool) -> egui::Response {
    let colors = palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let fill = if on {
        colors.accent.gamma_multiply(0.25)
    } else if resp.hovered() {
        colors.bg2
    } else {
        theme::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
    let col = if on {
        colors.accent
    } else if resp.hovered() {
        colors.text
    } else {
        colors.dim
    };
    paint_icon(
        ui.painter(),
        icon,
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        col,
    );
    resp.on_hover_text(tip)
}

pub(super) fn small_button(ui: &mut Ui, text: &str) -> egui::Response {
    let colors = palette(ui.ctx());
    ui.add(
        egui::Button::new(RichText::new(text).size(12.5).color(colors.text))
            .fill(colors.bg2)
            .stroke(Stroke::new(1.0f32, colors.line))
            .corner_radius(CornerRadius::same(7)),
    )
}

pub(super) fn accent_button(ui: &mut Ui, text: &str) -> egui::Response {
    let colors = palette(ui.ctx());
    ui.add(
        egui::Button::new(RichText::new(text).color(colors.on_accent).strong())
            .fill(colors.accent)
            .corner_radius(CornerRadius::same(8)),
    )
}

pub(super) fn section(ui: &mut Ui, title: &str) {
    let colors = palette(ui.ctx());
    ui.add_space(10.0);
    ui.label(
        RichText::new(title.to_uppercase())
            .size(11.0)
            .strong()
            .color(colors.accent),
    );
    ui.add_space(2.0);
}

pub(super) fn empty_note(ui: &mut Ui, text: &str) {
    let colors = palette(ui.ctx());
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        icon_label(
            ui,
            Icon::Clipboard,
            40.0,
            mix(colors.hover, colors.dim, 0.45),
        );
        ui.add_space(4.0);
        ui.label(RichText::new(text).color(colors.dim));
    });
}

pub(super) const PANEL_ROW_HEIGHT: f32 = 34.0;

pub(super) struct RowOut {
    pub response: egui::Response,
    pub clicked: bool,
    pub copy: bool,
    pub star: bool,
    pub edit: bool,
    pub delete: bool,
    #[allow(dead_code)]
    pub hovered_moved: bool,
}

pub(super) struct RowSpec<'a> {
    pub n: usize,
    pub selected: bool,
    pub content: &'a Content,
    pub title: Option<&'a str>,
    pub meta: &'a str,
    pub fav: Option<bool>,
    pub dot: Option<Color32>,
    pub marked: bool,
}

pub(super) fn draw_row(
    ui: &mut Ui,
    textures: &mut HashMap<String, Option<TextureHandle>>,
    s: RowSpec<'_>,
) -> RowOut {
    draw_indented_row(ui, textures, s, 0.0)
}

pub(super) fn draw_indented_row(
    ui: &mut Ui,
    textures: &mut HashMap<String, Option<TextureHandle>>,
    s: RowSpec<'_>,
    indent: f32,
) -> RowOut {
    let colors = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), PANEL_ROW_HEIGHT), Sense::click());
    let hovered = response.contains_pointer();
    panel_row_fill(ui, rect, s.selected, hovered);
    let center = rect.center().y;
    let x = rect.left() + 10.0 + indent.min((rect.width() - 170.0).max(0.0));
    let leading = Rect::from_center_size(pos2(x + 12.0, center), vec2(24.0, 24.0));
    if s.marked {
        let check = leading.shrink(2.0);
        ui.painter()
            .rect_filled(check, CornerRadius::same(5), colors.accent);
        paint_icon(
            ui.painter(),
            Icon::Check,
            check.shrink(3.0),
            colors.on_accent,
        );
    } else {
        match s.content {
            Content::Image { file, .. } => {
                let texture = textures
                    .entry(file.to_string())
                    .or_insert_with(|| load_panel_image(ui.ctx(), file, [96, 96]));
                if let Some(texture) = texture {
                    ui.painter()
                        .rect_filled(leading, CornerRadius::same(5), colors.hover);
                    paint_texture(ui, texture, leading, 5);
                } else {
                    paint_icon(ui.painter(), Icon::Image, leading.shrink(5.0), colors.dim);
                }
            }
            Content::Text { text } => {
                let text = text.trim();
                let icon = if text.starts_with("https://") || text.starts_with("http://") {
                    Icon::Link
                } else {
                    Icon::Text
                };
                paint_icon(ui.painter(), icon, leading.shrink(5.0), colors.dim);
            }
        }
    }
    let mut primary_left = leading.right() + 8.0;
    if let Some(color) = s.dot {
        ui.painter()
            .circle_filled(pos2(primary_left, center), 3.0, color);
        primary_left += 10.0;
    }

    let mut right = rect.right() - 10.0;
    let command_held = ui.input(|i| i.modifiers.command);
    if s.n < 9 && command_held {
        let badge = key_label(Modifiers::COMMAND, &(s.n + 1).to_string());
        right = paint_keycap(ui, right, center, &badge) - 5.0;
    }
    let active = s.selected || hovered;
    let mut copy = false;
    let mut delete = false;
    let mut star = false;
    let mut edit = false;
    if active && !command_held {
        let del_tip = format!("Delete ({})", key_label(Modifiers::COMMAND, "⌫"));
        delete = panel_row_tool(ui, &response, &mut right, Icon::Trash, &del_tip, colors.dim);
        if s.fav.is_some() {
            let (icon, color) = if s.fav == Some(true) {
                (Icon::StarFilled, colors.accent)
            } else {
                (Icon::Star, colors.dim)
            };
            let fav_tip = format!("Choose a group ({})", key_label(Modifiers::COMMAND, "S"));
            star = panel_row_tool(ui, &response, &mut right, icon, &fav_tip, color);
        }
        let edit_tip = format!("Edit ({})", key_label(Modifiers::COMMAND, "E"));
        edit = panel_row_tool(
            ui,
            &response,
            &mut right,
            Icon::Pencil,
            &edit_tip,
            colors.dim,
        );
        let copy_tip = format!("Copy ({})", key_label(Modifiers::COMMAND, "↵"));
        copy = panel_row_tool(ui, &response, &mut right, Icon::Copy, &copy_tip, colors.dim);
    } else if !active {
        if s.fav == Some(true) {
            let badge = Rect::from_center_size(pos2(right - 5.5, center), vec2(11.0, 11.0));
            paint_icon(ui.painter(), Icon::StarFilled, badge, colors.accent);
            right -= 17.0;
        }
        if !s.meta.is_empty() {
            let width = (right - primary_left - 96.0).clamp(1.0, 112.0);
            let meta = elided(ui, s.meta, FontId::proportional(11.0), colors.dim, width, 1);
            right -= meta.size().x;
            ui.painter()
                .galley(pos2(right, center - meta.size().y / 2.0), meta, colors.dim);
            right -= 8.0;
        }
    }

    let width = (right - primary_left - 3.0).max(1.0);
    if let Some(title) = s.title.filter(|title| !title.trim().is_empty()) {
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        let galley = elided(
            ui,
            &title,
            FontId::proportional(13.0),
            colors.text,
            width,
            1,
        );
        panel_semibold(
            ui,
            pos2(primary_left, center - galley.size().y / 2.0),
            galley,
            colors.text,
        );
    } else {
        let galley = elided(
            ui,
            &s.content.preview(180),
            FontId::monospace(12.5),
            colors.text,
            width,
            1,
        );
        ui.painter().galley(
            pos2(primary_left, center - galley.size().y / 2.0),
            galley,
            colors.text,
        );
    }
    let hovered_moved = hovered && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
    RowOut {
        clicked: response.clicked(),
        response,
        copy,
        star,
        edit,
        delete,
        hovered_moved,
    }
}

pub(super) fn panel_row_fill(ui: &Ui, rect: Rect, selected: bool, hover: bool) {
    let colors = palette(ui.ctx());
    if selected || hover {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(8),
            if selected {
                colors.accent_soft
            } else {
                colors.bg2
            },
        );
    }
    if selected {
        let bar = Rect::from_min_size(
            rect.left_top() + vec2(0.0, 6.0),
            vec2(3.0, rect.height() - 12.0),
        );
        ui.painter()
            .rect_filled(bar, CornerRadius::same(2), colors.accent);
    }
}

pub(super) fn panel_row_tool(
    ui: &Ui,
    row: &egui::Response,
    right: &mut f32,
    icon: Icon,
    tip: &str,
    color: Color32,
) -> bool {
    let colors = palette(ui.ctx());
    let rect = Rect::from_center_size(pos2(*right - 10.0, row.rect.center().y), vec2(20.0, 20.0));
    *right -= 22.0;
    if icon == Icon::Trash {
        *right -= DELETE_ICON_GAP;
    }
    let response = ui.interact(rect, row.id.with(icon), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), colors.hover);
    }
    let ink = if response.hovered() && color == colors.dim {
        colors.text
    } else {
        color
    };
    paint_icon(ui.painter(), icon, rect.shrink(3.0), ink);
    response.on_hover_text(tip).clicked()
}

pub(super) fn paint_keycap(ui: &Ui, right: f32, center_y: f32, key: &str) -> f32 {
    let colors = palette(ui.ctx());
    let (text, return_icon) = key
        .strip_suffix('↵')
        .map_or((key, false), |text| (text, true));
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_string(), FontId::proportional(10.5), colors.text);
    let icon_width = if return_icon { 11.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_width + 9.0, 17.0);
    let rect = Rect::from_min_size(pos2(right - size.x, center_y - size.y / 2.0), size);
    ui.painter().rect(
        rect,
        CornerRadius::same(4),
        colors.keycap_bg,
        Stroke::new(1.0_f32, colors.line),
        StrokeKind::Inside,
    );
    let text_x = rect.left() + 4.5;
    let icon_x = text_x + galley.size().x;
    ui.painter().galley(
        pos2(text_x, center_y - galley.size().y / 2.0),
        galley,
        colors.text,
    );
    if return_icon {
        let icon = Rect::from_min_size(pos2(icon_x, center_y - 5.5), vec2(11.0, 11.0));
        paint_icon(ui.painter(), Icon::Return, icon, colors.text);
    }
    rect.left()
}

pub(super) fn panel_semibold(ui: &Ui, at: Pos2, text: Arc<egui::Galley>, color: Color32) {
    ui.painter().galley(at, text.clone(), color);
    ui.painter().galley(at + vec2(0.32, 0.0), text, color);
}

pub(super) fn section_title(ui: &Ui, rect: Rect, label: &str) {
    let colors = palette(ui.ctx());
    let mut job = egui::text::LayoutJob::default();
    let mut format = egui::TextFormat::simple(FontId::proportional(10.5), colors.dim);
    format.extra_letter_spacing = 0.85;
    job.append(label, 0.0, format);
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    panel_semibold(
        ui,
        pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        colors.dim,
    );
}

pub(super) fn search_field(
    ui: &mut Ui,
    id: &'static str,
    value: &mut String,
    placeholder: &str,
    width: f32,
    focus: bool,
) -> egui::Response {
    let colors = palette(ui.ctx());
    Frame::NONE
        .fill(colors.bg2)
        .stroke(Stroke::new(1.0_f32, colors.line))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 7.0;
                icon_label(ui, Icon::Search, 16.0, colors.dim);
                let id = Id::new(id);
                if focus && !ui.memory(|memory| memory.has_focus(id)) {
                    ui.memory_mut(|memory| memory.request_focus(id));
                }
                ui.add(
                    TextEdit::singleline(value)
                        .id(id)
                        .hint_text(RichText::new(placeholder).color(colors.dim))
                        .font(FontId::proportional(13.0))
                        .frame(false)
                        .desired_width((width - 43.0).max(1.0)),
                )
            })
            .inner
        })
        .inner
}

pub(super) fn segmented_control<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: &'static str,
    choices: &[(&str, T)],
    current: T,
) -> Option<T> {
    let colors = palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(
        vec2(6.0 + 75.0 * choices.len() as f32, 32.0),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, CornerRadius::same(10), colors.hover);
    let mut selection = None;
    for (index, (label, value)) in choices.iter().copied().enumerate() {
        let segment = Rect::from_min_size(
            rect.min + vec2(3.0 + 75.0 * index as f32, 3.0),
            vec2(75.0, 26.0),
        );
        let response = ui.interact(segment, ui.id().with((id, index)), Sense::click());
        let selected = value == current;
        if selected {
            ui.painter().rect_filled(
                segment.translate(vec2(0.0, 1.0)),
                CornerRadius::same(7),
                colors.segment_shadow,
            );
            ui.painter()
                .rect_filled(segment, CornerRadius::same(7), colors.bg2);
        }
        let color = if selected || response.hovered() {
            colors.text
        } else {
            colors.dim
        };
        ui.painter().text(
            segment.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.5),
            color,
        );
        if response.clicked() {
            selection = Some(value);
        }
    }
    selection
}

fn load_panel_image(ctx: &egui::Context, file: &str, max: [u32; 2]) -> Option<TextureHandle> {
    let img = image::open(store::image_path(file).ok()?).ok()?;
    let rgba = img.thumbnail(max[0], max[1]).to_rgba8();
    let (w, h) = rgba.dimensions();
    let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    Some(ctx.load_texture(file, img, Default::default()))
}

pub(super) fn preview_texture<'a>(
    ctx: &egui::Context,
    textures: &'a mut HashMap<String, Option<TextureHandle>>,
    file: &str,
    size: egui::Vec2,
) -> Option<&'a TextureHandle> {
    let scale = ctx.pixels_per_point();
    let physical = [
        size.x.mul_add(scale, 0.0).ceil() as u32,
        size.y.mul_add(scale, 0.0).ceil() as u32,
    ];
    let key = format!("preview:{}x{}:{file}", physical[0], physical[1]);
    // Preview pixels are larger than row thumbnails. Keep only the currently requested preview.
    textures.retain(|id, _| !id.starts_with("preview:") || id == &key);
    textures
        .entry(key)
        .or_insert_with(|| load_panel_image(ctx, file, physical))
        .as_ref()
}

pub(super) fn paint_texture(ui: &Ui, texture: &TextureHandle, bounds: Rect, radius: u8) {
    let original = texture.size_vec2();
    let size = original
        * (bounds.width() / original.x)
            .min(bounds.height() / original.y)
            .min(1.0);
    egui::Image::from_texture(texture)
        .corner_radius(radius)
        .paint_at(ui, Rect::from_center_size(bounds.center(), size));
}

pub(super) fn group_editor(
    ui: &mut Ui,
    e: &mut GroupEdit,
    save: &mut bool,
    cancel: &mut bool,
    recording: bool,
    record: &mut bool,
) {
    let colors = palette(ui.ctx());
    Frame::NONE
        .fill(colors.bg2)
        .stroke(Stroke::new(1.0f32, colors.line))
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new(("gedit", e.id))
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Group name").color(colors.dim));
                    let r = ui.add(TextEdit::singleline(&mut e.name).desired_width(200.0));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        *save = true;
                    }
                    ui.end_row();
                    ui.label(RichText::new("Shortcut").color(colors.dim));
                    if hotkey_field(ui, &mut e.hotkey, recording, true) {
                        *record = true;
                    }
                    ui.end_row();
                    ui.label(RichText::new("Color").color(colors.dim));
                    ui.horizontal(|ui| {
                        for c in PALETTE {
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                            ui.painter().circle_filled(r.center(), 7.5, rgb(c));
                            if e.color == c {
                                ui.painter().circle_stroke(
                                    r.center(),
                                    9.0,
                                    Stroke::new(1.5f32, colors.text),
                                );
                            }
                            if resp.clicked() {
                                e.color = c;
                            }
                        }
                    });
                    ui.end_row();
                });
            ui.horizontal(|ui| {
                if accent_button(ui, "Save").clicked() {
                    *save = true;
                }
                if ui.button("Cancel").clicked() {
                    *cancel = true;
                }
            });
        });
}

/// Shows a shortcut as a chip with a button to record a new one.
/// Returns true when the record button was pressed.
pub(super) fn hotkey_field(
    ui: &mut Ui,
    text: &mut String,
    recording: bool,
    optional: bool,
) -> bool {
    let colors = palette(ui.ctx());
    let mut clicked = false;
    ui.horizontal(|ui| {
        let (label, col, fill) = if recording {
            (
                "Press keys… (Esc cancels)".to_string(),
                colors.on_accent,
                colors.accent,
            )
        } else if text.is_empty() {
            ("None".to_string(), colors.dim, colors.bg)
        } else {
            (text.clone(), colors.text, colors.bg)
        };
        Frame::NONE
            .fill(fill)
            .stroke(Stroke::new(1.0f32, colors.line))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.set_min_width(130.0);
                ui.label(RichText::new(label).size(12.5).color(col));
            });
        if !recording && small_button(ui, "Change").clicked() {
            clicked = true;
        }
        if optional && !recording && !text.is_empty() && small_button(ui, "Remove").clicked() {
            *text = String::new();
        }
    });
    clicked
}

/// Turns a key press into text such as "Cmd+Shift+V". None for keys that need a modifier.
pub(super) fn hotkey_text(key: Key, m: Modifiers) -> Option<String> {
    let name = key.name();
    let is_f =
        name.len() >= 2 && name.starts_with('F') && name[1..].chars().all(|c| c.is_ascii_digit());
    if !(m.mac_cmd || m.ctrl || m.alt || m.shift) && !is_f {
        return None;
    }
    let mut parts = Vec::new();
    if m.mac_cmd {
        parts.push("Cmd");
    }
    if m.ctrl {
        parts.push("Ctrl");
    }
    if m.alt {
        parts.push("Alt");
    }
    if m.shift {
        parts.push("Shift");
    }
    let mut out = parts.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(name);
    Some(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeyStyle {
    Mac,
    Windows,
}

impl KeyStyle {
    pub(super) const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Windows
        }
    }
}

/// Formats a shortcut for UI display in either macOS symbol style (`⌘⇧V`) or Windows word style
/// (`Ctrl+Shift+V`).
pub(super) fn format_key_label(style: KeyStyle, mods: Modifiers, key: &str) -> String {
    match style {
        KeyStyle::Mac => {
            let mut out = String::new();
            if mods.command || mods.mac_cmd {
                out.push('⌘');
            }
            if mods.ctrl {
                out.push('⌃');
            }
            if mods.alt {
                out.push('⌥');
            }
            if mods.shift {
                out.push('⇧');
            }
            let mapped = match key {
                "Backspace" => "⌫",
                "Enter" => "↵",
                other => other,
            };
            out.push_str(mapped);
            out
        }
        KeyStyle::Windows => {
            let mut parts: Vec<&str> = Vec::new();
            if mods.ctrl || mods.command {
                parts.push("Ctrl");
            }
            if mods.alt {
                parts.push("Alt");
            }
            if mods.shift {
                parts.push("Shift");
            }
            if mods.mac_cmd {
                parts.push("Win");
            }
            let mapped = match key {
                "⌫" => "Backspace",
                "↵" | "↩" => "Enter",
                other => other,
            };
            if !mapped.is_empty() {
                parts.push(mapped);
            }
            parts.join("+")
        }
    }
}

pub(super) fn key_label(mods: Modifiers, key: &str) -> String {
    format_key_label(KeyStyle::current(), mods, key)
}

/// True when only the platform's primary command modifier is held (Cmd on macOS, Ctrl elsewhere).
pub(super) fn command_only(mods: Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        mods.mac_cmd && !mods.alt && !mods.shift && !mods.ctrl
    } else {
        mods.ctrl && !mods.alt && !mods.shift && !mods.mac_cmd
    }
}

#[cfg(test)]
mod key_label_tests {
    use super::*;

    #[test]
    fn key_label_formats_macos_symbols_and_windows_words() {
        assert_eq!(
            format_key_label(KeyStyle::Mac, Modifiers::COMMAND | Modifiers::SHIFT, "V"),
            "⌘⇧V"
        );
        assert_eq!(
            format_key_label(
                KeyStyle::Windows,
                Modifiers::COMMAND | Modifiers::SHIFT,
                "V"
            ),
            "Ctrl+Shift+V"
        );
        assert_eq!(
            format_key_label(KeyStyle::Mac, Modifiers::CTRL | Modifiers::ALT, "F"),
            "⌃⌥F"
        );
        assert_eq!(
            format_key_label(KeyStyle::Windows, Modifiers::CTRL | Modifiers::ALT, "F"),
            "Ctrl+Alt+F"
        );
        assert_eq!(
            format_key_label(KeyStyle::Mac, Modifiers::COMMAND, "K"),
            "⌘K"
        );
        assert_eq!(
            format_key_label(KeyStyle::Windows, Modifiers::COMMAND, "K"),
            "Ctrl+K"
        );
        assert_eq!(
            format_key_label(KeyStyle::Mac, Modifiers::COMMAND, "⌫"),
            "⌘⌫"
        );
        assert_eq!(
            format_key_label(KeyStyle::Windows, Modifiers::COMMAND, "⌫"),
            "Ctrl+Backspace"
        );
        assert_eq!(
            format_key_label(KeyStyle::Mac, Modifiers::COMMAND, "↵"),
            "⌘↵"
        );
        assert_eq!(
            format_key_label(KeyStyle::Windows, Modifiers::COMMAND, "↵"),
            "Ctrl+Enter"
        );
        assert_eq!(format_key_label(KeyStyle::Mac, Modifiers::NONE, "↵"), "↵");
        assert_eq!(
            format_key_label(KeyStyle::Windows, Modifiers::NONE, "↵"),
            "Enter"
        );
        assert_eq!(format_key_label(KeyStyle::Mac, Modifiers::COMMAND, ""), "⌘");
        assert_eq!(
            format_key_label(KeyStyle::Windows, Modifiers::COMMAND, ""),
            "Ctrl"
        );
    }
}
