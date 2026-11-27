//! The wheel menu: groups around a smooth dial, plus a search bar and item cards
//! beside it. Hovering or pressing arrows/numbers previews items on the right;
//! Cmd+K focuses fast search across favorites and history.

use super::*;

/// Rows shown per group in the card beside the wheel when not searching.
pub(super) const LIST_MAX: usize = 9;
pub(super) const SCROLL_TO_ROW: &str = "wheel_scroll_to_row";
pub(super) const SHORTCUT_NUMBERS: [Key; 10] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
    Key::Num0,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WheelShortcut {
    Segment(usize),
    Row(usize),
    NextPage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WheelRowAction {
    Edit,
    Delete,
}

fn take_wheel_row_action(events: &mut Vec<egui::Event>) -> Option<WheelRowAction> {
    let mut action = None;
    events.retain(|event| {
        let egui::Event::Key {
            key,
            modifiers,
            pressed,
            repeat,
            ..
        } = event
        else {
            return true;
        };
        if !command_only(*modifiers) {
            return true;
        }
        let row_action = match key {
            Key::E => WheelRowAction::Edit,
            Key::Backspace => WheelRowAction::Delete,
            _ => return true,
        };
        if *pressed && !repeat {
            action.get_or_insert(row_action);
        }
        false
    });
    action
}

pub(super) fn shortcut_digit(index: usize) -> usize {
    (index + 1) % SHORTCUT_NUMBERS.len()
}

pub(super) fn take_wheel_shortcut(
    events: &mut Vec<egui::Event>,
    number_panel: bool,
) -> Option<WheelShortcut> {
    let mut shortcut = None;
    events.retain(|event| {
        let egui::Event::Key {
            key,
            physical_key,
            modifiers,
            pressed,
            repeat,
        } = event
        else {
            return true;
        };
        if !command_only(*modifiers) {
            return true;
        }
        let action = if *key == Key::J && !number_panel {
            WheelShortcut::NextPage
        } else {
            let number_key = physical_key.unwrap_or(*key);
            let Some(index) = SHORTCUT_NUMBERS
                .iter()
                .position(|&number| number == number_key)
            else {
                return true;
            };
            if number_panel {
                WheelShortcut::Row(index)
            } else {
                WheelShortcut::Segment(index)
            }
        };
        if *pressed && !repeat {
            shortcut.get_or_insert(action);
        }
        false
    });
    shortcut
}

pub(super) fn take_wheel_copy(ctx: &egui::Context, search: Id, search_focused: bool) -> bool {
    let has_text_selection = search_focused
        && TextEdit::load_state(ctx, search)
            .and_then(|state| state.cursor.char_range())
            .is_some_and(|range| range.primary.index != range.secondary.index);
    ctx.input_mut(|input| {
        let mut copy = false;
        input.events.retain_mut(|event| {
            if matches!(event, egui::Event::Key { key: Key::C, pressed: true, modifiers, .. }
                if modifiers.ctrl && !modifiers.alt && !modifiers.shift && !modifiers.mac_cmd)
            {
                *event = egui::Event::Copy;
            }
            if matches!(event, egui::Event::Copy) && !has_text_selection {
                copy = true;
                false
            } else {
                true
            }
        });
        copy
    })
}

pub(super) fn search_id() -> Id {
    Id::new("wheel_search")
}

pub(super) fn group_hotkey_destination(store: &Store, id: u64) -> (Level, Option<usize>) {
    if store.children(Some(id)).is_empty()
        && let Some(group) = store.group(id)
    {
        let parent = group.parent.filter(|&parent| store.group(parent).is_some());
        if let Some(index) = store
            .children(parent)
            .iter()
            .position(|&sibling| sibling == id)
        {
            return (parent.map_or(Level::Root, Level::Group), Some(index));
        }
    }
    (Level::Group(id), None)
}

pub(super) enum SegAction {
    Open(Level),
    Panel(u64),
    Paste(Src),
    Nothing,
}

impl SegAction {
    pub(super) fn group_id(&self) -> Option<u64> {
        match self {
            Self::Open(Level::Group(id)) | Self::Panel(id) => Some(*id),
            _ => None,
        }
    }
}

/// Where a row in the right-hand card comes from, so it can be deleted.
#[derive(Clone, Copy, PartialEq, Hash)]
pub(super) enum Src {
    Fav(u64),
    Clip(u64),
    /// Sub-menu title row inside the card: not selectable.
    Header(u64),
}

/// One row in the right-hand card.
#[derive(Clone)]
pub(super) struct Item {
    pub title: String,
    pub detail: String,
    pub src: Src,
}

pub(super) struct Seg {
    pub label: String,
    pub action: SegAction,
    pub items: Vec<Item>,
    pub detail: String,
}

impl App {
    /// Leaf groups use the full side list. Groups with children have another wheel level.
    fn group_seg(&self, g: &Group) -> Seg {
        let favs = self.store.favorites_in(g.id);
        let subs = self.store.children(Some(g.id));
        let fav_item = |f: &Favorite| Item {
            title: f.title.clone(),
            detail: f.content.preview(90),
            src: Src::Fav(f.id),
        };
        let items: Vec<Item> = if subs.is_empty() {
            favs.iter().map(|f| fav_item(f)).collect()
        } else {
            // a group with sub-menus lists everything: each sub-menu name, then its commands
            let mut v: Vec<Item> = favs.iter().map(|f| fav_item(f)).collect();
            for &sid in &subs {
                let Some(sg) = self.store.group(sid) else {
                    continue;
                };
                v.push(Item {
                    title: sg.name.clone(),
                    detail: String::new(),
                    src: Src::Header(sid),
                });
                v.extend(self.store.favorites_in(sid).iter().map(|f| fav_item(f)));
            }
            v
        };
        let action = if subs.is_empty() {
            SegAction::Panel(g.id)
        } else {
            SegAction::Open(Level::Group(g.id))
        };
        Seg {
            label: g.name.clone(),
            action,
            items,
            detail: String::new(),
        }
    }

    pub(super) fn wheel_segments(&self) -> (Vec<Seg>, usize) {
        let all: Vec<Seg> = match self.wheel.level {
            Level::Root => {
                let mut v: Vec<Seg> = self
                    .store
                    .groups
                    .iter()
                    .filter(|g| g.parent.is_none())
                    .map(|g| self.group_seg(g))
                    .collect();
                v.push(Seg {
                    label: "Recent".into(),
                    action: SegAction::Open(Level::Recent),
                    items: self
                        .store
                        .history
                        .iter()
                        .take(LIST_MAX)
                        .map(|c| Item {
                            title: format!(
                                "{}{}",
                                ago(c.created),
                                c.source
                                    .as_ref()
                                    .map(|s| format!(" · {s}"))
                                    .unwrap_or_default()
                            ),
                            detail: c.content.preview(90),
                            src: Src::Clip(c.id),
                        })
                        .collect(),
                    detail: String::new(),
                });
                v
            }
            Level::Recent => self
                .store
                .history
                .iter()
                .take(PER_PAGE * 3)
                .map(|c| Seg {
                    label: c.content.preview(22),
                    action: SegAction::Paste(Src::Clip(c.id)),
                    items: Vec::new(),
                    detail: c.content.preview(600),
                })
                .collect(),
            Level::Group(gid) => {
                let mut v: Vec<Seg> = self
                    .store
                    .groups
                    .iter()
                    .filter(|g| g.parent == Some(gid))
                    .map(|g| self.group_seg(g))
                    .collect();
                v.extend(self.store.favorites_in(gid).into_iter().map(|f| Seg {
                    label: f.title.clone(),
                    action: SegAction::Paste(Src::Fav(f.id)),
                    items: Vec::new(),
                    detail: f.content.preview(600),
                }));
                v
            }
        };
        let pages = all.len().div_ceil(PER_PAGE).max(1);
        let page = self.wheel.page.min(pages - 1);
        let mut segs: Vec<Seg> = all
            .into_iter()
            .skip(page * PER_PAGE)
            .take(PER_PAGE)
            .collect();
        if segs.is_empty() {
            segs.push(Seg {
                label: "Empty".into(),
                action: SegAction::Nothing,
                items: Vec::new(),
                detail: "No favorites in this group. Click an item’s star in the panel to add it."
                    .into(),
            });
        }
        (segs, pages)
    }

    pub(super) fn wheel_title(&self) -> String {
        match self.wheel.level {
            Level::Root => "Favorites".into(),
            Level::Recent => "Recent".into(),
            Level::Group(g) => self.store.group_path(g),
        }
    }

    /// Items shown on the right: search results across favorites + history when typing,
    /// otherwise the lit wheel piece's items.
    pub(super) fn wheel_displayed_items(&self, seg: &Seg) -> Vec<Item> {
        if self.wheel_query.trim().is_empty() {
            seg.items.clone()
        } else {
            wheel_search(&self.store, &self.wheel_query)
        }
    }

    pub(super) fn wheel_activate(&mut self, ctx: &egui::Context, index: usize) {
        let (mut segs, _) = self.wheel_segments();
        if index >= segs.len() {
            return;
        }
        let seg = segs.swap_remove(index);
        match seg.action {
            SegAction::Open(level) => {
                ctx.memory_mut(|m| m.surrender_focus(search_id()));
                self.wheel = WheelState::new(level, ctx.input(|i| i.time));
                self.wheel_query.clear();
            }
            SegAction::Panel(_) => self.wheel_focus_group_panel(ctx, index, &seg.items),
            SegAction::Paste(source) => self.paste_wheel(source),
            SegAction::Nothing => {}
        }
    }

    fn wheel_focus_group_panel(&mut self, ctx: &egui::Context, index: usize, items: &[Item]) {
        ctx.memory_mut(|m| m.surrender_focus(search_id()));
        self.wheel_query.clear();
        self.wheel.sel = index;
        self.wheel.item = step_item(items, None, 1);
        self.wheel.number_panel = true;
        self.wheel.reset_card_scroll = true;
    }

    pub(super) fn wheel_numbered_segment(&mut self, ctx: &egui::Context, shortcut: WheelShortcut) {
        let (segs, pages) = self.wheel_segments();
        match shortcut {
            WheelShortcut::Segment(index) => {
                let Some(seg) = segs.get(index) else { return };
                self.wheel.sel = index;
                self.wheel.item = None;
                if let SegAction::Paste(source) = seg.action {
                    self.copy_wheel(source);
                } else if matches!(seg.action, SegAction::Panel(_)) {
                    self.wheel_focus_group_panel(ctx, index, &seg.items);
                } else {
                    self.wheel_activate(ctx, index);
                }
            }
            WheelShortcut::NextPage if pages > 1 => {
                self.wheel.page = (self.wheel.page + 1) % pages;
                self.wheel.sel = 0;
                self.wheel.item = None;
            }
            _ => {}
        }
    }

    pub(super) fn wheel_paste_displayed(&mut self, seg: &Seg, row: usize) {
        let items = self.wheel_displayed_items(seg);
        if let Some(item) = items
            .into_iter()
            .nth(row)
            .filter(|i| !matches!(i.src, Src::Header(_)))
        {
            self.paste_wheel(item.src);
        }
    }

    /// Drop of a dragged row: a favorite moves; a history entry becomes a favorite
    /// (or moves, when it already is one) in `group`, before favorite `before`.
    pub(super) fn wheel_drop(&mut self, src: Src, group: u64, before: Option<u64>) {
        let fav = match src {
            Src::Fav(id) => Some(id),
            Src::Clip(id) => {
                let content = self
                    .store
                    .history
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.content.clone());
                content.and_then(|c| {
                    if let Some(f) = self.store.favorites.iter().find(|f| f.content == c) {
                        Some(f.id)
                    } else {
                        self.add_fav_now(c, group);
                        self.store.favorites.last().map(|f| f.id)
                    }
                })
            }
            Src::Header(_) => None,
        };
        if let Some(id) = fav {
            self.store.move_favorite(id, group, before);
            self.store.reindex();
            self.wheel.item = None;
            self.dirty = true;
        }
    }

    /// Opens the favorite editor (in the panel, on the Favorites tab).
    pub(super) fn wheel_edit_fav(&mut self, ctx: &egui::Context, id: u64) {
        let Some(f) = self.store.favorites.iter().find(|f| f.id == id).cloned() else {
            return;
        };
        self.show(ctx, Mode::Panel, Level::Root);
        self.tab = Tab::Favorites;
        self.editor = Some(FavEditor {
            id: Some(id),
            title: f.title.clone(),
            text: match &f.content {
                Content::Text { text } => text.clone(),
                _ => String::new(),
            },
            content: f.content.clone(),
            group: f.group,
        });
    }

    /// Opens the editor for a history entry; saving makes it a favorite
    /// (an entry that already is a favorite opens that favorite instead).
    pub(super) fn wheel_edit_clip(&mut self, ctx: &egui::Context, clip_id: u64) {
        let Some(content) = self
            .store
            .history
            .iter()
            .find(|c| c.id == clip_id)
            .map(|c| c.content.clone())
        else {
            return;
        };
        if let Some(fid) = self
            .store
            .favorites
            .iter()
            .find(|f| f.content == content)
            .map(|f| f.id)
        {
            return self.wheel_edit_fav(ctx, fid);
        }
        let group = self.default_group();
        self.show(ctx, Mode::Panel, Level::Root);
        self.tab = Tab::Favorites;
        self.editor = Some(FavEditor {
            id: None,
            title: content.preview(28),
            text: match &content {
                Content::Text { text } => text.clone(),
                _ => String::new(),
            },
            content,
            group,
        });
    }

    pub(super) fn wheel_edit_row(&mut self, ctx: &egui::Context, src: Src) {
        match src {
            Src::Fav(id) => self.wheel_edit_fav(ctx, id),
            Src::Clip(id) => self.wheel_edit_clip(ctx, id),
            Src::Header(_) => {}
        }
    }

    /// Mouse clicks arm the row first. The keyboard shortcut deletes immediately.
    pub(super) fn wheel_delete_row(&mut self, src: Src, requires_confirmation: bool) {
        if requires_confirmation && self.wheel_del_armed != Some(src) {
            self.wheel_del_armed = Some(src);
            return;
        }
        self.wheel_del_armed = None;
        self.delete_item(src);
        self.wheel.item = None;
    }

    pub(super) fn wheel_back(&mut self, ctx: &egui::Context) {
        if self.wheel.number_panel {
            self.wheel.number_panel = false;
            self.wheel_query.clear();
            self.wheel.item = None;
            ctx.memory_mut(|m| m.surrender_focus(search_id()));
            return;
        }
        if !self.wheel_query.is_empty() {
            self.wheel_query.clear();
            self.wheel.item = None;
            return;
        }
        if self.wheel.item.is_some() {
            self.wheel.item = None;
            return;
        }
        match self.wheel.level {
            Level::Root => self.hide(),
            level => {
                let (target, sel) = match level {
                    Level::Group(g) => {
                        let parent = self.store.group(g).and_then(|x| x.parent);
                        let pos = self
                            .store
                            .children(parent)
                            .iter()
                            .position(|&id| id == g)
                            .unwrap_or(0);
                        (parent.map(Level::Group).unwrap_or(Level::Root), pos)
                    }
                    _ => (Level::Root, self.store.children(None).len()),
                };
                self.wheel = WheelState::new(target, ctx.input(|i| i.time));
                self.wheel.sel = sel % PER_PAGE;
                self.wheel.page = sel / PER_PAGE;
            }
        }
    }

    fn wheel_select(&mut self, segs: &[Seg], sel: usize, keep_list: bool) {
        self.wheel.sel = sel;
        self.wheel.item = if keep_list {
            step_item(&segs[sel].items, None, 1)
        } else {
            None
        };
    }

    pub(super) fn wheel_keys(&mut self, ctx: &egui::Context) {
        if self.pending_snippet.is_some() {
            return;
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::Z)) {
            self.undo_delete();
            return;
        }
        let (segs, pages) = self.wheel_segments();
        let n = segs.len();
        if self.wheel.sel >= n {
            self.wheel.sel = 0;
            self.wheel.item = None;
        }

        let search_id = search_id();
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) {
            ctx.memory_mut(|m| m.request_focus(search_id));
            return;
        }
        let search_focused = ctx.memory(|m| m.has_focus(search_id));

        let (esc, panel) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::COMMAND, Key::Comma),
            )
        });
        if panel {
            self.show(ctx, Mode::Panel, Level::Root);
            self.tab = Tab::Favorites;
            return;
        }
        if esc {
            if self.wheel.number_panel {
                self.wheel_back(ctx);
                return;
            }
            // egui releases the focused widget before the app receives Escape.
            let was_searching = ctx.memory(|m| m.had_focus_last_frame(search_id));
            if (search_focused || was_searching) && self.wheel_query.trim().is_empty() {
                self.wheel_query.clear();
                self.wheel.item = None;
                ctx.memory_mut(|m| m.surrender_focus(search_id));
            } else {
                if was_searching && !self.wheel_query.trim().is_empty() {
                    ctx.memory_mut(|m| m.request_focus(search_id));
                }
                self.wheel_back(ctx);
            }
            return;
        }

        let row_action = ctx.input_mut(|i| take_wheel_row_action(&mut i.events));
        if let Some(action) = row_action {
            if let Some(row) = self.wheel.item {
                let displayed = self.wheel_displayed_items(&segs[self.wheel.sel]);
                if let Some(item) = displayed
                    .get(row)
                    .filter(|item| !matches!(item.src, Src::Header(_)))
                {
                    match action {
                        WheelRowAction::Edit => self.wheel_edit_row(ctx, item.src),
                        WheelRowAction::Delete => self.wheel_delete_row(item.src, false),
                    }
                }
            }
            return;
        }

        if search_focused || !self.wheel_query.trim().is_empty() {
            return;
        }

        let displayed = self.wheel_displayed_items(&segs[self.wheel.sel]);
        let rows = displayed.len();
        let (up, down, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        let (left, right, space, back, tab) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowLeft),
                i.consume_key(Modifiers::NONE, Key::ArrowRight),
                i.consume_key(Modifiers::NONE, Key::Space),
                i.consume_key(Modifiers::NONE, Key::Backspace),
                i.consume_key(Modifiers::NONE, Key::Tab),
            )
        });
        let enter = enter || space;
        if back {
            self.wheel_back(ctx);
            return;
        }
        if tab && pages > 1 && !self.wheel.number_panel {
            self.wheel.page = (self.wheel.page + 1) % pages;
            self.wheel.sel = 0;
            self.wheel.item = None;
            return;
        }
        let in_list = self.wheel.item.is_some();
        if right && !self.wheel.number_panel {
            self.wheel_select(&segs, (self.wheel.sel + 1) % n, in_list);
        }
        if left && !self.wheel.number_panel {
            self.wheel_select(&segs, (self.wheel.sel + n - 1) % n, in_list);
        }
        match self.wheel.item {
            Some(it) if rows > 0 => {
                if down {
                    self.wheel.item = step_item(&displayed, Some(it), 1);
                }
                if up {
                    self.wheel.item = step_item(&displayed, Some(it), -1);
                }
                if enter {
                    self.wheel_paste_displayed(&segs[self.wheel.sel], it.min(rows - 1));
                    return;
                }
            }
            _ => {
                if enter
                    && !self.wheel.number_panel
                    && matches!(segs[self.wheel.sel].action, SegAction::Panel(_))
                {
                    self.wheel_activate(ctx, self.wheel.sel);
                    return;
                }
                if down || (enter && rows > 0) {
                    if rows > 0 {
                        self.wheel.item = step_item(&displayed, None, 1);
                    } else if down && !self.wheel.number_panel {
                        self.wheel_select(&segs, (self.wheel.sel + 1) % n, false);
                    }
                } else if up && !self.wheel.number_panel {
                    self.wheel_select(&segs, (self.wheel.sel + n - 1) % n, false);
                }
                if enter && rows == 0 {
                    self.wheel_activate(ctx, self.wheel.sel);
                    return;
                }
            }
        }
        if (up || down) && self.wheel.item.is_some() {
            self.wheel.number_panel = true;
        }
        if (up || down || left || right) && self.wheel.item.is_some() {
            ctx.data_mut(|data| data.insert_temp(Id::new(SCROLL_TO_ROW), true));
        }
        // Unmodified numbers and the legacy Ctrl/Shift variants navigate the lit ring or card.
        let nums = SHORTCUT_NUMBERS;
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.command && !modifiers.alt => nums.iter().position(|k| k == key),
                _ => None,
            })
        });
        let Some(k) = pressed else { return };
        ctx.input_mut(|i| {
            i.events
                .retain(|e| !matches!(e, egui::Event::Key { key, .. } if nums.contains(key)))
        });
        if self.wheel.item.is_some() {
            if k < rows {
                self.wheel_paste_displayed(&segs[self.wheel.sel], k);
            }
        } else if k < n && !self.wheel.number_panel {
            if segs[k].items.is_empty() || matches!(segs[k].action, SegAction::Panel(_)) {
                self.wheel.sel = k;
                self.wheel_activate(ctx, k);
            } else {
                self.wheel_select(&segs, k, true);
            }
        }
    }
}

fn wheel_search(store: &Store, query: &str) -> Vec<Item> {
    let now = store::now();
    let paths: HashMap<_, _> = store
        .groups
        .iter()
        .map(|g| (g.id, store.group_path(g.id)))
        .collect();
    let favorites = store.favorites.iter().filter_map(|favorite| {
        let path = paths.get(&favorite.group).map_or("", String::as_str);
        let score = store::score_favorite(path, &favorite.lower, query)?;
        let title = if path.is_empty() {
            favorite.title.clone()
        } else {
            format!("{path} · {}", favorite.title)
        };
        let item = Item {
            title,
            detail: favorite.content.preview(90),
            src: Src::Fav(favorite.id),
        };
        Some((
            score,
            true,
            store::frecency(favorite.uses, favorite.last_used, now),
            0,
            item,
        ))
    });
    let history = store.history.iter().filter_map(|clip| {
        let score = store::score(&clip.lower, query)?;
        let title = format!(
            "{}{}",
            ago(clip.created),
            clip.source
                .as_ref()
                .map(|source| format!(" · {source}"))
                .unwrap_or_default()
        );
        let item = Item {
            title,
            detail: clip.content.preview(90),
            src: Src::Clip(clip.id),
        };
        Some((score, false, 0.0, clip.created, item))
    });
    let mut hits: Vec<_> = favorites.chain(history).collect();
    // On equal scores, keep favorites ahead of history. Each type has its own tie-break.
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| b.2.total_cmp(&a.2))
            .then_with(|| b.3.cmp(&a.3))
    });
    hits.into_iter()
        .take(20)
        .map(|(_, _, _, _, item)| item)
        .collect()
}

/// Next selectable row (headers are skipped), wrapping around; `dir` is 1 or -1.
pub(super) fn step_item(items: &[Item], from: Option<usize>, dir: isize) -> Option<usize> {
    let n = items.len() as isize;
    if n == 0 {
        return None;
    }
    let mut i = match from {
        Some(f) => f as isize,
        None if dir > 0 => -1,
        None => n,
    };
    for _ in 0..n {
        i = (i + dir).rem_euclid(n);
        if !matches!(items[i as usize].src, Src::Header(_)) {
            return Some(i as usize);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(key: Key, physical_key: Key, modifiers: Modifiers, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(physical_key),
            pressed: true,
            repeat,
            modifiers,
        }
    }

    #[test]
    fn wheel_search_ranks_before_limiting_results_and_keeps_favorite_ids() {
        let mut store = Store::default();
        for id in 1..=21 {
            store.favorites.push(Favorite {
                id,
                title: "Find".into(),
                content: Content::Text {
                    text: "needle".into(),
                },
                uses: id as u32,
                last_used: store::now(),
                ..Favorite::default()
            });
        }
        store.history.push_back(Clip {
            id: 50,
            content: Content::Text {
                text: "needle".into(),
            },
            ..Clip::default()
        });
        store.reindex();
        let items = wheel_search(&store, "needle");
        assert_eq!(items.len(), 20);
        assert!(matches!(items[0].src, Src::Clip(50)));
        assert!(matches!(items[1].src, Src::Fav(21)));
        assert!(matches!(items[2].src, Src::Fav(20)));
    }

    fn command_mods() -> Modifiers {
        if cfg!(target_os = "macos") {
            Modifiers::MAC_CMD | Modifiers::COMMAND
        } else {
            Modifiers::CTRL | Modifiers::COMMAND
        }
    }

    #[test]
    fn row_actions_consume_command_chords_without_repeating_delete() {
        let command = command_mods();
        let plain_back = event(Key::Backspace, Key::Backspace, Modifiers::NONE, false);
        let shifted_back = event(
            Key::Backspace,
            Key::Backspace,
            command | Modifiers::SHIFT,
            false,
        );
        let mut events = vec![
            plain_back.clone(),
            shifted_back.clone(),
            event(Key::Backspace, Key::Backspace, command, true),
        ];
        assert_eq!(take_wheel_row_action(&mut events), None);
        assert_eq!(events, [plain_back, shifted_back]);

        let mut edit = vec![event(Key::E, Key::E, command, false)];
        assert_eq!(take_wheel_row_action(&mut edit), Some(WheelRowAction::Edit));
        assert!(edit.is_empty());
        let mut delete = vec![event(Key::Backspace, Key::Backspace, command, false)];
        assert_eq!(
            take_wheel_row_action(&mut delete),
            Some(WheelRowAction::Delete)
        );
        assert!(delete.is_empty());
    }

    #[test]
    fn command_numbers_use_physical_key_and_preserve_search_text() {
        let mut events = vec![
            egui::Event::Text("dns ".into()),
            event(Key::A, Key::Num1, command_mods(), false),
            event(Key::A, Key::A, Modifiers::NONE, false),
            egui::Event::Text("a".into()),
        ];
        assert_eq!(
            take_wheel_shortcut(&mut events, false),
            Some(WheelShortcut::Segment(0))
        );
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], egui::Event::Text("dns ".into()));
        assert_eq!(events[2], egui::Event::Text("a".into()));
    }

    #[test]
    fn command_zero_targets_the_active_area_and_ignores_repeat() {
        let modifiers = command_mods();
        let key = event(Key::Num0, Key::Num0, modifiers, false);
        assert_eq!(
            take_wheel_shortcut(&mut vec![key.clone()], false),
            Some(WheelShortcut::Segment(9))
        );
        assert_eq!(
            take_wheel_shortcut(&mut vec![key], true),
            Some(WheelShortcut::Row(9))
        );

        let mut events = vec![event(Key::Num0, Key::Num0, modifiers, true)];
        assert_eq!(take_wheel_shortcut(&mut events, true), None);
        assert!(events.is_empty());
    }

    #[test]
    fn command_shift_digits_and_command_tab_are_reserved() {
        let command = command_mods();
        let mut reserved = vec![
            event(Key::Num3, Key::Num3, command | Modifiers::SHIFT, false),
            event(Key::Tab, Key::Tab, command, false),
        ];
        assert_eq!(take_wheel_shortcut(&mut reserved, false), None);
        assert_eq!(reserved.len(), 2);
        let mut next_page = vec![event(Key::J, Key::J, command, false)];
        assert_eq!(take_wheel_shortcut(&mut next_page.clone(), true), None);
        assert_eq!(
            take_wheel_shortcut(&mut next_page, false),
            Some(WheelShortcut::NextPage)
        );
    }

    #[test]
    fn command_control_digits_do_not_activate_a_card() {
        let extra = if cfg!(target_os = "macos") {
            Modifiers::CTRL
        } else {
            Modifiers::ALT
        };
        let modifiers = command_mods() | extra;
        let mut events = vec![event(Key::Num1, Key::Num1, modifiers, false)];
        assert_eq!(take_wheel_shortcut(&mut events, true), None);
    }

    #[test]
    fn group_hotkeys_keep_leaf_groups_on_the_parent_wheel() {
        let mut store = Store::default();
        for (id, name, parent) in [
            (1, "dns-commands", None),
            (2, "performance-tools", None),
            (3, "CPU", Some(2)),
        ] {
            store.groups.push(Group {
                id,
                name: name.into(),
                color: PALETTE[0],
                hotkey: String::new(),
                parent,
            });
        }
        let (level, panel) = group_hotkey_destination(&store, 1);
        assert!(level == Level::Root);
        assert_eq!(panel, Some(0));
        let (level, panel) = group_hotkey_destination(&store, 2);
        assert!(level == Level::Group(2));
        assert_eq!(panel, None);
        let (level, panel) = group_hotkey_destination(&store, 3);
        assert!(level == Level::Group(2));
        assert_eq!(panel, Some(0));
    }
}
