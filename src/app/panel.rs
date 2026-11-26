//! The floating panel and its keyboard-driven menus.

use super::*;

const LIST_WIDTH: f32 = 340.0;
const TOP_HEIGHT: f32 = 40.0;
const ACTION_HEIGHT: f32 = 28.0;
const SECTION_HEIGHT: f32 = 22.0;
const GROUP_HEIGHT: f32 = 30.0;
const ROW_GAP: f32 = 2.0;
const MENU_WIDTH: f32 = 280.0;

#[derive(Clone, Copy)]
enum PanelRow {
    Clip(usize),
    Favorite(usize, usize),
    Group(u64, usize),
}

impl PanelRow {
    fn source(self, store: &Store) -> Option<Src> {
        match self {
            Self::Clip(index) => Some(Src::Clip(store.history[index].id)),
            Self::Favorite(index, _) => Some(Src::Fav(store.favorites[index].id)),
            Self::Group(..) => None,
        }
    }
}

enum ListEntry {
    Section(&'static str),
    Row(PanelRow),
}

struct PanelList {
    entries: Vec<ListEntry>,
    rows: Vec<PanelRow>,
}

impl PanelList {
    fn new(entries: Vec<ListEntry>) -> Self {
        let rows = entries.iter().filter_map(|entry| match entry {
            ListEntry::Row(row) => Some(*row),
            ListEntry::Section(_) => None,
        }).collect();
        Self { entries, rows }
    }

    fn numbered(&self, index: usize, store: &Store) -> Option<Src> {
        self.rows.iter().filter_map(|row| row.source(store)).nth(index)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PopupKind {
    Actions,
    Groups,
}

pub(super) struct PanelPopup {
    kind: PopupKind,
    filter: String,
    selected: usize,
    scroll_to_selected: bool,
    rendered: bool,
}

#[derive(Clone, Copy)]
enum Command {
    Paste,
    Copy,
    Favorite,
    Edit,
    Delete,
    Undo,
    NewFavorite,
    NewGroup,
    OpenWheel,
    Settings,
}

impl Command {
    fn label(self) -> &'static str {
        match self {
            Self::Paste => "Paste",
            Self::Copy => "Copy",
            Self::Favorite => "Add to favorites…",
            Self::Edit => "Edit",
            Self::Delete => "Delete",
            Self::Undo => "Undo delete",
            Self::NewFavorite => "New favorite",
            Self::NewGroup => "New group",
            Self::OpenWheel => "Open wheel",
            Self::Settings => "Settings",
        }
    }

    fn shortcut(self) -> &'static str {
        match self {
            Self::Paste => "↵",
            Self::Copy => "⌘↵",
            Self::Favorite => "⌘S",
            Self::Edit => "⌘E",
            Self::Delete => "⌘⌫",
            Self::Undo => "⌘Z",
            Self::Settings => "⌘,",
            _ => "",
        }
    }
}

const COMMANDS: [Command; 10] = [
    Command::Paste, Command::Copy, Command::Favorite, Command::Edit, Command::Delete,
    Command::Undo, Command::NewFavorite, Command::NewGroup, Command::OpenWheel, Command::Settings,
];

enum Choice {
    Command(Command),
    Group(u64),
    RemoveFavorite,
}

struct MenuItem {
    choice: Choice,
    label: String,
    badge: &'static str,
    enabled: bool,
    score: i32,
}

enum ListAction {
    Click(usize),
    Copy(usize),
    Edit(usize),
    Delete(usize),
    Favorite(usize),
    Group(Action, usize),
}

#[derive(Debug, PartialEq, Eq)]
enum ClickAction {
    Select,
    Paste,
    RangeMark,
    ToggleMark,
}

fn click_action(double_clicked: bool, mods: Modifiers) -> ClickAction {
    if mods.shift {
        ClickAction::RangeMark
    } else if mods.command {
        ClickAction::ToggleMark
    } else if double_clicked {
        ClickAction::Paste
    } else {
        ClickAction::Select
    }
}

impl App {
    fn panel_list(&self) -> PanelList {
        let searching = !self.query.trim().is_empty();
        let entries = match self.tab {
            Tab::History => {
                let indices = self.history_matches();
                if searching || indices.is_empty() {
                    indices.into_iter().map(|index| ListEntry::Row(PanelRow::Clip(index))).collect()
                } else {
                    let cuts = history_cutoffs(store::now());
                    let mut sections: [Vec<usize>; 4] = Default::default();
                    for index in indices {
                        let created = self.store.history[index].created;
                        let section = cuts.iter().position(|&cut| created >= cut).unwrap_or(3);
                        sections[section].push(index);
                    }
                    ["TODAY", "YESTERDAY", "THIS WEEK", "OLDER"].into_iter().zip(sections)
                        .filter(|(_, indices)| !indices.is_empty())
                        .flat_map(|(title, indices)| std::iter::once(ListEntry::Section(title))
                            .chain(indices.into_iter().map(|index| ListEntry::Row(PanelRow::Clip(index)))))
                        .collect()
                }
            }
            Tab::Favorites if searching => self.favorite_search_matches().into_iter()
                .map(|index| ListEntry::Row(PanelRow::Favorite(index, 0)))
                .collect(),
            Tab::Favorites => {
                let mut favorites: HashMap<_, _> = self.favorite_matches().into_iter().collect();
                let mut entries = Vec::new();
                for (group, depth) in self.group_order() {
                    if self.group_listed(group) {
                        entries.push(ListEntry::Row(PanelRow::Group(group, depth)));
                        if let Some(indices) = favorites.remove(&group) {
                            entries.extend(indices.into_iter()
                                .map(|index| ListEntry::Row(PanelRow::Favorite(index, depth + 1))));
                        }
                    }
                }
                entries
            }
            Tab::Settings => Vec::new(),
        };
        PanelList::new(entries)
    }

    fn selected_source(&self, list: &PanelList) -> Option<Src> {
        list.rows.get(self.sel)?.source(&self.store)
    }

    fn marked_sources(&self, list: &PanelList) -> Vec<Src> {
        if self.marks.is_empty() {
            return Vec::new();
        }
        let marked_id = |source| match source {
            Src::Clip(id) | Src::Fav(id) if self.marks.contains(&id) => Some(id),
            _ => None,
        };
        let mut visible = HashSet::new();
        let mut sources: Vec<_> = list.rows.iter().filter_map(|row| row.source(&self.store))
            .filter(|&source| marked_id(source).is_some_and(|id| visible.insert(id)))
            .collect();
        // Collapsing a group leaves its marked favorites in the bulk selection.
        if self.tab == Tab::Favorites && visible.len() < self.marks.len() {
            sources.extend(self.store.favorites.iter()
                .filter(|f| self.marks.contains(&f.id) && !visible.contains(&f.id))
                .map(|f| Src::Fav(f.id)));
        }
        sources
    }

    fn action_sources(&self, list: &PanelList) -> Vec<Src> {
        if self.marks.is_empty() {
            self.selected_source(list).into_iter().collect()
        } else {
            self.marked_sources(list)
        }
    }

    pub(super) fn switch_panel_tab(&mut self) {
        self.set_panel_tab(if self.tab == Tab::History { Tab::Favorites } else { Tab::History });
    }

    fn set_panel_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.panel_popup = None;
        self.reset_panel_selection();
    }

    fn reset_panel_selection(&mut self) {
        self.clear_marks();
        self.sel = 0;
        self.scroll_want = Some(0.0);
    }

    pub(super) fn panel_keys(&mut self, ctx: &egui::Context) {
        if self.pending_snippet.is_some() {
            return;
        }
        if self.recording.is_some() {
            self.record_keys(ctx);
            return;
        }
        let escape = take_key(ctx, Modifiers::NONE, Key::Escape);
        if self.editor.is_some() || self.group_edit.is_some() {
            if escape {
                self.editor = None;
                self.group_edit = None;
            }
            return;
        }
        if self.panel_popup.is_some() {
            if escape {
                self.panel_popup = None;
            }
            return;
        }
        if self.tab == Tab::Settings {
            if escape {
                self.set_panel_tab(Tab::History);
            }
            return;
        }
        self.check_marks();
        if escape {
            if !self.marks.is_empty() {
                self.clear_marks();
            } else if !self.query.is_empty() {
                self.query.clear();
                self.reset_panel_selection();
            } else {
                self.hide();
            }
            return;
        }
        if take_key(ctx, Modifiers::SHIFT, Key::Tab) || take_key(ctx, Modifiers::NONE, Key::Tab) {
            self.switch_panel_tab();
            return;
        }

        if !ctx.input(|input| input.events.iter().any(|event| {
            matches!(event, egui::Event::Key { pressed: true, .. })
        })) {
            return;
        }
        let list = self.panel_list();
        self.sel = self.sel.min(list.rows.len().saturating_sub(1));
        let (down, up) = movement(ctx);
        let shift_down = take_key(ctx, Modifiers::SHIFT, Key::ArrowDown);
        let shift_up = take_key(ctx, Modifiers::SHIFT, Key::ArrowUp);
        if down || up || shift_down || shift_up {
            let next = if down || shift_down {
                (self.sel + 1).min(list.rows.len().saturating_sub(1))
            } else {
                self.sel.saturating_sub(1)
            };
            if shift_down || shift_up {
                self.mark_range(&list, next);
            } else {
                self.sel = next;
            }
            self.sel_moved = true;
        }
        if take_key(ctx, Modifiers::COMMAND, Key::A) {
            self.mark_between(&list, 0, list.rows.len().saturating_sub(1));
            self.anchor = Some(self.sel);
        }
        if self.tab == Tab::Favorites && self.query.trim().is_empty() {
            if take_key(ctx, Modifiers::NONE, Key::ArrowRight)
                && let Some(PanelRow::Group(group, _)) = list.rows.get(self.sel)
                && self.open_groups.insert(*group)
            {
                self.anchor = None;
            }
            if take_key(ctx, Modifiers::NONE, Key::ArrowLeft) {
                self.close_or_select_parent(&list);
            }
        }
        if take_key(ctx, Modifiers::NONE, Key::PageDown) {
            self.preview_scroll = -1.0;
        } else if take_key(ctx, Modifiers::NONE, Key::PageUp) {
            self.preview_scroll = 1.0;
        }
        if take_key(ctx, Modifiers::COMMAND, Key::K) {
            self.open_popup(PopupKind::Actions);
            return;
        }
        for (key, command) in [
            (Key::Enter, Command::Copy), (Key::S, Command::Favorite),
            (Key::E, Command::Edit), (Key::Backspace, Command::Delete),
            (Key::Z, Command::Undo), (Key::Comma, Command::Settings),
        ] {
            if take_key(ctx, Modifiers::COMMAND, key) {
                self.run_command(ctx, command, &list);
                return;
            }
        }
        if take_key(ctx, Modifiers::NONE, Key::Enter) {
            self.run_command(ctx, Command::Paste, &list);
            return;
        }
        for (index, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5,
            Key::Num6, Key::Num7, Key::Num8, Key::Num9].into_iter().enumerate()
        {
            if take_key(ctx, Modifiers::COMMAND, key) {
                if let Some(source) = list.numbered(index, &self.store) {
                    self.use_panel_sources(&[source], true);
                }
                return;
            }
        }
    }

    fn mark_range(&mut self, list: &PanelList, end: usize) {
        let start = self.anchor.unwrap_or(self.sel);
        self.mark_between(list, start, end);
        self.anchor = Some(start);
        self.sel = end;
    }

    fn mark_between(&mut self, list: &PanelList, a: usize, b: usize) {
        self.marks = list.rows.iter().skip(a.min(b)).take(a.abs_diff(b) + 1)
            .filter_map(|row| match row.source(&self.store) {
                Some(Src::Clip(id) | Src::Fav(id)) => Some(id),
                _ => None,
            }).collect();
        self.marks_ctx = (self.tab, self.query.clone());
    }

    fn mark_toggle(&mut self, list: &PanelList, index: usize) {
        let Some(Src::Clip(id) | Src::Fav(id)) = list.rows[index].source(&self.store) else { return };
        if !self.marks.remove(&id) {
            self.marks.insert(id);
        }
        self.sel = index;
        self.anchor = if self.marks.is_empty() { None } else { Some(index) };
        self.marks_ctx = (self.tab, self.query.clone());
    }

    fn close_or_select_parent(&mut self, list: &PanelList) {
        let group = match list.rows.get(self.sel).copied() {
            Some(PanelRow::Group(group, _)) => {
                if self.open_groups.remove(&group) {
                    self.anchor = None;
                    return;
                }
                self.store.group(group).and_then(|g| g.parent)
            }
            Some(PanelRow::Favorite(index, _)) => Some(self.store.favorites[index].group),
            _ => None,
        };
        if let Some(index) = list.rows.iter().position(|row| {
            matches!(row, PanelRow::Group(id, _) if Some(*id) == group)
        }) {
            self.sel = index;
            self.sel_moved = true;
        }
    }

    fn toggle_group(&mut self, group: u64) {
        self.anchor = None;
        if !self.open_groups.remove(&group) {
            self.open_groups.insert(group);
        }
    }

    fn use_panel_sources(&mut self, sources: &[Src], paste: bool) {
        self.request_clipboard(sources, paste, true);
    }

    fn edit_panel_source(&mut self, source: Src) {
        match source {
            Src::Fav(id) => self.run_actions(vec![Action::EditFav(id)]),
            Src::Clip(_) => {
                if let Some(content) = self.source_content(source).cloned() {
                    self.editor = Some(FavEditor {
                        id: None,
                        title: content.preview(28),
                        text: editor_text(&content),
                        content,
                        group: self.default_group(),
                    });
                }
            }
            Src::Header(_) => {}
        }
    }

    fn run_command(&mut self, ctx: &egui::Context, command: Command, list: &PanelList) {
        match command {
            Command::Paste if self.marks.is_empty() => {
                if let Some(PanelRow::Group(group, _)) = list.rows.get(self.sel) {
                    self.toggle_group(*group);
                } else if let Some(source) = self.selected_source(list) {
                    self.use_panel_sources(&[source], true);
                }
            }
            Command::Paste | Command::Copy => {
                self.use_panel_sources(&self.action_sources(list), matches!(command, Command::Paste));
            }
            Command::Favorite => {
                if !self.action_sources(list).is_empty() {
                    self.open_popup(PopupKind::Groups);
                }
            }
            Command::Edit => {
                if let Some(source) = self.selected_source(list) {
                    self.edit_panel_source(source);
                }
            }
            Command::Delete => {
                self.delete_panel_sources(self.action_sources(list));
                self.clear_marks();
            }
            Command::Undo => self.undo_delete(),
            Command::NewFavorite => {
                let group = match list.rows.get(self.sel) {
                    Some(PanelRow::Group(id, _)) => *id,
                    Some(PanelRow::Favorite(index, _)) => self.store.favorites[*index].group,
                    _ => self.default_group(),
                };
                self.run_actions(vec![Action::NewFav(group)]);
            }
            Command::NewGroup => {
                self.set_panel_tab(Tab::Favorites);
                self.query.clear();
                self.new_group();
            }
            Command::OpenWheel => self.show(ctx, Mode::Wheel, Level::Root),
            Command::Settings => self.set_panel_tab(Tab::Settings),
        }
    }

    fn delete_panel_sources(&mut self, sources: Vec<Src>) {
        let clips: HashSet<_> = sources.iter().filter_map(|source| match source {
            Src::Clip(id) => Some(*id),
            _ => None,
        }).collect();
        let favorites: HashSet<_> = sources.iter().filter_map(|source| match source {
            Src::Fav(id) => Some(*id),
            _ => None,
        }).collect();
        // Delete from the end so the last removed row retains its original index.
        let ordered: Vec<_> = self.store.history.iter().rev()
            .filter(|clip| clips.contains(&clip.id)).map(|clip| Src::Clip(clip.id))
            .chain(self.store.favorites.iter().rev()
                .filter(|favorite| favorites.contains(&favorite.id)).map(|favorite| Src::Fav(favorite.id)))
            .collect();
        for source in ordered {
            self.delete_item(source);
        }
    }

    fn open_popup(&mut self, kind: PopupKind) {
        self.panel_popup = Some(PanelPopup {
            kind, filter: String::new(), selected: 0, scroll_to_selected: true, rendered: false,
        });
    }

    fn menu_items(&self, popup: &PanelPopup, list: &PanelList) -> Vec<MenuItem> {
        let sources = self.action_sources(list);
        let mut items = Vec::new();
        let mut add = |choice, label: &str, badge, enabled| {
            if let Some(score) = store::score(&label.to_lowercase(), &popup.filter) {
                items.push(MenuItem { choice, label: label.into(), badge, enabled, score });
            }
        };
        match popup.kind {
            PopupKind::Actions => {
                for command in COMMANDS {
                    let enabled = match command {
                        Command::Paste | Command::Copy | Command::Favorite | Command::Delete => !sources.is_empty(),
                        Command::Edit => self.selected_source(list).is_some(),
                        Command::Undo => self.deleted.is_some(),
                        _ => true,
                    };
                    let label = if matches!(command, Command::Favorite) && self.tab == Tab::Favorites {
                        "Move to group…"
                    } else {
                        command.label()
                    };
                    add(Choice::Command(command), label, command.shortcut(), enabled);
                }
            }
            PopupKind::Groups => {
                let existing: Vec<_> = sources.iter().map(|&source| self.existing_favorite(source)).collect();
                for (group, _) in self.group_order() {
                    let already_there = !existing.is_empty() && existing.iter().all(|f| {
                        f.is_some_and(|favorite| favorite.group == group)
                    });
                    add(Choice::Group(group), &self.store.group_path(group), if already_there { "✓" } else { "" }, true);
                }
                if existing.iter().any(Option::is_some) {
                    add(Choice::RemoveFavorite, "Remove from favorites", "", true);
                }
            }
        }
        if popup.kind == PopupKind::Actions {
            items.sort_by_key(|item| std::cmp::Reverse(item.score));
        }
        items
    }

    fn existing_favorite(&self, source: Src) -> Option<&Favorite> {
        match source {
            Src::Fav(id) => self.store.favorites.iter().find(|f| f.id == id),
            Src::Clip(_) => {
                let content = self.source_content(source)?;
                self.store.favorites.iter().find(|f| &f.content == content)
            }
            Src::Header(_) => None,
        }
    }

    fn run_choice(&mut self, ctx: &egui::Context, choice: Choice, list: &PanelList) {
        self.panel_popup = None;
        match choice {
            Choice::Command(command) => self.run_command(ctx, command, list),
            Choice::Group(group) => {
                for source in self.action_sources(list) {
                    if let Some(id) = self.existing_favorite(source).map(|f| f.id) {
                        if let Some(favorite) = self.store.favorites.iter_mut().find(|f| f.id == id) {
                            favorite.group = group;
                        }
                    } else if let Some(content) = self.source_content(source).cloned() {
                        self.add_fav_now(content, group);
                    }
                }
                self.last_group = Some(group);
                self.store.reindex();
                self.dirty = true;
                self.clear_marks();
            }
            Choice::RemoveFavorite => {
                let favorites = self.action_sources(list).into_iter()
                    .filter_map(|source| self.existing_favorite(source).map(|f| Src::Fav(f.id)))
                    .collect();
                self.delete_panel_sources(favorites);
                self.clear_marks();
            }
        }
    }

    pub(super) fn draw_panel(&mut self, ctx: &egui::Context) {
        let colors = palette(ctx);
        let deferred = if self.pending_snippet.is_none() && self.tab != Tab::Settings && self.editor.is_none()
            && self.group_edit.is_none() && self.recording.is_none() && self.panel_popup.is_none()
        {
            defer_shortcuts_after_typing(ctx)
        } else {
            None
        };
        if deferred.is_none() {
            self.panel_keys(ctx);
        }
        if self.mode != Mode::Panel {
            return;
        }
        let frame = Frame::NONE.fill(colors.bg)
            .stroke(Stroke::new(1.0_f32, colors.line))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(11));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if self.pending_snippet.is_some() {
                ui.disable();
            }
            let bounds = ui.max_rect();
            let top = Rect::from_min_size(bounds.min, vec2(bounds.width(), TOP_HEIGHT));
            self.top_bar(&mut pane(ui, "panel_top", top, Layout::left_to_right(Align::Center)));
            if let Some(events) = deferred {
                ctx.input_mut(|input| input.events.extend(events));
                self.panel_keys(ctx);
                ctx.request_repaint();
            }
            if self.mode != Mode::Panel {
                return;
            }

            let mut body_top = top.bottom() + 7.0;
            if let Some(notice) = self.store.load_notice {
                let rect = Rect::from_min_size(pos2(bounds.left(), body_top), vec2(bounds.width(), 19.0));
                pane(ui, "storage_notice", rect, Layout::left_to_right(Align::Center))
                    .label(RichText::new(notice).size(11.5).color(colors.warning));
                body_top = rect.bottom() + 5.0;
            }
            if self.tab == Tab::Settings {
                let rect = Rect::from_min_max(pos2(bounds.left(), body_top), bounds.max);
                self.settings_page(&mut pane(ui, "panel_settings", rect, Layout::top_down(Align::Min)));
                return;
            }
            self.check_marks();
            let list = self.panel_list();
            self.sel = self.sel.min(list.rows.len().saturating_sub(1));
            let bottom = Rect::from_min_max(pos2(bounds.left(), bounds.bottom() - ACTION_HEIGHT), bounds.max);
            let body_bottom = bottom.top() - 6.0;
            let divider = bounds.left() + LIST_WIDTH;
            let left = Rect::from_min_max(pos2(bounds.left(), body_top), pos2(divider - 1.0, body_bottom));
            let right = Rect::from_min_max(pos2(divider + 1.0, body_top), pos2(bounds.right(), body_bottom));
            let line = Stroke::new(1.0_f32, colors.line);
            ui.painter().hline(bounds.x_range(), body_top - 2.0, line);
            ui.painter().vline(divider, body_top - 2.0..=bottom.top(), line);
            ui.painter().hline(bounds.x_range(), bottom.top(), line);
            let action = self.draw_list(&mut pane(ui, "panel_list", left, Layout::top_down(Align::Min)), &list);
            self.draw_preview(&mut pane(ui, "panel_preview", right, Layout::top_down(Align::Min)), &list);
            self.draw_action_bar(&mut pane(ui, "panel_bottom", bottom, Layout::left_to_right(Align::Center)), &list);
            let action = action.filter(|_| self.panel_popup.is_none() && self.pending_snippet.is_none());
            self.run_list_action(ctx, action, &list);
            self.draw_popup(ctx);
        });
        self.editor_window(ctx);
        self.draw_group_editor(ctx);
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing.x = 4.0;
        if self.tab == Tab::Settings {
            if icon_button(ui, Icon::Back, "Back to History (Esc)", false).clicked() {
                self.set_panel_tab(Tab::History);
            }
            ui.label(RichText::new("Settings").strong().size(13.0));
        } else {
            let w = ui.available_width() - 164.0 - 34.0 - 3.0 * 28.0
                - if self.tab == Tab::Favorites { 28.0 } else { 0.0 };
            let placeholder = if self.tab == Tab::History { "Search clipboard…" } else { "Search favorites…" };
            let focus = self.pending_snippet.is_none() && self.panel_popup.is_none() && self.editor.is_none() && self.group_edit.is_none() && self.shot.is_none();
            if !focus {
                ui.memory_mut(|memory| memory.surrender_focus(Id::new("panel_search")));
            }
            if search_field(ui, "panel_search", &mut self.query, placeholder, w, focus).changed() {
                self.reset_panel_selection();
            }
        }
        let drag_width = if self.tab == Tab::Settings { ui.available_width() - 56.0 } else { 30.0 };
        let (_, drag) = ui.allocate_exact_size(vec2(drag_width.max(0.0), TOP_HEIGHT), Sense::drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if self.tab != Tab::Settings {
            if let Some(tab) = segmented_control(
                ui, "tab", &[("History", Tab::History), ("Favorites", Tab::Favorites)], self.tab,
            ) {
                self.set_panel_tab(tab);
            }
            if self.tab == Tab::Favorites && icon_button(ui, Icon::Plus, "New group or favorite (⌘K)", false).clicked() {
                self.open_popup(PopupKind::Actions);
                if let Some(popup) = &mut self.panel_popup {
                    popup.filter = "new".into();
                }
            }
            if icon_button(ui, Icon::Settings, "Settings (⌘,)", false).clicked() {
                self.set_panel_tab(Tab::Settings);
            }
        }
        if icon_button(ui, Icon::Pin, "Pin the panel to keep it open", self.pinned).clicked() {
            self.pinned = !self.pinned;
        }
        if icon_button(ui, Icon::Close, "Hide (Esc)", false).clicked() {
            self.hide();
        }
    }

    fn draw_list(&mut self, ui: &mut Ui, list: &PanelList) -> Option<ListAction> {
        if list.rows.is_empty() {
            let empty = match self.tab {
                Tab::History if self.store.history.is_empty() => "No copies yet.",
                Tab::History => "No matching items.",
                _ => "No matching favorites.",
            };
            empty_note(ui, empty);
            return None;
        }
        let searching = !self.query.trim().is_empty();
        let scroll_to_selected = std::mem::take(&mut self.sel_moved);
        let mut area = ScrollArea::vertical().auto_shrink([false, false])
            .id_salt(if self.tab == Tab::History { "history" } else { "favorites" });
        if let Some(offset) = self.scroll_want.take() {
            area = area.vertical_scroll_offset(offset);
        }
        let mut action = None;
        area.show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = ROW_GAP;
            let mut row = 0;
            let mut item = 0;
            for entry in &list.entries {
                let top = ui.cursor().top();
                let height = match entry {
                    ListEntry::Section(_) => SECTION_HEIGHT,
                    ListEntry::Row(PanelRow::Group(..)) => GROUP_HEIGHT,
                    ListEntry::Row(_) => PANEL_ROW_HEIGHT,
                };
                if top + height < ui.clip_rect().top() || top > ui.clip_rect().bottom() {
                    ui.allocate_space(vec2(ui.available_width(), height));
                    if let ListEntry::Row(entry) = entry {
                        if scroll_to_selected && row == self.sel {
                            ui.scroll_to_rect(Rect::from_min_size(pos2(ui.cursor().left(), top), vec2(1.0, height)), None);
                        }
                        row += 1;
                        item += usize::from(!matches!(entry, PanelRow::Group(..)));
                    }
                    continue;
                }
                match entry {
                    ListEntry::Section(title) => {
                        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
                        section_title(ui, rect, title);
                    }
                    ListEntry::Row(PanelRow::Group(group, depth)) => {
                        let Some(g) = self.store.group(*group) else { continue };
                        let (response, command) = self.draw_group_row(ui, g, *depth, row == self.sel);
                        if let Some(command) = command {
                            action = Some(ListAction::Group(command, row));
                        } else if response.clicked() {
                            action = Some(ListAction::Click(row));
                        }
                        if scroll_to_selected && row == self.sel {
                            response.scroll_to_me(None);
                        }
                        row += 1;
                    }
                    ListEntry::Row(entry) => {
                        let (content, title, meta, fav, id, depth) = match *entry {
                            PanelRow::Clip(index) => {
                                let clip = &self.store.history[index];
                                let age = ago(clip.created);
                                let meta = clip.source.as_ref().map_or_else(|| age.clone(), |source| format!("{source} · {age}"));
                                (&clip.content, None, meta, Some(self.store.is_favorite(&clip.content)), clip.id, 0)
                            }
                            PanelRow::Favorite(index, depth) => {
                                let favorite = &self.store.favorites[index];
                                let path = if searching { self.store.group_path(favorite.group) } else { String::new() };
                                (&favorite.content, Some(favorite.title.as_str()), path, None, favorite.id, depth)
                            }
                            PanelRow::Group(..) => unreachable!(),
                        };
                        let spec = RowSpec {
                            n: item, selected: self.sel == row, content, title, meta: &meta, fav,
                            dot: None, marked: self.marks.contains(&id),
                        };
                        let output = if depth == 0 {
                            draw_row(ui, &mut self.textures, spec)
                        } else {
                            draw_indented_row(ui, &mut self.textures, spec, depth as f32 * 16.0)
                        };
                        if output.copy {
                            action = Some(ListAction::Copy(row));
                        } else if output.star {
                            action = Some(ListAction::Favorite(row));
                        } else if output.edit {
                            action = Some(ListAction::Edit(row));
                        } else if output.delete {
                            action = Some(ListAction::Delete(row));
                        } else if output.clicked {
                            action = Some(ListAction::Click(row));
                        }
                        if scroll_to_selected && row == self.sel {
                            output.response.scroll_to_me(None);
                        }
                        row += 1;
                        item += 1;
                    }
                }
            }
        });
        action
    }

    fn draw_group_row(&self, ui: &mut Ui, group: &Group, depth: usize, selected: bool) -> (egui::Response, Option<Action>) {
        let colors = palette(ui.ctx());
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), GROUP_HEIGHT), Sense::click());
        let hover = response.contains_pointer();
        panel_row_fill(ui, rect, selected, hover);
        let x = (rect.left() + 10.0 + depth as f32 * 16.0).min(rect.right() - 132.0);
        let center = rect.center().y;
        let chevron = if self.open_groups.contains(&group.id) { Icon::Down } else { Icon::Right };
        paint_icon(ui.painter(), chevron, Rect::from_center_size(pos2(x + 5.0, center), vec2(12.0, 12.0)), colors.dim);
        ui.painter().circle_filled(pos2(x + 22.0, center), 4.0, rgb(group.color));
        let mut right = rect.right() - 10.0;
        let mut action = None;
        if selected || hover {
            let confirming = self.confirm_group_delete == Some(group.id);
            if self.store.groups.len() > 1 {
                let tip = if confirming { "Click again to delete group and contents" } else { "Delete group" };
                if panel_row_tool(ui, &response, &mut right, Icon::Trash, tip, if confirming { colors.accent } else { colors.dim }) {
                    action = Some(Action::DeleteGroup(group.id));
                }
            }
            if panel_row_tool(ui, &response, &mut right, Icon::Pencil, "Edit group", colors.dim) {
                action = Some(Action::EditGroup(group.id));
            }
            if panel_row_tool(ui, &response, &mut right, Icon::Plus, "New favorite in this group", colors.dim) {
                action = Some(Action::NewFav(group.id));
            }
        }
        let count = self.store.favorite_count(group.id).to_string();
        let count = ui.painter().layout_no_wrap(count, FontId::proportional(11.0), colors.dim);
        let left = x + 33.0;
        let width = (right - left - count.size().x - 10.0).max(1.0);
        let name = elided(ui, &group.name, FontId::proportional(13.0), colors.text, width, 1);
        let at = pos2(left, center - name.size().y / 2.0);
        panel_semibold(ui, at, name.clone(), colors.text);
        ui.painter().galley(pos2(at.x + name.size().x + 8.0, center - count.size().y / 2.0), count, colors.dim);
        (response, action)
    }

    fn run_list_action(&mut self, ctx: &egui::Context, action: Option<ListAction>, list: &PanelList) {
        let (mods, released, now) = ctx.input(|i| (i.modifiers, i.pointer.any_released(), i.time));
        // egui counts clicks across widgets. Pair only consecutive plain clicks on the same item.
        let click_id = Id::new("panel_last_item_click");
        let last_click = if released {
            ctx.data_mut(|data| data.remove_temp::<Option<(Src, f64)>>(click_id)).flatten()
        } else {
            None
        };
        let Some(action) = action else { return };
        ctx.request_repaint();
        match action {
            ListAction::Group(command, index) => {
                self.sel = index;
                self.run_actions(vec![command]);
            }
            ListAction::Click(index) => {
                let source = list.rows[index].source(&self.store);
                let double_clicked = last_click.is_some_and(|(last, time)| {
                    source == Some(last)
                        && now - time < ctx.options(|opts| opts.input_options.max_double_click_delay)
                });
                match click_action(double_clicked, mods) {
                    ClickAction::RangeMark => self.mark_range(list, index),
                    ClickAction::ToggleMark => self.mark_toggle(list, index),
                    click @ (ClickAction::Select | ClickAction::Paste) => {
                        self.sel = index;
                        self.clear_marks();
                        if click == ClickAction::Paste || source.is_none() {
                            self.run_command(ctx, Command::Paste, list);
                        } else if released {
                            ctx.data_mut(|data| data.insert_temp(click_id, source.map(|src| (src, now))));
                        }
                    }
                }
            }
            ListAction::Copy(index) | ListAction::Edit(index) | ListAction::Delete(index) | ListAction::Favorite(index) => {
                self.sel = index;
                if let Some(source) = self.selected_source(list) {
                    match action {
                        ListAction::Copy(_) => self.use_panel_sources(&[source], false),
                        ListAction::Edit(_) => self.edit_panel_source(source),
                        ListAction::Delete(_) => {
                            self.delete_item(source);
                            if let Src::Clip(id) | Src::Fav(id) = source {
                                self.marks.remove(&id);
                            }
                        }
                        ListAction::Favorite(_) => self.open_popup(PopupKind::Groups),
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    fn draw_preview(&mut self, ui: &mut Ui, list: &PanelList) {
        let colors = palette(ui.ctx());
        let bounds = ui.max_rect().shrink(16.0);
        let scroll = std::mem::take(&mut self.preview_scroll);
        if let Some(PanelRow::Group(id, _)) = list.rows.get(self.sel) {
            self.draw_group_summary(ui, bounds, *id, scroll);
            return;
        }
        let Some(source) = self.selected_source(list) else {
            ui.painter().text(bounds.center(), Align2::CENTER_CENTER, "Nothing selected",
                FontId::proportional(12.5), colors.dim);
            return;
        };
        let (content, title, mut metadata) = match source {
            Src::Clip(id) => {
                let clip = self.store.history.iter().find(|clip| clip.id == id).unwrap();
                let relative = ago(clip.created);
                let relative = if relative == "now" { relative } else { format!("{relative} ago") };
                (&clip.content, "", vec![
                    ("Source", clip.source.clone().unwrap_or_else(|| "Unknown".into())),
                    ("Copied", format!("{relative} · {}", local_clock(clip.created))),
                ])
            }
            Src::Fav(id) => {
                let f = self.store.favorites.iter().find(|f| f.id == id).unwrap();
                (&f.content, f.title.as_str(), vec![])
            }
            Src::Header(_) => return,
        };
        let label = match content {
            Content::Text { text } => {
                let lines = if text.is_empty() { 0 } else { 1 + text.bytes().filter(|&b| b == b'\n').count() };
                let chars = text.chars().count();
                let char_unit = if chars == 1 { "char" } else { "chars" };
                let line_unit = if lines == 1 { "line" } else { "lines" };
                metadata.push(("Length", format!("{chars} {char_unit} · {lines} {line_unit}")));
                "Text".into()
            }
            Content::Image { w, h, .. } => format!("Image {w}×{h}"),
        };
        if let Src::Fav(id) = source {
            let f = self.store.favorites.iter().find(|f| f.id == id).unwrap();
            metadata.push(("Group", self.store.group_path(f.group)));
            metadata.push(("Uses", f.uses.to_string()));
        }
        let title = if title.trim().is_empty() { &label } else { title };
        let title = elided(ui, title, FontId::proportional(13.0), colors.text, bounds.width(), 1);
        panel_semibold(ui, bounds.min, title, colors.text);

        let variables = snippet_text(source, content).map(crate::snippet::vars);
        let variables = variables.map(|variables| {
            let names = variables.into_iter().map(|var| {
                let name = format!("{{{{{}}}}}", var.name);
                match var.default {
                    Some(default) => format!("{name} (default: {default:?})"),
                    None => name,
                }
            }).collect::<Vec<_>>().join(" · ");
            ui.painter().layout(names, FontId::monospace(11.0), colors.text, bounds.width() - 72.0)
        });
        let variables_height = variables.as_ref().map_or(0.0, |galley| galley.size().y.clamp(17.0, 85.0));
        let metadata_top = bounds.bottom() - metadata.len() as f32 * 17.0 - variables_height;
        let body_bottom = if metadata.is_empty() { bounds.bottom() } else { metadata_top - 16.0 };
        if !metadata.is_empty() {
            ui.painter().hline(bounds.x_range(), metadata_top - 7.0, Stroke::new(1.0_f32, colors.line));
        }
        for (index, (label, value)) in metadata.into_iter().enumerate() {
            let at = pos2(bounds.left(), metadata_top + 17.0 * index as f32);
            ui.painter().text(at, Align2::LEFT_TOP, label, FontId::proportional(11.0), colors.dim);
            let value = elided(ui, &value, FontId::proportional(11.0), colors.text, bounds.width() - 62.0, 1);
            ui.painter().galley(at + vec2(62.0, 0.0), value, colors.text);
        }
        if let Some(galley) = variables {
            let top = bounds.bottom() - variables_height;
            ui.painter().text(pos2(bounds.left(), top), Align2::LEFT_TOP, "Variables", FontId::proportional(11.0), colors.dim);
            let rect = Rect::from_min_max(pos2(bounds.left() + 62.0, top), bounds.max);
            let mut value = pane(ui, "preview_variables", rect, Layout::top_down(Align::Min));
            value.set_clip_rect(value.clip_rect().intersect(rect));
            ScrollArea::vertical().id_salt(("variables", source)).max_height(variables_height)
                .auto_shrink([false, false]).show(&mut value, |ui| {
                    ui.add(egui::Label::new(galley).selectable(true));
                });
        }
        let body = Rect::from_min_max(pos2(bounds.left(), bounds.top() + 26.0), pos2(bounds.right(), body_bottom));
        match content {
            Content::Text { text } => {
                let mut preview = pane(ui, "preview_body", body, Layout::top_down(Align::Min));
                ScrollArea::vertical().id_salt(("preview_text", source)).auto_shrink([false, false])
                    .show(&mut preview, |ui| {
                        if scroll != 0.0 {
                            ui.scroll_with_delta(vec2(0.0, scroll * (body.height() - 17.5)));
                        }
                        ui.add(egui::Label::new(RichText::new(text).monospace().size(12.5)
                            .line_height(Some(17.5)).color(colors.text)).wrap().selectable(true));
                    });
            }
            Content::Image { file, .. } => {
                if let Some(texture) = preview_texture(ui.ctx(), &mut self.textures, file, body.size()) {
                    paint_texture(ui, texture, body, 8);
                } else {
                    ui.painter().text(body.center(), Align2::CENTER_CENTER, "Image unavailable",
                        FontId::proportional(12.0), colors.dim);
                }
            }
        }
    }

    fn draw_group_summary(&self, ui: &mut Ui, bounds: Rect, id: u64, scroll: f32) {
        let colors = palette(ui.ctx());
        let Some(group) = self.store.group(id) else { return };
        let mut summary = pane(ui, "group_summary", bounds, Layout::top_down(Align::Min));
        ScrollArea::vertical().id_salt(("group_summary", id)).show(&mut summary, |ui| {
            if scroll != 0.0 {
                ui.scroll_with_delta(vec2(0.0, scroll * (bounds.height() - 17.0)));
            }
            ui.add(egui::Label::new(RichText::new(self.store.group_path(id)).strong().size(13.0)).wrap());
            ui.add_space(10.0);
            let subgroups: Vec<_> = self.store.groups.iter()
                .filter(|child| child.parent == Some(id))
                .map(|child| child.name.as_str()).collect();
            let subgroups = if subgroups.is_empty() { "None".into() } else { subgroups.join(" · ") };
            let mut rows = vec![("Favorites", self.store.favorite_count(id).to_string()), ("Subgroups", subgroups)];
            if !group.hotkey.trim().is_empty() {
                rows.push(("Shortcut", group.hotkey.clone()));
            }
            egui::Grid::new("group_metadata").min_col_width(62.0).spacing(vec2(0.0, 5.0)).show(ui, |ui| {
                for (label, value) in rows {
                    ui.label(RichText::new(label).size(11.0).color(colors.dim));
                    ui.add(egui::Label::new(RichText::new(value).size(11.0).color(colors.text)).wrap());
                    ui.end_row();
                }
            });
        });
    }

    fn draw_action_bar(&self, ui: &mut Ui, list: &PanelList) {
        let colors = palette(ui.ctx());
        let rect = ui.max_rect();
        let searching = !self.query.trim().is_empty();
        let counter = match self.tab {
            Tab::History if searching => format!("{} results", list.rows.len()),
            Tab::History => format!("{} / {} items", self.store.history.len(), self.store.settings.history_limit),
            Tab::Favorites if searching => format!("{} results", list.rows.len()),
            Tab::Favorites => format!("{} groups · {} favorites", self.store.groups.len(), self.store.favorites.len()),
            Tab::Settings => String::new(),
        };
        ui.painter().text(pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, counter,
            FontId::proportional(11.0), colors.dim);
        let marked = self.marked_sources(list).len();
        let hints: Vec<(&str, String)> = if marked != 0 {
            vec![("↵", format!("Paste {marked}")), ("⌘S", format!("Favorite {marked}")),
                ("⌘⌫", format!("Delete {marked}")), ("Esc", "Clear".into())]
        } else if self.selected_source(list).is_some() {
            let mut hints = vec![("↵", "Paste".into()), ("⌘↵", "Copy".into())];
            if self.tab == Tab::History {
                hints.push(("⌘S", "Favorite".into()));
            }
            hints.extend([("⌘E", "Edit".into()), ("⌘⌫", "Delete".into()), ("⌘K", "Actions".into())]);
            hints
        } else if matches!(list.rows.get(self.sel), Some(PanelRow::Group(..))) {
            vec![("↵", "Toggle group".into()), ("⌘K", "Actions".into())]
        } else {
            vec![("⌘K", "Actions".into())]
        };
        let mut right = rect.right();
        for (key, label) in hints.into_iter().rev() {
            let label = ui.painter().layout_no_wrap(label, FontId::proportional(11.0), colors.dim);
            right -= label.size().x;
            ui.painter().galley(pos2(right, rect.center().y - label.size().y / 2.0), label, colors.dim);
            right = paint_keycap(ui, right - 5.0, rect.center().y, key) - 13.0;
        }
    }

    fn draw_popup(&mut self, ctx: &egui::Context) {
        let colors = palette(ctx);
        let Some(mut popup) = self.panel_popup.take() else { return };
        let (down, up) = movement(ctx);
        let enter = take_key(ctx, Modifiers::NONE, Key::Enter);
        let mut choice = None;
        let list = self.panel_list();
        let group_picker = popup.kind == PopupKind::Groups;
        let area = egui::Area::new(Id::new("panel_popup"))
            .order(egui::Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, vec2(-12.0, -46.0))
            .default_size(vec2(MENU_WIDTH, 300.0))
            .interactable(true);
        let out = area.show(ctx, |ui| {
            Frame::NONE.fill(colors.bg2).stroke(Stroke::new(1.0_f32, colors.line)).corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(9))
                .shadow(egui::epaint::Shadow { offset: [0, 5], blur: 17, spread: 0, color: colors.popup_shadow })
                .show(ui, |ui| {
                    ui.set_width(MENU_WIDTH - 20.0);
                    let hint = if group_picker { "Find a group…" } else { "Find an action…" };
                    if search_field(ui, "popup_filter", &mut popup.filter, hint, MENU_WIDTH - 20.0, true).changed() {
                        popup.selected = 0;
                        popup.scroll_to_selected = true;
                    }
                    let items = self.menu_items(&popup, &list);
                    if items.is_empty() {
                        ui.label(RichText::new(if group_picker { "No matching groups." } else { "No matching actions." })
                            .size(12.0).color(colors.dim));
                        return;
                    }
                    popup.selected = popup.selected.min(items.len() - 1);
                    if !items[popup.selected].enabled {
                        popup.selected = items.iter().position(|item| item.enabled).unwrap_or(0);
                    }
                    let next = if down {
                        items.iter().enumerate().skip(popup.selected + 1)
                            .find(|(_, item)| item.enabled).map(|(index, _)| index)
                    } else if up {
                        items[..popup.selected].iter().rposition(|item| item.enabled)
                    } else {
                        None
                    };
                    if let Some(next) = next {
                        popup.selected = next;
                        popup.scroll_to_selected = true;
                    }
                    ScrollArea::vertical().id_salt(("popup_choices", group_picker)).max_height(280.0)
                        .auto_shrink([false, true]).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for (index, item) in items.into_iter().enumerate() {
                                let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0),
                                    if item.enabled { Sense::click() } else { Sense::hover() });
                                let selected = item.enabled && index == popup.selected;
                                if selected || response.hovered() && item.enabled {
                                    ui.painter().rect_filled(rect, CornerRadius::same(6), if selected { colors.accent_soft } else { colors.hover });
                                }
                                if selected && std::mem::take(&mut popup.scroll_to_selected) {
                                    response.scroll_to_me(None);
                                }
                                let mut right = rect.right() - 7.0;
                                if item.badge == "✓" {
                                    let check = Rect::from_center_size(pos2(right - 6.0, rect.center().y), vec2(12.0, 12.0));
                                    paint_icon(ui.painter(), Icon::Check, check, colors.accent);
                                    right -= 16.0;
                                } else if !item.badge.is_empty() {
                                    right = paint_keycap(ui, right, rect.center().y, item.badge) - 6.0;
                                }
                                let color = if item.enabled { colors.text } else { colors.dim };
                                let label = elided(ui, &item.label, FontId::proportional(12.5), color,
                                    right - rect.left() - 8.0, 1);
                                ui.painter().galley(pos2(rect.left() + 8.0, rect.center().y - label.size().y / 2.0), label, color);
                                if response.clicked() || (enter && selected) {
                                    choice = Some(item.choice);
                                }
                            }
                        });
                });
        });
        let clicked_outside = popup.rendered && out.response.clicked_elsewhere();
        popup.rendered = true;
        if let Some(choice) = choice {
            self.run_choice(ctx, choice, &list);
        } else if !clicked_outside {
            self.panel_popup = Some(popup);
        }
    }

    fn draw_group_editor(&mut self, ctx: &egui::Context) {
        let Some(edit) = self.group_edit.as_mut() else { return };
        let (mut save, mut cancel, mut record) = (false, false, false);
        egui::Window::new("Edit group").anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false).collapsible(false).fixed_size([320.0, 0.0])
            .show(ctx, |ui| group_editor(ui, edit, &mut save, &mut cancel, self.recording == Some(RecTarget::Group), &mut record));
        if save {
            if let Some(group) = self.store.groups.iter_mut().find(|group| group.id == edit.id) {
                group.name = if edit.name.trim().is_empty() { "Group".into() } else { edit.name.trim().into() };
                group.hotkey = edit.hotkey.trim().into();
                group.color = edit.color;
            }
            self.group_edit = None;
            self.dirty = true;
            self.reapply_hotkeys();
        } else if cancel {
            self.group_edit = None;
        } else if record {
            self.start_recording(RecTarget::Group);
        }
    }

    pub(super) fn run_actions(&mut self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::EditFav(id) => {
                    if let Some(f) = self.store.favorites.iter().find(|f| f.id == id) {
                        self.editor = Some(FavEditor {
                            id: Some(id), title: f.title.clone(), text: editor_text(&f.content),
                            content: f.content.clone(), group: f.group,
                        });
                    }
                }
                Action::NewFav(group) => {
                    self.open_groups.insert(group);
                    self.editor = Some(FavEditor {
                        id: None, title: String::new(), text: String::new(),
                        content: Content::default(), group,
                    });
                }
                Action::EditGroup(id) => {
                    if let Some(g) = self.store.group(id) {
                        self.group_edit = Some(GroupEdit {
                            id, name: g.name.clone(), hotkey: g.hotkey.clone(), color: g.color,
                        });
                    }
                }
                Action::DeleteGroup(id) => {
                    if self.confirm_group_delete == Some(id) && self.shot.is_none() {
                        self.store.remove_group(id);
                        self.confirm_group_delete = None;
                        self.dirty = true;
                        self.reapply_hotkeys();
                    } else {
                        self.confirm_group_delete = Some(id);
                    }
                }
            }
        }
    }

    /// Adds a group and opens its editor.
    pub(super) fn new_group(&mut self) {
        let id = self.store.new_id();
        let color = PALETTE[self.store.groups.len() % PALETTE.len()];
        let name = format!("New group {}", self.store.groups.len() + 1);
        self.store.groups.push(Group { id, name: name.clone(), color, hotkey: String::new(), parent: None });
        self.open_groups.insert(id);
        self.group_edit = Some(GroupEdit { id, name, hotkey: String::new(), color });
        self.dirty = true;
    }

    pub(super) fn start_recording(&mut self, target: RecTarget) {
        // Free the current shortcuts so pressing them is seen here, not by the system.
        if let Some(mgr) = &self.hotkeys.mgr {
            let _ = mgr.unregister_all(&self.hotkeys.registered);
        }
        self.recording = Some(target);
    }

    fn record_keys(&mut self, ctx: &egui::Context) {
        let mut result: Option<Option<String>> = None;
        ctx.input_mut(|i| {
            for e in &i.events {
                if let egui::Event::Key { key, pressed: true, modifiers, .. } = e {
                    if *key == Key::Escape && modifiers.is_none() {
                        result = Some(None);
                    } else if let Some(t) = hotkey_text(*key, *modifiers) {
                        result = Some(Some(t));
                    }
                }
            }
            i.events.retain(|_| false);
        });
        let Some(res) = result else { return };
        let target = self.recording.take();
        if let Some(text) = res {
            match target {
                Some(RecTarget::Panel) => self.store.settings.hotkey_panel = text,
                Some(RecTarget::Wheel) => self.store.settings.hotkey_wheel = text,
                Some(RecTarget::Favorites) => self.store.settings.hotkey_favorites = text,
                Some(RecTarget::Group) => {
                    if let Some(e) = self.group_edit.as_mut() {
                        e.hotkey = text;
                    }
                }
                None => {}
            }
            self.dirty = true;
        }
        self.reapply_hotkeys();
    }
}

/// Let the text edit apply a new query before handling a shortcut batched after it.
fn defer_shortcuts_after_typing(ctx: &egui::Context) -> Option<Vec<egui::Event>> {
    ctx.input_mut(|input| {
        let is_shortcut = |event: &egui::Event| {
            let egui::Event::Key { key, physical_key, pressed: true, modifiers, .. } = event else { return false };
            use Key::*;
            let matches = |expected| panel_modifiers(*modifiers, expected);
            if matches(Modifiers::COMMAND) {
                let number = |key| matches!(key, Num1 | Num2 | Num3 | Num4 | Num5 | Num6 | Num7 | Num8 | Num9);
                number(*key) || physical_key.is_some_and(number)
                    || matches!(key, A | K | S | E | Z | Enter | Backspace | Comma)
            } else if matches(Modifiers::CTRL) {
                matches!(key, N | P | J | K)
            } else if matches(Modifiers::SHIFT) {
                matches!(key, ArrowUp | ArrowDown | Tab)
            } else {
                matches(Modifiers::NONE) && matches!(key, ArrowUp | ArrowDown | Enter | Escape | Tab | PageUp | PageDown)
            }
        };
        let first_shortcut = input.events.iter().position(is_shortcut)?;
        let typed_first = input.events[..first_shortcut].iter().any(|event| {
            matches!(event, egui::Event::Text(text) | egui::Event::Paste(text) | egui::Event::Ime(egui::ImeEvent::Commit(text))
                if !text.is_empty() && text != "\n" && text != "\r")
        });
        if !typed_first {
            return None;
        }
        let (deferred, remaining) = std::mem::take(&mut input.events).into_iter().partition(is_shortcut);
        input.events = remaining;
        Some(deferred)
    })
}

fn take_key(ctx: &egui::Context, modifiers: Modifiers, key: Key) -> bool {
    ctx.input_mut(|input| {
        let mut found = false;
        input.events.retain(|event| {
            let matching = matches!(event, egui::Event::Key { key: logical, physical_key, pressed: true, modifiers: actual, .. }
                if (*logical == key || (matches!(key, Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4 | Key::Num5
                    | Key::Num6 | Key::Num7 | Key::Num8 | Key::Num9) && *physical_key == Some(key)))
                    && panel_modifiers(*actual, modifiers));
            found |= matching;
            !matching
        });
        found
    })
}

fn panel_modifiers(actual: Modifiers, expected: Modifiers) -> bool {
    if cfg!(target_os = "macos") && expected == Modifiers::COMMAND {
        actual.matches_exact(Modifiers::MAC_CMD)
    } else {
        actual.matches_exact(expected) && !actual.mac_cmd
    }
}

fn movement(ctx: &egui::Context) -> (bool, bool) {
    let down = take_key(ctx, Modifiers::NONE, Key::ArrowDown)
        | take_key(ctx, Modifiers::CTRL, Key::N) | take_key(ctx, Modifiers::CTRL, Key::J);
    let up = take_key(ctx, Modifiers::NONE, Key::ArrowUp)
        | take_key(ctx, Modifiers::CTRL, Key::P) | take_key(ctx, Modifiers::CTRL, Key::K);
    (down, up)
}

fn pane(ui: &mut Ui, id: &'static str, rect: Rect, layout: Layout) -> Ui {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(rect).layout(layout));
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    child
}

fn editor_text(content: &Content) -> String {
    match content {
        Content::Text { text } => text.clone(),
        Content::Image { .. } => String::new(),
    }
}

#[cfg(target_os = "macos")]
fn history_cutoffs(now: i64) -> [i64; 3] {
    use objc2_foundation::{NSCalendar, NSCalendarOptions, NSCalendarUnit, NSDate};
    let calendar = NSCalendar::currentCalendar();
    let date = NSDate::dateWithTimeIntervalSince1970(now as f64);
    let today = calendar.startOfDayForDate(&date);
    let weekday = calendar.component_fromDate(NSCalendarUnit::Weekday, &today);
    let days_since_week_start = (weekday - calendar.firstWeekday() as isize).rem_euclid(7);
    [0, -1, -days_since_week_start].map(|days| {
        calendar.dateByAddingUnit_value_toDate_options(NSCalendarUnit::Day, days, &today, NSCalendarOptions::empty())
            .map_or(now - now.rem_euclid(86_400) + days as i64 * 86_400, |date| date.timeIntervalSince1970() as i64)
    })
}

#[cfg(not(target_os = "macos"))]
fn history_cutoffs(now: i64) -> [i64; 3] {
    let today = now - now.rem_euclid(86_400);
    [today, today - 86_400, today - 6 * 86_400]
}

#[cfg(target_os = "macos")]
fn local_clock(created: i64) -> String {
    use objc2_foundation::{NSDate, NSDateFormatter, ns_string};
    let format = NSDateFormatter::new();
    format.setDateFormat(Some(ns_string!("HH:mm")));
    format.stringFromDate(&NSDate::dateWithTimeIntervalSince1970(created as f64)).to_string()
}

#[cfg(not(target_os = "macos"))]
fn local_clock(created: i64) -> String {
    let minutes = created.rem_euclid(86_400) / 60;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

#[cfg(target_os = "macos")]
pub(super) fn copy_multiple_items(contents: &[&Content]) -> bool {
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{
        NSPasteboard, NSPasteboardItem, NSPasteboardTypePNG, NSPasteboardTypeString,
    };
    use objc2_foundation::{NSArray, NSData, NSString};

    // SAFETY: AppKit exposes both pasteboard types as immutable NSStrings.
    let (string_type, png_type) = unsafe { (NSPasteboardTypeString, NSPasteboardTypePNG) };
    let mut items = Vec::with_capacity(contents.len());
    for content in contents {
        let item = NSPasteboardItem::new();
        let ok = match content {
            Content::Text { text } => item.setString_forType(&NSString::from_str(text), string_type),
            Content::Image { file, .. } => {
                let Ok(data) = store::image_path(file).and_then(std::fs::read) else { return false };
                item.setData_forType(&NSData::with_bytes(&data), png_type)
            }
        };
        if !ok {
            return false;
        }
        items.push(ProtocolObject::from_retained(item));
    }
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    pasteboard.writeObjects(&NSArray::from_retained_slice(&items))
}

#[cfg(not(target_os = "macos"))]
pub(super) fn copy_multiple_items(_contents: &[&Content]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_action_selects_pastes_or_marks() {
        assert_eq!(click_action(false, Modifiers::NONE), ClickAction::Select);
        assert_eq!(click_action(true, Modifiers::NONE), ClickAction::Paste);
        assert_eq!(click_action(false, Modifiers::SHIFT), ClickAction::RangeMark);
        assert_eq!(click_action(false, Modifiers::COMMAND), ClickAction::ToggleMark);
        assert_eq!(click_action(true, Modifiers::SHIFT), ClickAction::RangeMark);
        assert_eq!(click_action(true, Modifiers::COMMAND), ClickAction::ToggleMark);
    }
}
