//! Favorite and snippet dialogs and the settings page.

use super::*;
use crate::snippet;

pub(crate) struct SnippetPrompt {
    pub title: String,
    templates: Vec<String>,
    pub fields: Vec<(String, String)>,
    pub focus: usize,
    opening: bool,
}

impl SnippetPrompt {
    pub(crate) fn new(title: &str, templates: Vec<String>, last: &HashMap<String, String>) -> Self {
        let mut defaults = HashMap::new();
        let mut names = Vec::new();
        for template in &templates {
            for var in snippet::vars(template) {
                let default = defaults.entry(var.name.clone()).or_insert_with(|| {
                    names.push(var.name.clone());
                    None
                });
                if default.is_none() {
                    *default = var.default;
                }
            }
        }
        let fields = names.into_iter().map(|name| {
            let value = last.get(&name).cloned()
                .or_else(|| defaults.remove(&name).flatten()).unwrap_or_default();
            (name, value)
        }).collect();
        Self {
            title: title.to_owned(),
            templates,
            fields,
            focus: 0,
            opening: true,
        }
    }

    fn preview(&self, font_id: FontId, color: Color32, accent: Color32) -> egui::text::LayoutJob {
        let values: HashMap<_, _> = self.fields.iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        let normal = egui::TextFormat::simple(font_id.clone(), color);
        let unfilled = egui::TextFormat::simple(font_id, accent);
        let mut job = egui::text::LayoutJob::default();
        for (index, template) in self.templates.iter().enumerate() {
            if index > 0 {
                job.append("\n", 0.0, normal.clone());
            }
            let mut end = 0;
            for placeholder in snippet::placeholders(template) {
                job.append(&template[end..placeholder.range.start], 0.0, normal.clone());
                match values.get(placeholder.name).filter(|value| !value.is_empty()) {
                    Some(value) => job.append(value, 0.0, normal.clone()),
                    None => job.append(&format!("{{{{{}}}}}", placeholder.name), 0.0, unfilled.clone()),
                }
                end = placeholder.range.end;
            }
            job.append(&template[end..], 0.0, normal.clone());
        }
        job
    }
}

pub(crate) enum SnippetOutcome {
    Pending,
    Submit(Vec<(String, String)>),
    Cancel,
}

/// Draw each frame while active. Discard the prompt after Submit or Cancel.
pub(crate) fn snippet_prompt(ctx: &egui::Context, p: &mut SnippetPrompt, action: &str) -> SnippetOutcome {
    let colors = palette(ctx);
    let opening = std::mem::take(&mut p.opening);
    let (cancel_key, submit_key, backward, forward) = ctx.input_mut(|input| {
        if opening {
            // Space and unmodified wheel digits also emit text when activating an item.
            input.events.retain(|event| !matches!(event,
                egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Ime(_)));
        }
        let cancel = input.consume_key(Modifiers::NONE, Key::Escape);
        let submit = input.consume_key(Modifiers::NONE, Key::Enter);
        let backward = input.count_and_consume_key(Modifiers::SHIFT, Key::Tab);
        let forward = input.count_and_consume_key(Modifiers::NONE, Key::Tab);
        (cancel, submit, backward, forward)
    });

    let tabbing = forward + backward > 0;
    let count = p.fields.len();
    if count > 0 {
        p.focus = (p.focus.min(count - 1) + forward % count + count - backward % count) % count;
    }
    if tabbing {
        // egui processes Tab for focus traversal before the key can be consumed.
        // Clear its starting focus and apply our wrapped field index after layout.
        ctx.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
        });
    }

    let mut cancel = cancel_key;
    // The Enter that opens the prompt may still be present in its first frame.
    let mut submit = submit_key && !opening;
    egui::Modal::new(Id::new("snippet_prompt"))
        .frame(Frame::window(&ctx.style()))
        .show(ctx, |ui| {
            let width = (ctx.screen_rect().width() - 32.0).clamp(0.0, 340.0);
            ui.set_width(width);
            let title = if p.title.trim().is_empty() {
                "Fill snippet"
            } else {
                &p.title
            };
            ui.heading(title);
            ui.add_space(6.0);
            let focus_field = ScrollArea::vertical()
                .id_salt("snippet_fields")
                .max_height(210.0)
                .show(ui, |ui| {
                    ui.visuals_mut().selection.stroke = Stroke::new(1.5_f32, colors.accent);
                    let mut focus_field = None;
                    for (index, (name, value)) in p.fields.iter_mut().enumerate() {
                        let label = ui.label(RichText::new(name.as_str()).size(12.0).color(colors.dim));
                        let id = Id::new(("snippet_field", index));
                        if opening && !tabbing && index == p.focus {
                            ui.memory_mut(|memory| memory.request_focus(id));
                        }
                        let field = ui
                            .add(
                                TextEdit::singleline(value)
                                    .id(id)
                                    .font(egui::TextStyle::Monospace)
                                    .background_color(colors.hover)
                                    .desired_width(f32::INFINITY),
                            )
                            .labelled_by(label.id);
                        if (opening || tabbing) && index == p.focus {
                            field.scroll_to_me(None);
                            focus_field = Some(field);
                        } else if !tabbing && field.has_focus() {
                            p.focus = index;
                        }
                        ui.add_space(6.0);
                    }
                    focus_field
                })
                .inner;

            ui.label(RichText::new("Preview").size(12.0).color(colors.dim));
            ScrollArea::vertical()
                .id_salt("snippet_preview")
                .max_height(110.0)
                .show(ui, |ui| {
                    let font = egui::TextStyle::Monospace.resolve(ui.style());
                    let preview = p.preview(font, ui.visuals().text_color(), colors.accent);
                    ui.add(egui::Label::new(preview).wrap());
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                submit |= accent_button(ui, action).clicked();
                cancel |= ui.button("Cancel").clicked();
            });
            ui.label(RichText::new(format!("Tab next · Enter {action} · Esc cancel")).size(11.0).color(colors.dim));

            if let Some(field) = focus_field && !field.has_focus() {
                field.request_focus();
            }
        });

    if cancel {
        SnippetOutcome::Cancel
    } else if submit {
        SnippetOutcome::Submit(p.fields.clone())
    } else {
        SnippetOutcome::Pending
    }
}

impl App {
    pub(super) fn editor_window(&mut self, ctx: &egui::Context) {
        let colors = palette(ctx);
        let Some(ed) = self.editor.as_mut() else { return };
        let mut save = false;
        let mut cancel = false;
        let groups: Vec<(u64, String)> = self.store.groups.iter().map(|g| (g.id, self.store.group_path(g.id))).collect();
        let title = if ed.id.is_some() { "Edit favorite" } else { "Add favorite" };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([340.0, 0.0])
            .show(ctx, |ui| {
                ui.label(RichText::new("Title (shown on the wheel)").size(12.0).color(colors.dim));
                ui.add(TextEdit::singleline(&mut ed.title).desired_width(f32::INFINITY).hint_text("E.g. email signature"));
                ui.add_space(6.0);
                ui.label(RichText::new("Group").size(12.0).color(colors.dim));
                let current = groups.iter().find(|(id, _)| *id == ed.group).map(|(_, n)| n.clone()).unwrap_or_default();
                egui::ComboBox::from_id_salt("fav_group").selected_text(current).width(320.0).show_ui(ui, |ui| {
                    for (id, name) in &groups {
                        ui.selectable_value(&mut ed.group, *id, name);
                    }
                });
                ui.add_space(6.0);
                match &ed.content {
                    Content::Text { .. } => {
                        ui.label(RichText::new("Content").size(12.0).color(colors.dim));
                        ui.add(TextEdit::multiline(&mut ed.text).desired_rows(5).desired_width(f32::INFINITY));
                    }
                    Content::Image { w, h, .. } => {
                        ui.label(RichText::new(format!("Content: image {w}×{h}")).color(colors.dim));
                    }
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if accent_button(ui, "Save").clicked() {
                        save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            self.editor = None;
        }
        if !save {
            return;
        }
        let Some(ed) = self.editor.take() else { return };
        let content = match ed.content {
            Content::Text { .. } => Content::Text { text: ed.text.clone() },
            other => other,
        };
        if let Content::Text { text } = &content && text.is_empty() {
            return;
        }
        let title = if ed.title.trim().is_empty() { content.preview(28) } else { ed.title.trim().to_string() };
        self.last_group = Some(ed.group);
        match ed.id {
            Some(id) => {
                if let Some(f) = self.store.favorites.iter_mut().find(|f| f.id == id) {
                    f.title = title;
                    f.content = content;
                    f.group = ed.group;
                }
            }
            None => {
                let id = self.store.new_id();
                self.store.favorites.push(Favorite { id, group: ed.group, title, content, lower: String::new(), ..Default::default() });
            }
        }
        self.store.reindex();
        self.dirty = true;
    }

    pub(super) fn settings_page(&mut self, ui: &mut Ui) {
        let colors = palette(ui.ctx());
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Appearance");
                let current = self.shot_theme.unwrap_or(self.store.settings.theme);
                if let Some(theme) = segmented_control(ui, "appearance", &[
                    ("System", Theme::System), ("Light", Theme::Light), ("Dark", Theme::Dark),
                ], current) {
                    self.dirty |= self.store.settings.theme != theme;
                    self.store.settings.theme = theme;
                    ui.ctx().request_repaint();
                }
            });

            section(ui, "History");
            ui.horizontal(|ui| {
                ui.label("Items to keep");
                let s = &mut self.store.settings;
                if ui.add(egui::Slider::new(&mut s.history_limit, store::HISTORY_MIN..=store::HISTORY_MAX).logarithmic(true).step_by(50.0)).changed() {
                    self.dirty = true;
                }
            });
            if self.shot.is_none() && self.store.history.len() > self.store.settings.history_limit {
                self.store.trim();
            }
            let s = &mut self.store.settings;
            if ui.checkbox(&mut s.skip_concealed, "Skip passwords, temporary copies, and automatically generated copies").changed() {
                self.skip_concealed.store(s.skip_concealed, Ordering::Relaxed);
                self.dirty = true;
            }
            ui.add_space(4.0);
            ui.label("When copying the same item again");
            let before = s.duplicates;
            ui.radio_value(&mut s.duplicates, store::Duplicates::NotWhenOnTop, "Skip if it matches the latest item");
            ui.radio_value(&mut s.duplicates, store::Duplicates::Never, "Skip if it’s already in history");
            ui.radio_value(&mut s.duplicates, store::Duplicates::Allow, "Always add a new item");
            if s.duplicates != before {
                self.dirty = true;
            }
            if ui.checkbox(&mut s.move_used_to_top, "Move pasted items to the top").changed() {
                self.dirty = true;
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let label = if self.confirm_clear { "Click again to confirm" } else { "Clear history" };
                if ui.button(label).clicked() && self.shot.is_none() {
                    if self.confirm_clear {
                        self.store.clear_history();
                        self.dirty = true;
                    }
                    self.confirm_clear = !self.confirm_clear;
                }
                ui.label(RichText::new("Favorites are kept.").size(11.5).color(colors.dim));
            });

            section(ui, "Shortcuts");
            let mut rec = None;
            egui::Grid::new("hotkeys").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                let s = &mut self.store.settings;
                ui.label("Show or hide the panel");
                if hotkey_field(ui, &mut s.hotkey_panel, self.recording == Some(RecTarget::Panel), false) {
                    rec = Some(RecTarget::Panel);
                }
                ui.end_row();
                ui.label("Open favorites wheel");
                if hotkey_field(ui, &mut s.hotkey_wheel, self.recording == Some(RecTarget::Wheel), false) {
                    rec = Some(RecTarget::Wheel);
                }
                ui.end_row();
                ui.label("Open favorites list");
                if hotkey_field(ui, &mut s.hotkey_favorites, self.recording == Some(RecTarget::Favorites), false) {
                    rec = Some(RecTarget::Favorites);
                }
                ui.end_row();
            });
            if let Some(t) = rec {
                self.start_recording(t);
            }
            ui.label(
                RichText::new("Click Change, then press the new shortcut (e.g. Cmd+Shift+V). To set a group shortcut, click its ✏ button in Favorites.")
                    .size(11.5)
                    .color(colors.dim),
            );
            ui.horizontal(|ui| {
                if self.hotkey_errors.is_empty() && self.hotkeys.mgr.is_some() {
                    ui.label(RichText::new("✔ Shortcuts are active").size(11.5).color(colors.success));
                }
            });
            for e in &self.hotkey_errors {
                ui.label(RichText::new(format!("⚠ {e}")).size(11.5).color(colors.error));
            }

            section(ui, "Pasting");
            let s = &mut self.store.settings;
            if ui.checkbox(&mut s.auto_paste, "Automatically paste into the active app").changed() {
                self.dirty = true;
            }
            if ui.checkbox(&mut s.hide_on_blur, "Hide the panel when clicking outside").changed() {
                self.dirty = true;
            }
            let ok = platform::accessibility_ok();
            ui.horizontal(|ui| {
                if ok {
                    ui.label(RichText::new("✔ Accessibility permission granted").size(12.0).color(colors.success));
                } else {
                    let warn = "⚠ Auto-paste needs Accessibility permission";
                    ui.label(RichText::new(warn).size(12.0).color(colors.warning));
                    if ui.button("Grant access…").clicked() {
                        platform::open_accessibility_settings();
                    }
                }
            });

            section(ui, "Data");
            ui.label(RichText::new("Exports contain favorites in plain text. Clipboard history stays on this Mac.")
                .size(11.5).color(colors.warning));
            ui.horizontal(|ui| {
                if small_button(ui, "Export settings…").clicked() && self.shot.is_none() {
                    let path = rfd::FileDialog::new()
                        .set_title(format!("Export {} settings", crate::DISPLAY_NAME))
                        .add_filter("JSON", &["json"])
                        .set_file_name("copywheel-settings.json")
                        .save_file();
                    self.was_focused = false;
                    ui.ctx().send_viewport_cmd(ViewportCommand::Focus);
                    if let Some(path) = path {
                        self.backup_note = Some(match self.store.export_config(&path) {
                            Ok(()) => {
                                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                                (true, format!("Exported {name}"))
                            }
                            Err(e) => (false, e),
                        });
                    }
                }
                if small_button(ui, "Import settings…").clicked() && self.shot.is_none() {
                    let path = rfd::FileDialog::new()
                        .set_title(format!("Import {} settings", crate::DISPLAY_NAME))
                        .add_filter("JSON", &["json"])
                        .pick_file();
                    self.was_focused = false;
                    ui.ctx().send_viewport_cmd(ViewportCommand::Focus);
                    if let Some(path) = path {
                        match self.store.import_config(&path) {
                            Ok(res) => {
                                self.skip_concealed.store(self.store.settings.skip_concealed, Ordering::Relaxed);
                                self.reapply_hotkeys();
                                self.store.save();
                                self.dirty = false;
                                self.backup_note = Some((
                                    true,
                                    format!("Imported {} groups and {} favorites.", res.groups_added, res.favorites_added),
                                ));
                            }
                            Err(e) => {
                                self.backup_note = Some((false, e));
                            }
                        }
                    }
                }
            });
            if let Some((ok, note)) = &self.backup_note {
                let col = if *ok { colors.success } else { colors.error };
                ui.label(RichText::new(note).size(11.5).color(col));
            }
            let dir = store::data_dir();
            ui.label(RichText::new(dir.display().to_string()).size(11.5).color(colors.dim));
            if ui.button("Open folder").clicked() && self.shot.is_none()
                && store::create_private_dir(&dir).is_ok()
            {
                platform::open_folder(&dir);
            }
        });
    }
}

#[cfg(test)]
mod snippet_prompt_tests {
    use super::*;

    #[test]
    fn batch_prefills_shared_fields_in_first_use_order() {
        let prompt = SnippetPrompt::new(
            "Favorites",
            vec!["{{host}} {{port:53}}".into(), "{{host:local}} {{port:54}}".into()],
            &HashMap::from([("port".into(), "853".into())]),
        );
        assert_eq!(prompt.fields, [("host".into(), "local".into()), ("port".into(), "853".into())]);
    }

    #[test]
    fn opening_shortcut_text_does_not_edit_a_reopened_prompt() {
        let ctx = egui::Context::default();
        let mut prompt = SnippetPrompt::new("Connect", vec!["{{host:local}}".into()], &HashMap::new());
        let render = |prompt: &mut SnippetPrompt, events| {
            let mut outcome = SnippetOutcome::Pending;
            let _ = ctx.run(egui::RawInput { events, ..Default::default() }, |ctx| {
                outcome = snippet_prompt(ctx, prompt, "Copy");
            });
            outcome
        };
        render(&mut prompt, vec![]);
        render(&mut prompt, vec![]);
        let escape = egui::Event::Key {
            key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE,
        };
        assert!(matches!(render(&mut prompt, vec![escape]), SnippetOutcome::Cancel));

        let mut next = SnippetPrompt::new("Connect", vec!["{{host:local}}".into()], &HashMap::new());
        render(&mut next, vec![egui::Event::Text("2".into())]);
        assert_eq!(next.fields[0].1, "local");
    }

    #[test]
    fn snippet_renders_unfilled_variables_and_focus_in_accent() {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let mut prompt = SnippetPrompt::new(
            "Connect",
            vec!["λ {{host}} {{port:53}} {{host:}}".into(), "{{literal}}".into()],
            &HashMap::from([("literal".into(), "{{host}}".into())]),
        );
        let render = |prompt: &mut SnippetPrompt| {
            ctx.run(egui::RawInput::default(), |ctx| {
                snippet_prompt(ctx, prompt, "Paste");
            })
        };
        render(&mut prompt);
        let output = render(&mut prompt);
        let colors = palette(&ctx);
        let preview = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text.starts_with("λ ") => Some(&text.galley.job),
            _ => None,
        }).expect("preview text");
        assert_eq!(preview.text, "λ {{host}} 53 {{host}}\n{{host}}");
        let highlighted: Vec<_> = preview.sections.iter()
            .filter(|section| section.format.color == colors.accent)
            .map(|section| &preview.text[section.byte_range.clone()])
            .collect();
        assert_eq!(highlighted, ["{{host}}", "{{host}}"]);
        assert_eq!(output.shapes.iter().filter(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke == Stroke::new(1.5_f32, colors.accent))
        }).count(), 1);
    }

    #[test]
    fn snippet_keyboard_cycles_fields_and_submits_the_edited_values() {
        let ctx = egui::Context::default();
        let mut prompt = SnippetPrompt::new(
            "Connect",
            vec!["{{host:local}}:{{port:53}}".into()],
            &HashMap::from([("host".into(), "remote".into())]),
        );
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let mut frame = |events: Vec<egui::Event>| {
            let mut outcome = SnippetOutcome::Pending;
            let _ = ctx.run(egui::RawInput { events, ..Default::default() }, |ctx| {
                outcome = snippet_prompt(ctx, &mut prompt, "Paste");
            });
            outcome
        };
        let focused = || ctx.memory(|memory| memory.focused());

        let enter = key(Key::Enter, Modifiers::NONE);
        assert!(matches!(frame(vec![enter.clone()]), SnippetOutcome::Pending));
        assert_eq!(focused(), Some(Id::new(("snippet_field", 0usize))));

        frame(vec![key(Key::Tab, Modifiers::SHIFT)]);
        assert_eq!(focused(), Some(Id::new(("snippet_field", 1usize))));
        frame(vec![key(Key::Tab, Modifiers::NONE)]);
        assert_eq!(focused(), Some(Id::new(("snippet_field", 0usize))));
        frame(vec![key(Key::Tab, Modifiers::NONE)]);
        assert_eq!(focused(), Some(Id::new(("snippet_field", 1usize))));

        let outcome = frame(vec![egui::Event::Text("0".into()), enter]);
        let SnippetOutcome::Submit(values) = outcome else {
            panic!("Enter should submit the snippet");
        };
        let colors = palette(&ctx);
        assert_eq!(prompt.preview(FontId::monospace(12.0), colors.text, colors.accent).text, "remote:530");
        assert_eq!(
            values,
            [("host".into(), "remote".into()), ("port".into(), "530".into())]
        );
    }
}
