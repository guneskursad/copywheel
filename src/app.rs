//! The window: a floating panel (history, favorites, settings) and the wheel menu.

mod forms;
mod icons;
mod panel;
mod shapes;
mod theme;
mod wheel;
mod wheel_draw;
mod widgets;

use crate::platform;
use crate::store::{self, ago, Clip, Content, Favorite, Group, Store, Theme, PALETTE};
use wheel::Src;
use crate::watch::{self, Msg};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Frame, Id, Key, Layout, Margin,
    Modifiers, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextureHandle,
    Ui, ViewportCommand,
};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::collections::{HashMap, HashSet};
use std::f32::consts::{PI, TAU};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use icons::*;
use theme::{mix, palette, with_alpha};
use widgets::*;

pub const WIN_W: f32 = 760.0;
pub const WIN_H: f32 = 480.0;
/// The wheel in the middle, with room for its search/command card on either side
/// (the card opens beside the chosen piece; the unused side is see-through).
pub const WHEEL_W: f32 = 1100.0;
pub const WHEEL_H: f32 = 470.0;

const DELETE_ICON_GAP: f32 = 6.0;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Panel,
    Wheel,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    History,
    Favorites,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum HotAction {
    Panel,
    Wheel,
    Favorites,
    Group(u64),
}

/// Pieces on one page of the wheel.
const PER_PAGE: usize = 10;

/// What the wheel shows: the groups, the latest copies, or one group's favorites.
#[derive(Clone, Copy, PartialEq)]
enum Level {
    Root,
    Recent,
    Group(u64),
}

/// `sel` is the lit piece; `item` is the lit row in the card beside the wheel
/// (None while the keys move on the ring).
struct WheelState {
    level: Level,
    sel: usize,
    page: usize,
    item: Option<usize>,
    number_panel: bool,
    reset_card_scroll: bool,
    opened: f64,
}

impl WheelState {
    fn new(level: Level, opened: f64) -> Self {
        Self { level, sel: 0, page: 0, item: None, number_panel: false, reset_card_scroll: false, opened }
    }
}

struct FavEditor {
    id: Option<u64>,
    title: String,
    text: String,
    content: Content,
    group: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum RecTarget {
    Panel,
    Wheel,
    Favorites,
    Group,
}

struct GroupEdit {
    id: u64,
    name: String,
    hotkey: String,
    color: [u8; 3],
}

enum Action {
    EditFav(u64),
    NewFav(u64),
    EditGroup(u64),
    DeleteGroup(u64),
}

enum DeletedRow {
    Clip(usize, Clip),
    Favorite(usize, Favorite),
}

impl DeletedRow {
    fn content(&self) -> &Content {
        match self {
            Self::Clip(_, clip) => &clip.content,
            Self::Favorite(_, favorite) => &favorite.content,
        }
    }
}

struct DeletedItem {
    row: DeletedRow,
    image: Option<Vec<u8>>,
}

struct Hotkeys {
    mgr: Option<GlobalHotKeyManager>,
    registered: Vec<HotKey>,
    map: HashMap<u32, HotAction>,
}

impl Hotkeys {
    fn apply(&mut self, store: &Store) -> Vec<String> {
        let Some(mgr) = &self.mgr else {
            return vec!["Could not start global shortcuts.".into()];
        };
        let _ = mgr.unregister_all(&self.registered);
        self.registered.clear();
        self.map.clear();
        let mut list = vec![
            (store.settings.hotkey_panel.clone(), HotAction::Panel, "Panel".to_string()),
            (store.settings.hotkey_wheel.clone(), HotAction::Wheel, "Wheel".to_string()),
            (store.settings.hotkey_favorites.clone(), HotAction::Favorites, "Favorites list".to_string()),
        ];
        for g in &store.groups {
            list.push((g.hotkey.clone(), HotAction::Group(g.id), g.name.clone()));
        }
        let mut errors = Vec::new();
        for (text, action, name) in list {
            if text.trim().is_empty() {
                continue;
            }
            match text.trim().parse::<HotKey>() {
                Ok(hk) => match mgr.register(hk) {
                    Ok(()) => {
                        self.registered.push(hk);
                        self.map.insert(hk.id(), action);
                    }
                    Err(_) => errors.push(format!("{name}: \"{text}\" is already in use.")),
                },
                Err(_) => errors.push(format!("{name}: \"{text}\" is not a valid shortcut (e.g. Cmd+Shift+V).")),
            }
        }
        errors
    }
}

struct ClipboardRequest {
    items: Vec<(Src, Content)>,
    paste: bool,
    close: bool,
}

struct PendingSnippet {
    prompt: forms::SnippetPrompt,
    request: ClipboardRequest,
    return_focus: Option<Id>,
}

fn snippet_text(source: Src, content: &Content) -> Option<&str> {
    match (source, content) {
        (Src::Fav(_), Content::Text { text }) if crate::snippet::has_vars(text) => Some(text),
        _ => None,
    }
}

pub struct App {
    store: Store,
    dirty: bool,
    last_save: f64,
    rx: Receiver<Msg>,
    hotkeys: Hotkeys,
    hotkey_errors: Vec<String>,
    _tray: Option<tray_icon::TrayIcon>,
    skip_concealed: Arc<AtomicBool>,

    mode: Mode,
    tab: Tab,
    query: String,
    sel: usize,
    sel_moved: bool,
    scroll_want: Option<f32>,
    preview_scroll: f32,
    panel_popup: Option<panel::PanelPopup>,
    pending_snippet: Option<PendingSnippet>,
    deleted: Option<DeletedItem>,
    wheel_root_position: (usize, usize),
    wheel: WheelState,
    wheel_query: String,
    wheel_del_armed: Option<wheel::Src>,
    /// Favorite being dragged inside the wheel card.
    wheel_drag: Option<wheel::Src>,
    /// Groups expanded in the panel's favorites list (all start collapsed).
    open_groups: HashSet<u64>,
    editor: Option<FavEditor>,
    group_edit: Option<GroupEdit>,
    confirm_group_delete: Option<u64>,
    confirm_clear: bool,
    backup_note: Option<(bool, String)>,
    recording: Option<RecTarget>,
    textures: HashMap<String, Option<TextureHandle>>,
    /// Rows marked for a bulk action (history clip ids or favorite ids).
    marks: HashSet<u64>,
    /// Where a Shift range starts (row position in the current list).
    anchor: Option<usize>,
    /// The tab and search the marks belong to; changing either drops them.
    marks_ctx: (Tab, String),
    /// Group used for quick (Shift) adds: the last one picked.
    last_group: Option<u64>,

    visible: bool,
    pinned: bool,
    was_focused: bool,
    shot: Option<PathBuf>,
    shot_theme: Option<Theme>,
    frame: u64,
    /// What we last put on the clipboard ourselves, so the watcher can tell it apart.
    own_paste: Option<Content>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>) -> Self {
        let ctx = cc.egui_ctx.clone();
        setup_style(&ctx);

        let shot_theme = shot.as_ref().and_then(|_| {
            match std::env::var("COPYWHEEL_THEME").as_deref() {
                Ok("light") => Some(Theme::Light),
                Ok("dark") => Some(Theme::Dark),
                _ => None,
            }
        });
        let mut store = Store::load(shot.is_some());
        theme::apply(&ctx, shot_theme.unwrap_or(store.settings.theme));
        if shot.is_some() && store.load_notice.is_none() && store.history.is_empty() {
            store.demo_history();
        }

        let (tx, rx) = channel::<Msg>();
        let skip_concealed = Arc::new(AtomicBool::new(store.settings.skip_concealed));
        if shot.is_none() {
            watch::spawn(tx.clone(), ctx.clone(), skip_concealed.clone());
        }

        let mut hotkeys = Hotkeys { mgr: None, registered: Vec::new(), map: HashMap::new() };
        let mut hotkey_errors = Vec::new();
        let mut tray = None;
        if shot.is_none() {
            hotkeys.mgr = GlobalHotKeyManager::new().ok();
            hotkey_errors = hotkeys.apply(&store);
            install_hotkey_handler(tx.clone(), ctx.clone());
            tray = build_tray(tx.clone(), ctx.clone());
        }

        // Start view for testing pictures: wheel, favorites or settings.
        let start = std::env::var("COPYWHEEL_START").unwrap_or_default();
        let mut app = Self {
            store,
            dirty: false,
            last_save: 0.0,
            rx,
            hotkeys,
            hotkey_errors,
            _tray: tray,
            skip_concealed,
            mode: if start.starts_with("wheel") { Mode::Wheel } else { Mode::Panel },
            tab: match start.as_str() {
                "favorites" => Tab::Favorites,
                "settings" => Tab::Settings,
                _ => Tab::History,
            },
            query: String::new(),
            sel: 0,
            sel_moved: false,
            scroll_want: None,
            preview_scroll: 0.0,
            panel_popup: None,
            pending_snippet: None,
            deleted: None,
            wheel_root_position: (0, 0),
            // "wheel-list": the second group lit, keys in its list
            // "wheel-<n>": piece n lit (to see where its list card opens)
            wheel: if matches!(start.as_str(), "wheel-list" | "wheel-snippet") {
                WheelState { sel: 1, item: Some(0), ..WheelState::new(Level::Root, 0.0) }
            } else if let Some(n) = start.strip_prefix("wheel-").and_then(|s| s.parse().ok()) {
                WheelState { sel: n, ..WheelState::new(Level::Root, 0.0) }
            } else {
                WheelState::new(Level::Root, 0.0)
            },
            wheel_query: String::new(),
            wheel_del_armed: None,
            wheel_drag: None,
            open_groups: HashSet::new(),
            editor: None,
            group_edit: None,
            confirm_group_delete: None,
            confirm_clear: false,
            backup_note: None,
            recording: None,
            textures: HashMap::new(),
            marks: HashSet::new(),
            anchor: None,
            marks_ctx: (Tab::History, String::new()),
            last_group: None,
            visible: true,
            pinned: false,
            was_focused: false,
            shot,
            shot_theme,
            frame: 0,
            own_paste: None,
        };
        if app.shot.is_some() && start == "wheel-snippet" {
            app.pending_snippet = Some(PendingSnippet {
                prompt: forms::SnippetPrompt::new(
                    "DNS lookup",
                    vec!["dig @{{resolver:10.0.0.53}} {{domain}} A +notcp".into()],
                    &HashMap::new(),
                ),
                request: ClipboardRequest { items: Vec::new(), paste: true, close: true },
                return_focus: None,
            });
        }
        app
    }

    // ---------- showing and hiding ----------

    fn show(&mut self, ctx: &egui::Context, mode: Mode, level: Level) {
        self.remember_wheel_root();
        self.mode = mode;
        self.panel_popup = None;
        self.pending_snippet = None;
        self.query.clear();
        self.wheel_query.clear();
        self.wheel_del_armed = None;
        self.sel = 0;
        self.scroll_want = Some(0.0);
        self.editor = None;
        self.clear_marks();
        self.wheel = WheelState::new(level, ctx.input(|i| i.time));
        if mode == Mode::Wheel && level == Level::Root {
            (self.wheel.sel, self.wheel.page) = self.wheel_root_position;
            let (_, pages) = self.wheel_segments();
            self.wheel.page = self.wheel.page.min(pages.saturating_sub(1));
            let (segments, _) = self.wheel_segments();
            self.wheel.sel = self.wheel.sel.min(segments.len().saturating_sub(1));
        }
        ctx.memory_mut(|m| {
            if mode == Mode::Wheel && level == Level::Root {
                m.request_focus(wheel::search_id());
            } else {
                m.surrender_focus(wheel::search_id());
            }
        });
        let (w, h) = match mode {
            Mode::Wheel => (WHEEL_W, WHEEL_H),
            Mode::Panel => (WIN_W, WIN_H),
        };
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(w, h)));
        if let Some((x, y)) = platform::cursor_pos() {
            let monitor = ctx.input(|i| i.viewport().monitor_size).unwrap_or(vec2(4000.0, 3000.0));
            let (mut px, mut py) = match mode {
                Mode::Wheel => (x - w / 2.0, y - h / 2.0),
                Mode::Panel => (x - w / 2.0, y - 30.0),
            };
            if mode == Mode::Panel {
                px = px.clamp(0.0, (monitor.x - w).max(0.0));
                py = py.clamp(0.0, (monitor.y - h).max(0.0));
            } else {
                // keep the whole wheel and both places for its list on screen
                px = px.clamp(-10.0, (monitor.x - w + 10.0).max(0.0));
                py = py.clamp(-60.0, (monitor.y - h + 60.0).max(0.0));
            }
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos2(px, py)));
        }
        platform::show_app();
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        self.visible = true;
        self.was_focused = false;
        ctx.request_repaint();
    }

    fn hide(&mut self) {
        if self.shot.is_some() {
            return;
        }
        self.remember_wheel_root();
        self.visible = false;
        self.panel_popup = None;
        self.pending_snippet = None;
        self.editor = None;
        self.group_edit = None;
        platform::hide_app();
    }

    fn remember_wheel_transition(&mut self, previous_level: Level) {
        if self.mode == Mode::Wheel && previous_level == Level::Root && self.wheel.level != Level::Root {
            let roots = self.store.children(None);
            let index = match self.wheel.level {
                Level::Recent => Some(roots.len()),
                Level::Group(group) => roots.iter().position(|&id| id == group),
                Level::Root => None,
            };
            if let Some(index) = index {
                self.wheel_root_position = (index % PER_PAGE, index / PER_PAGE);
            }
        } else {
            self.remember_wheel_root();
        }
    }

    fn remember_wheel_root(&mut self) {
        if self.mode == Mode::Wheel && self.wheel.level == Level::Root {
            self.wheel_root_position = (self.wheel.sel, self.wheel.page);
        }
    }

    fn paste_wheel(&mut self, source: Src) {
        self.request_clipboard(&[source], true, true);
    }

    fn copy_wheel(&mut self, source: Src) {
        self.request_clipboard(&[source], false, !self.pinned);
    }

    fn source_content(&self, source: Src) -> Option<&Content> {
        match source {
            Src::Clip(id) => self.store.history.iter().find(|c| c.id == id).map(|c| &c.content),
            Src::Fav(id) => self.store.favorites.iter().find(|f| f.id == id).map(|f| &f.content),
            Src::Header(_) => None,
        }
    }

    fn touch_source(&mut self, source: Src) {
        match source {
            Src::Clip(id) => self.store.touch_clip(id),
            Src::Fav(id) => self.store.touch_favorite(id),
            Src::Header(_) => return,
        }
        self.dirty = true;
    }

    fn request_clipboard(&mut self, sources: &[Src], paste: bool, close: bool) {
        if self.shot.is_some() || self.pending_snippet.is_some() {
            return;
        }
        let items: Vec<_> = sources.iter().filter_map(|&source| {
            self.source_content(source).map(|content| (source, content.clone()))
        }).collect();
        if items.is_empty() {
            return;
        }
        let templates: Vec<_> = items.iter().filter_map(|(source, content)| {
            snippet_text(*source, content).map(str::to_owned)
        }).collect();
        let request = ClipboardRequest { items, paste, close };
        if templates.is_empty() {
            self.finish_clipboard(request);
            return;
        }
        let title = match request.items.as_slice() {
            [(Src::Fav(id), _)] => self.store.favorites.iter()
                .find(|favorite| favorite.id == *id).map_or("", |favorite| favorite.title.as_str()),
            _ => "Fill favorite snippets",
        };
        self.pending_snippet = Some(PendingSnippet {
            prompt: forms::SnippetPrompt::new(title, templates, &self.store.snippet_values),
            request,
            return_focus: None,
        });
        self.wheel_drag = None;
    }

    fn draw_snippet_prompt(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_snippet.as_mut() else { return };
        let action = if pending.request.paste { "Paste" } else { "Copy" };
        let return_focus = pending.return_focus;
        match forms::snippet_prompt(ctx, &mut pending.prompt, action) {
            forms::SnippetOutcome::Pending => return,
            forms::SnippetOutcome::Cancel => {
                self.pending_snippet = None;
            }
            forms::SnippetOutcome::Submit(values) => {
                let mut request = self.pending_snippet.take().unwrap().request;
                let values: HashMap<_, _> = values.into_iter().collect();
                for (source, content) in &mut request.items {
                    if let Some(text) = snippet_text(*source, content) {
                        *content = Content::Text { text: crate::snippet::fill(text, &values) };
                    }
                }
                if self.shot.is_none() {
                    self.store.snippet_values.extend(values);
                    self.dirty = true;
                    self.finish_clipboard(request);
                }
            }
        }
        if self.visible && let Some(id) = return_focus {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
        ctx.request_repaint();
    }

    fn finish_clipboard(&mut self, request: ClipboardRequest) {
        let copied = match request.items.as_slice() {
            [(_, content)] => self.copy_to_clipboard(content.clone()),
            items => {
                let texts: Option<Vec<_>> = items.iter().map(|(_, content)| match content {
                    Content::Text { text } => Some(text.as_str()),
                    Content::Image { .. } => None,
                }).collect();
                if let Some(texts) = texts {
                    self.copy_to_clipboard(Content::Text { text: texts.join("\n") })
                } else {
                    let contents: Vec<_> = items.iter().map(|(_, content)| content).collect();
                    let copied = panel::copy_multiple_items(&contents);
                    if copied {
                        self.own_paste = contents.iter().copied()
                            .find(|content| matches!(content, Content::Text { .. }))
                            .or_else(|| contents.first().copied()).cloned();
                    }
                    copied
                }
            }
        };
        if copied {
            for (source, _) in request.items {
                self.touch_source(source);
            }
            if request.close {
                self.hide();
            }
            if request.paste {
                self.post_auto_paste();
            }
        }
    }

    fn post_auto_paste(&self) {
        if self.store.settings.auto_paste && platform::accessibility_ok() {
            platform::post_paste();
        }
    }

    fn delete_item(&mut self, source: Src) {
        if self.shot.is_some() {
            return;
        }
        let row = match source {
            Src::Clip(id) => self.store.history.iter().enumerate().find(|(_, c)| c.id == id)
                .map(|(index, clip)| DeletedRow::Clip(index, clip.clone())),
            Src::Fav(id) => self.store.favorites.iter().enumerate().find(|(_, f)| f.id == id)
                .map(|(index, favorite)| DeletedRow::Favorite(index, favorite.clone())),
            Src::Header(_) => None,
        };
        if let Some(row) = row {
            let image = row.content().image_file()
                .and_then(|file| store::image_path(file).and_then(std::fs::read).ok());
            match source {
                Src::Clip(id) => self.store.remove_clip(id),
                Src::Fav(id) => self.store.remove_favorite(id),
                Src::Header(_) => return,
            }
            self.deleted = Some(DeletedItem { row, image });
            self.dirty = true;
        }
    }

    fn undo_delete(&mut self) {
        let Some(deleted) = self.deleted.take() else { return };
        if let (Some(file), Some(bytes)) = (deleted.row.content().image_file(), &deleted.image)
            && store::save_image(file, bytes).is_err()
        {
            self.deleted = Some(deleted);
            return;
        }
        match deleted.row {
            DeletedRow::Clip(index, clip) => {
                self.store.history.insert(index.min(self.store.history.len()), clip);
            }
            DeletedRow::Favorite(index, mut favorite) => {
                if self.store.group(favorite.group).is_none() {
                    favorite.group = self.default_group();
                }
                self.store.favorites.insert(index.min(self.store.favorites.len()), favorite);
            }
        }
        self.dirty = true;
    }

    fn copy_to_clipboard(&mut self, content: Content) -> bool {
        if self.shot.is_some() {
            return false;
        }
        self.own_paste = Some(content.clone());
        if !watch::set_clipboard(&content) {
            self.own_paste = None;
            return false;
        }
        true
    }

    // ---------- events ----------

    fn handle_messages(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Clip(content, hash, source) => {
                    // A copy we made ourselves by pasting an entry: move that entry up
                    // instead of adding it again (features.moveCopiedSnapshotToTheTop).
                    if let Some(own) = self.own_paste.take() {
                        let same = match (&own, &content) {
                            (Content::Image { w, h, .. }, Content::Image { w: w2, h: h2, .. }) => w == w2 && h == h2,
                            (a, b) => a == b,
                        };
                        if same && self.store.settings.move_used_to_top && self.store.bring_to_top(&own) {
                            self.dirty = true;
                            continue;
                        }
                    }
                    if self.store.add_clip(content, hash, source) {
                        self.dirty = true;
                    }
                }
                Msg::Hotkey(id) => {
                    if self.pending_snippet.is_some() {
                        platform::show_app();
                        ctx.send_viewport_cmd(ViewportCommand::Focus);
                        continue;
                    }
                    let Some(action) = self.hotkeys.map.get(&id).copied() else { continue };
                    if action == HotAction::Panel && self.visible && self.mode == Mode::Panel {
                        self.switch_panel_tab();
                        ctx.send_viewport_cmd(ViewportCommand::Focus);
                        continue;
                    }
                    let (mode, level, group_panel) = match action {
                        HotAction::Panel | HotAction::Favorites => (Mode::Panel, Level::Root, None),
                        HotAction::Wheel => (Mode::Wheel, Level::Root, None),
                        HotAction::Group(g) => {
                            let (level, group_panel) = wheel::group_hotkey_destination(&self.store, g);
                            (Mode::Wheel, level, group_panel)
                        }
                    };
                    let want_tab = if action == HotAction::Favorites { Tab::Favorites } else { Tab::History };
                    let focused = ctx.input(|i| i.viewport().focused).unwrap_or(false);
                    let same = self.mode == mode && match mode {
                        Mode::Panel => self.tab == want_tab,
                        Mode::Wheel => self.wheel.level == level && group_panel.is_none_or(|index| {
                            self.wheel.number_panel && self.wheel.page == index / PER_PAGE
                                && self.wheel.sel == index % PER_PAGE
                        }),
                    };
                    if self.visible && focused && same {
                        self.hide();
                    } else {
                        self.show(ctx, mode, level);
                        if mode == Mode::Panel {
                            self.tab = want_tab;
                        } else if let Some(index) = group_panel {
                            self.wheel.page = index / PER_PAGE;
                            self.wheel_activate(ctx, index % PER_PAGE);
                        }
                    }
                }
                Msg::Menu(id) => match id.as_str() {
                    "panel" => {
                        self.tab = Tab::History;
                        self.show(ctx, Mode::Panel, Level::Root)
                    }
                    "wheel" => self.show(ctx, Mode::Wheel, Level::Root),
                    "favorites" => {
                        self.show(ctx, Mode::Panel, Level::Root);
                        self.tab = Tab::Favorites;
                    }
                    "settings" => {
                        self.show(ctx, Mode::Panel, Level::Root);
                        self.tab = Tab::Settings;
                    }
                    "quit" => {
                        self.store.save();
                        std::process::exit(0);
                    }
                    _ => {}
                },
            }
        }
    }

    fn reapply_hotkeys(&mut self) {
        if self.hotkeys.mgr.is_some() {
            self.hotkey_errors = self.hotkeys.apply(&self.store);
        }
    }

    // ---------- lists ----------

    fn history_matches(&self) -> Vec<usize> {
        history_order(&self.store, &self.query)
    }

    /// Favorites in display order (grouped), filtered by the search.
    /// Group ids as a tree walk: each group followed by its sub-groups.
    fn group_order(&self) -> Vec<(u64, usize)> {
        fn walk(s: &Store, parent: Option<u64>, depth: usize, out: &mut Vec<(u64, usize)>) {
            for id in s.children(parent) {
                out.push((id, depth));
                if depth < 8 {
                    walk(s, Some(id), depth + 1, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.store, None, 0, &mut out);
        // groups whose parent is gone still show up, at the top level
        for g in &self.store.groups {
            if !out.iter().any(|(id, _)| *id == g.id) {
                out.push((g.id, 0));
            }
        }
        out
    }

    /// Open means: its own list is expanded and so is every group above it.
    fn group_open(&self, id: u64) -> bool {
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(g) = cur.and_then(|i| self.store.group(i)) {
            if !self.open_groups.contains(&g.id) {
                return false;
            }
            cur = g.parent;
            guard += 1;
            if guard > 8 {
                break;
            }
        }
        true
    }

    /// A sub-group is listed only when every group above it is open.
    fn group_listed(&self, id: u64) -> bool {
        match self.store.group(id).and_then(|g| g.parent) {
            Some(p) => self.group_open(p),
            None => true,
        }
    }

    fn favorite_matches(&self) -> Vec<(u64, Vec<usize>)> {
        let mut groups: Vec<_> = self.group_order().into_iter().map(|(id, _)| (id, Vec::new())).collect();
        let searching = !self.query.trim().is_empty();
        let indices = if searching {
            self.favorite_search_matches()
        } else {
            (0..self.store.favorites.len()).collect()
        };
        let by_group: HashMap<_, _> = groups.iter().enumerate().map(|(i, (id, _))| (*id, i)).collect();
        for index in indices {
            let group = self.store.favorites[index].group;
            if let Some(&i) = by_group.get(&group).filter(|_| searching || self.group_open(group)) {
                groups[i].1.push(index);
            }
        }
        groups
    }

    fn favorite_search_matches(&self) -> Vec<usize> {
        let paths: HashMap<_, _> = self.store.groups.iter()
            .map(|g| (g.id, self.store.group_path(g.id)))
            .collect();
        let now = store::now();
        let mut scored: Vec<_> = self.store.favorites.iter().enumerate().filter_map(|(index, favorite)| {
            let path = paths.get(&favorite.group).map_or("", String::as_str);
            let score = store::score_favorite(path, &favorite.lower, &self.query)?;
            let frecency = store::frecency(favorite.uses, favorite.last_used, now);
            Some((index, score, frecency))
        }).collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.total_cmp(&a.2)));
        scored.into_iter().map(|(index, _, _)| index).collect()
    }

    // ---------- marking several rows (Shift / Cmd) ----------

    fn clear_marks(&mut self) {
        self.marks.clear();
        self.anchor = None;
    }

    /// Drops the marks when the tab or the search changed under them.
    fn check_marks(&mut self) {
        if !self.marks.is_empty() && (self.marks_ctx.0 != self.tab || self.marks_ctx.1 != self.query) {
            self.clear_marks();
        }
    }

    /// The group quick adds go to: the last one picked, else the first.
    fn default_group(&self) -> u64 {
        self.last_group
            .filter(|g| self.store.group(*g).is_some())
            .or_else(|| self.store.groups.first().map(|g| g.id))
            .unwrap_or(0)
    }

    fn add_fav_now(&mut self, content: Content, group: u64) {
        if self.store.is_favorite(&content) {
            return;
        }
        let id = self.store.new_id();
        let title = content.preview(28);
        self.store.favorites.push(Favorite { id, group, title, content, lower: String::new(), ..Default::default() });
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::TRANSPARENT.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.remember_wheel_root();
        self.handle_messages(ctx);
        theme::apply(ctx, self.shot_theme.unwrap_or(self.store.settings.theme));

        // hide when another app takes focus
        let focused = ctx.input(|i| i.viewport().focused).unwrap_or(false);
        if self.visible
            && self.was_focused
            && !focused
            && self.store.settings.hide_on_blur
            && self.pending_snippet.is_none()
            && !self.pinned
            && self.shot.is_none()
        {
            self.hide();
        }
        self.was_focused = focused;

        let wheel_search_focused = self.mode == Mode::Wheel && self.pending_snippet.is_none()
            && ctx.memory(|memory| memory.has_focus(wheel::search_id()));
        match self.mode {
            Mode::Panel => self.draw_panel(ctx),
            Mode::Wheel => {
                let previous_level = self.wheel.level;
                let shortcut = if self.pending_snippet.is_none() {
                    ctx.input_mut(|input| {
                        wheel::take_wheel_shortcut(&mut input.events, self.wheel.number_panel)
                    })
                } else {
                    None
                };
                let numbered_row = match shortcut {
                    Some(wheel::WheelShortcut::Row(index)) => Some(index),
                    Some(shortcut) => {
                        self.wheel_numbered_segment(ctx, shortcut);
                        None
                    }
                    None => {
                        self.wheel_keys(ctx);
                        None
                    }
                };
                self.remember_wheel_transition(previous_level);
                if self.mode == Mode::Wheel && self.visible {
                    let previous_level = self.wheel.level;
                    self.draw_wheel(ctx, numbered_row);
                    self.remember_wheel_transition(previous_level);
                }
            }
        }

        if wheel_search_focused && let Some(pending) = &mut self.pending_snippet {
            pending.return_focus = Some(wheel::search_id());
        }
        self.draw_snippet_prompt(ctx);

        // save at most once a second
        let now = ctx.input(|i| i.time);
        if self.dirty && self.shot.is_none() && now - self.last_save > 1.0 {
            self.store.save();
            self.dirty = false;
            self.last_save = now;
        }
        if self.dirty {
            ctx.request_repaint_after(std::time::Duration::from_millis(1100));
        }

        // screenshot mode: draw ~120 frames, save the picture, quit
        if let Some(path) = self.shot.clone() {
            self.frame += 1;
            ctx.request_repaint();
            if self.frame == 120 {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
            }
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = shot {
                let [w, h] = img.size;
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let saved = image::RgbaImage::from_raw(w as u32, h as u32, bytes)
                    .zip(image::ImageFormat::from_path(&path).ok())
                    .is_some_and(|(buf, format)| {
                        let mut output = std::io::Cursor::new(Vec::new());
                        buf.write_to(&mut output, format).is_ok()
                            && store::write_private_file(&path, output.get_ref()).is_ok()
                    });
                std::process::exit(i32::from(!saved));
            }
            if self.frame > 400 {
                std::process::exit(1);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.shot.is_none() {
            self.store.save();
        }
    }
}

// ---------- system hooks ----------

fn install_hotkey_handler(tx: Sender<Msg>, ctx: egui::Context) {
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state() == HotKeyState::Pressed {
            let _ = tx.send(Msg::Hotkey(e.id()));
            platform::show_app();
            ctx.request_repaint();
        }
    }));
}

fn build_tray(tx: Sender<Msg>, ctx: egui::Context) -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    let menu = Menu::new();
    let panel = MenuItem::with_id("panel", "Open history", true, None);
    let wheel = MenuItem::with_id("wheel", "Open favorites wheel", true, None);
    let favs = MenuItem::with_id("favorites", "Edit favorites", true, None);
    let settings = MenuItem::with_id("settings", "Settings…", true, None);
    let quit = MenuItem::with_id("quit", "Quit", true, None);
    let _ = menu.append_items(&[&panel, &wheel, &favs, &PredefinedMenuItem::separator(), &settings, &quit]);
    MenuEvent::set_event_handler(Some(Arc::new(move |e: MenuEvent| {
        let _ = tx.send(Msg::Menu(e.id().0.clone()));
        platform::show_app();
        ctx.request_repaint();
    })));
    // Drawn at twice the menu-bar size so it stays sharp on Retina screens.
    let icon = tray_icon::Icon::from_rgba(crate::art::logo_rgba(44, crate::art::Style::Template), 44, 44).ok()?;
    tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon_templated(icon)
        .with_tooltip(crate::DISPLAY_NAME)
        .build()
        .ok()
}

fn history_order(store: &Store, query: &str) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..store.history.len()).collect();
    }
    let mut scored: Vec<_> = store.history.iter().enumerate().filter_map(|(index, clip)| {
        Some((index, store::score(&clip.lower, query)?, clip.created))
    }).collect();
    scored.sort_by_key(|&(_, score, created)| std::cmp::Reverse((score, created)));
    scored.into_iter().map(|(index, _, _)| index).collect()
}

#[cfg(test)]
mod panel_search_tests {
    use super::*;

    #[test]
    fn history_search_ranks_phrase_before_recency() {
        let mut store = Store::first_run();
        for (created, text) in [(80, "find the needle"), (20, "needle old"), (40, "needle new")] {
            let content = Content::Text { text: text.into() };
            let id = store.new_id();
            store.history.push_back(store::Clip { id, content, created, ..Default::default() });
        }
        store.reindex();
        assert_eq!(history_order(&store, "  NEEDLE "), vec![2, 1, 0]);
    }
}

#[cfg(test)]
mod clipboard_request_tests {
    use super::*;

    #[test]
    fn only_favorite_text_with_valid_variables_is_templated() {
        let content = Content::Text { text: "{{host:local}}".into() };
        assert_eq!(snippet_text(Src::Fav(2), &content), Some("{{host:local}}"));
        assert_eq!(snippet_text(Src::Clip(2), &content), None);
        assert_eq!(snippet_text(Src::Fav(2), &Content::default()), None);
        let image = Content::Image { file: "{{host}}".into(), w: 1, h: 1 };
        assert_eq!(snippet_text(Src::Fav(2), &image), None);
    }
}
