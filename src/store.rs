//! Data model and persistence: clipboard history, favorite groups, favorites, settings.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Content {
    Text { text: String },
    Image { file: String, w: u32, h: u32 },
}

impl Default for Content {
    fn default() -> Self {
        Self::Text {
            text: String::new(),
        }
    }
}

impl Content {
    /// One-line preview text.
    pub fn preview(&self, max_chars: usize) -> String {
        match self {
            Content::Text { text } => {
                let mut out = String::new();
                let mut last_space = false;
                for ch in text.trim().chars() {
                    let c = if ch.is_whitespace() { ' ' } else { ch };
                    if c == ' ' && last_space {
                        continue;
                    }
                    last_space = c == ' ';
                    out.push(c);
                    if out.chars().count() >= max_chars {
                        out.push('…');
                        break;
                    }
                }
                out
            }
            Content::Image { w, h, .. } => format!("Image {w}×{h}"),
        }
    }

    pub fn search_text(&self) -> String {
        match self {
            Content::Text { text } => text.to_lowercase(),
            Content::Image { w, h, .. } => format!("image {w}x{h}"),
        }
    }

    pub fn image_file(&self) -> Option<&str> {
        match self {
            Content::Image { file, .. } => Some(file),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Clip {
    pub id: u64,
    pub content: Content,
    pub hash: u64,
    pub created: i64,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub uses: u32,
    #[serde(default)]
    pub last_used: i64,
    #[serde(skip)]
    pub lower: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
    /// Optional global shortcut that opens the wheel directly in this group.
    #[serde(default)]
    pub hotkey: String,
    /// Parent group: a group with a parent shows up as a sub-menu inside it.
    #[serde(default)]
    pub parent: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Favorite {
    pub id: u64,
    pub group: u64,
    pub title: String,
    pub content: Content,
    #[serde(default)]
    pub uses: u32,
    #[serde(default)]
    pub last_used: i64,
    #[serde(skip)]
    pub lower: String,
}

/// What happens when something already in the history is copied again.
/// Same three choices as the original (`pasteboard.duplicate-behaviour`, functions
/// 0x10016ca84, 0x1000b7664, 0x1002150dc).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum Duplicates {
    /// Always add it as a new entry.
    #[serde(rename = "allow")]
    Allow,
    /// Ignore it only when it equals the newest entry (the original's default).
    #[default]
    #[serde(rename = "dontAllowWhenOnTop")]
    NotWhenOnTop,
    /// Ignore it when it is anywhere in the history.
    #[serde(rename = "dontAllow")]
    Never,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(default)]
    pub theme: Theme,
    pub duplicates: Duplicates,
    /// An entry pasted from the app moves to the top (`features.moveCopiedSnapshotToTheTop`,
    /// function 0x1000ecb7c).
    pub move_used_to_top: bool,
    pub history_limit: usize,
    pub hotkey_panel: String,
    pub hotkey_wheel: String,
    /// Opens the searchable favorites list.
    pub hotkey_favorites: String,
    pub auto_paste: bool,
    pub skip_concealed: bool,
    pub hide_on_blur: bool,
    pub launch_at_login: bool,
    /// Set once the old wheel shortcut (Ctrl+Alt+V) was moved to Ctrl+Shift+F.
    pub wheel_key_v2: bool,
}

pub fn default_hotkey_panel() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd+Shift+V"
    } else {
        "Ctrl+Shift+V"
    }
}

pub fn default_hotkey_wheel() -> &'static str {
    if cfg!(target_os = "macos") {
        "Ctrl+Shift+F"
    } else {
        "Ctrl+Alt+F"
    }
}

pub fn default_hotkey_favorites() -> &'static str {
    if cfg!(target_os = "macos") {
        "Ctrl+Alt+F"
    } else {
        "Ctrl+Alt+Shift+F"
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            duplicates: Duplicates::default(),
            move_used_to_top: true,
            history_limit: 500,
            hotkey_panel: default_hotkey_panel().into(),
            hotkey_wheel: default_hotkey_wheel().into(),
            hotkey_favorites: default_hotkey_favorites().into(),
            auto_paste: true,
            skip_concealed: true,
            hide_on_blur: true,
            launch_at_login: true,
            wheel_key_v2: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    pub next_id: u64,
    pub history: VecDeque<Clip>,
    pub groups: Vec<Group>,
    pub favorites: Vec<Favorite>,
    pub settings: Settings,
    #[serde(default)]
    pub snippet_values: HashMap<String, String>,
    #[serde(skip)]
    pub load_notice: Option<&'static str>,
    #[serde(skip)]
    save_blocked: bool,
}

/// Same bounds as the original (function 0x1000f3f94): 100..=5000.
pub const HISTORY_MIN: usize = 100;
pub const HISTORY_MAX: usize = 5000;

/// Group colours: crisp accents for light surfaces (Tangerine orange, blue, green, rose, purple, amber, teal, slate).
pub const PALETTE: [[u8; 3]; 8] = [
    [255, 84, 16],
    [26, 118, 224],
    [34, 169, 96],
    [229, 72, 109],
    [124, 92, 252],
    [224, 148, 12],
    [20, 158, 158],
    [110, 118, 134],
];

/// Earlier palettes, so saved groups move to the current crisp palette.
const DRACULA_PALETTE: [[u8; 3]; 8] = [
    [189, 147, 249],
    [139, 233, 253],
    [80, 250, 123],
    [255, 121, 198],
    [255, 184, 108],
    [241, 250, 140],
    [255, 85, 85],
    [98, 114, 164],
];

const OLD_PALETTE: [[u8; 3]; 8] = [
    [255, 140, 66],
    [72, 149, 239],
    [76, 201, 140],
    [232, 92, 120],
    [167, 119, 227],
    [245, 196, 70],
    [64, 196, 210],
    [150, 160, 175],
];

const WARM_PALETTE: [[u8; 3]; 8] = [
    [214, 110, 58],
    [58, 120, 168],
    [104, 146, 74],
    [190, 78, 98],
    [128, 92, 156],
    [204, 158, 52],
    [46, 140, 140],
    [140, 128, 112],
];

pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn data_dir() -> PathBuf {
    init_data_dir(false).clone()
}

fn init_data_dir(read_only: bool) -> &'static PathBuf {
    static DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DATA_DIR.get_or_init(|| {
        if let Some(dir) = std::env::var_os("COPYWHEEL_DATA_DIR") {
            return PathBuf::from(dir);
        }
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        if read_only {
            let current = base.join(crate::DISPLAY_NAME);
            let legacy = base.join("ClipWheel");
            if !current.exists() && legacy.is_dir() {
                legacy
            } else {
                current
            }
        } else {
            copywheel_data_dir(&base)
        }
    })
}

fn copywheel_data_dir(base: &std::path::Path) -> PathBuf {
    let current = base.join(crate::DISPLAY_NAME);
    let legacy = base.join("ClipWheel");
    if !current.exists()
        && legacy.is_dir()
        && fs::rename(&legacy, &current).is_err()
        && !current.is_dir()
    {
        return legacy;
    }
    current
}

#[cfg(unix)]
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(ErrorKind::NotADirectory.into());
    }
    if metadata.permissions().mode() & 0o7777 != 0o700 {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(ErrorKind::NotADirectory.into());
    }
    Ok(())
}

#[cfg(unix)]
fn tighten_file(path: &Path, metadata: fs::Metadata) -> io::Result<()> {
    if !metadata.is_file() {
        return Err(ErrorKind::InvalidData.into());
    }
    if metadata.permissions().mode() & (0o7777 & !0o600) != 0 {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn tighten_file(_path: &Path, metadata: fs::Metadata) -> io::Result<()> {
    if !metadata.is_file() {
        return Err(ErrorKind::InvalidData.into());
    }
    Ok(())
}

fn prepare_data_dir(dir: &Path) -> io::Result<()> {
    create_private_dir(dir)?;
    let images = dir.join("images");
    create_private_dir(&images)?;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "data.json"
            || name == "data.json.tmp"
            || name.starts_with("data.json.tmp-")
            || name.starts_with("data.json.corrupt-")
        {
            tighten_file(&entry.path(), fs::symlink_metadata(entry.path())?)?;
        }
    }
    for entry in fs::read_dir(images)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            tighten_file(&entry.path(), fs::symlink_metadata(entry.path())?)?;
        }
    }
    Ok(())
}

pub fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(false);
    #[cfg(unix)]
    opts.mode(0o600);
    let mut file = opts.open(path)?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.set_len(0)?;
    file.write_all(bytes)
}

fn create_private_file(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    opts.mode(0o600);
    opts.open(path)
}

pub fn image_path(file: &str) -> io::Result<PathBuf> {
    let path = Path::new(file);
    if path.file_name().and_then(|name| name.to_str()) != Some(file) {
        return Err(ErrorKind::InvalidInput.into());
    }
    let dir = data_dir();
    let images = dir.join("images");
    for path in [&dir, &images] {
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_dir() => return Err(ErrorKind::NotADirectory.into()),
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let path = images.join(file);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.is_file() => Err(ErrorKind::InvalidData.into()),
        Ok(_) => Ok(path),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(path),
        Err(error) => Err(error),
    }
}

pub fn save_image(file: &str, bytes: &[u8]) -> io::Result<()> {
    let path = image_path(file)?;
    create_private_dir(path.parent().ok_or(ErrorKind::InvalidInput)?)?;
    match create_private_file(&path) {
        Ok(mut output) => {
            #[cfg(unix)]
            let result = output
                .set_permissions(fs::Permissions::from_mode(0o600))
                .and_then(|()| output.write_all(bytes));
            #[cfg(not(unix))]
            let result = output.write_all(bytes);
            if result.is_err() {
                let _ = fs::remove_file(&path);
            }
            result
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            tighten_file(&path, fs::symlink_metadata(&path)?)
        }
        Err(error) => Err(error),
    }
}

fn preserve_corrupt_data(path: &Path, dir: &Path) -> io::Result<()> {
    let timestamp = now();
    for attempt in 0.. {
        let name = if attempt == 0 {
            format!("data.json.corrupt-{timestamp}")
        } else {
            format!("data.json.corrupt-{timestamp}-{attempt}")
        };
        let backup = dir.join(name);
        // Reserve the name so a second recovery cannot overwrite an earlier backup.
        match create_private_file(&backup) {
            Ok(_) => {
                let result = fs::rename(path, &backup);
                if result.is_err() {
                    let _ = fs::remove_file(&backup);
                }
                return result;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}

impl Store {
    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn first_run() -> Self {
        let mut s = Store::default();
        type SampleGroup = (
            &'static str,
            [u8; 3],
            &'static [(&'static str, &'static str)],
        );
        let samples: [SampleGroup; 3] = [
            (
                "General",
                PALETTE[0],
                &[
                    ("Thank you", "Thank you, and have a good day."),
                    ("Address", "1 Example Street, Istanbul"),
                ],
            ),
            (
                "Work",
                PALETTE[1],
                &[
                    ("Email signature", "Best regards,\nFull Name\nJob Title"),
                    ("Meeting", "Can we have a quick meeting tomorrow at 10:00?"),
                ],
            ),
            (
                "Code",
                PALETTE[2],
                &[("TODO", "// TODO: "), ("Log", "println!(\"{:?}\", value);")],
            ),
        ];
        for (name, color, items) in samples {
            let gid = s.new_id();
            s.groups.push(Group {
                id: gid,
                name: name.into(),
                color,
                hotkey: String::new(),
                parent: None,
            });
            for (title, text) in items {
                let id = s.new_id();
                s.favorites.push(Favorite {
                    id,
                    group: gid,
                    title: (*title).into(),
                    content: Content::Text {
                        text: (*text).into(),
                    },
                    ..Favorite::default()
                });
            }
        }
        s.reindex();
        s
    }

    pub fn load(read_only: bool) -> Self {
        Self::load_from(init_data_dir(read_only), read_only)
    }

    fn load_from(dir: &Path, read_only: bool) -> Self {
        if !read_only && prepare_data_dir(dir).is_err() {
            return Self::unavailable(
                "CopyWheel could not secure its data directory. Saving is disabled.",
            );
        }
        let path = dir.join("data.json");
        let mut s = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Self>(&bytes) {
                Ok(store) => store,
                Err(_) if read_only => {
                    return Self::unavailable(
                        "Clipboard data is corrupt. The original file was kept.",
                    );
                }
                Err(_) if preserve_corrupt_data(&path, dir).is_ok() => Self {
                    load_notice: Some(
                        "Clipboard data was corrupt. The original was saved and CopyWheel started empty.",
                    ),
                    ..Self::default()
                },
                Err(_) => {
                    return Self::unavailable(
                        "Clipboard data is corrupt and could not be moved. Saving is disabled.",
                    );
                }
            },
            Err(error) if error.kind() == ErrorKind::NotFound => Self::first_run(),
            Err(_) => {
                return Self::unavailable("Clipboard data could not be read. Saving is disabled.");
            }
        };
        if s.groups.is_empty() {
            let id = s.new_id();
            s.groups.push(Group {
                id,
                name: "General".into(),
                color: PALETTE[0],
                hotkey: String::new(),
                parent: None,
            });
        }
        s.settings.history_limit = s.settings.history_limit.clamp(HISTORY_MIN, HISTORY_MAX);
        // The person asked for Ctrl+Shift+F to open the favorites wheel.
        if !s.settings.wheel_key_v2 {
            if s.settings.hotkey_wheel == "Ctrl+Alt+V" {
                s.settings.hotkey_wheel = default_hotkey_wheel().into();
            }
            s.settings.wheel_key_v2 = true;
        }
        // group colours from earlier palettes move to the crisp light palette
        for g in s.groups.iter_mut() {
            if let Some(i) = OLD_PALETTE
                .iter()
                .chain(WARM_PALETTE.iter())
                .chain(DRACULA_PALETTE.iter())
                .position(|c| *c == g.color)
            {
                g.color = PALETTE[i % PALETTE.len()];
            }
        }
        s.reindex();
        s
    }

    fn unavailable(notice: &'static str) -> Self {
        Self {
            load_notice: Some(notice),
            save_blocked: true,
            ..Self::default()
        }
    }

    pub fn save(&self) {
        self.save_to(&data_dir());
    }

    fn save_to(&self, dir: &Path) {
        if self.save_blocked || create_private_dir(dir).is_err() {
            return;
        }
        let Ok(bytes) = serde_json::to_vec(self) else {
            return;
        };
        static SAVE_ID: AtomicU64 = AtomicU64::new(0);
        let id = SAVE_ID.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!("data.json.tmp-{}-{id}", std::process::id()));
        let Ok(mut file) = create_private_file(&tmp) else {
            return;
        };
        #[cfg(unix)]
        let result = file
            .set_permissions(fs::Permissions::from_mode(0o600))
            .and_then(|()| file.write_all(&bytes))
            .and_then(|()| fs::rename(&tmp, dir.join("data.json")));
        #[cfg(not(unix))]
        let result = file
            .write_all(&bytes)
            .and_then(|()| fs::rename(&tmp, dir.join("data.json")));
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
    }

    pub fn reindex(&mut self) {
        for c in self.history.iter_mut() {
            c.lower = clip_search_text(&c.content, c.source.as_deref());
        }
        for f in self.favorites.iter_mut() {
            f.lower = format!("{} {}", f.title.to_lowercase(), f.content.search_text());
        }
    }

    /// Adds a new copy at the top, following the duplicate rule (function 0x1002150dc):
    /// NotWhenOnTop ignores it when it equals the newest entry, Never ignores it when it
    /// is anywhere in the history, Allow always adds it. Returns whether it was added.
    pub fn add_clip(&mut self, content: Content, hash: u64, source: Option<String>) -> bool {
        let same = |c: &Clip| c.hash == hash && c.content == content;
        let skip = match self.settings.duplicates {
            Duplicates::Allow => false,
            Duplicates::NotWhenOnTop => self.history.front().is_some_and(same),
            Duplicates::Never => self.history.iter().any(same),
        };
        if skip {
            return false;
        }
        let id = self.new_id();
        let lower = clip_search_text(&content, source.as_deref());
        self.history.push_front(Clip {
            id,
            content,
            hash,
            created: now(),
            source,
            lower,
            ..Clip::default()
        });
        self.trim();
        true
    }

    pub fn touch_clip(&mut self, id: u64) {
        if let Some(clip) = self.history.iter_mut().find(|clip| clip.id == id) {
            clip.uses = clip.uses.saturating_add(1);
            clip.last_used = now();
        }
    }

    pub fn touch_favorite(&mut self, id: u64) {
        if let Some(favorite) = self.favorites.iter_mut().find(|favorite| favorite.id == id) {
            favorite.uses = favorite.uses.saturating_add(1);
            favorite.last_used = now();
        }
    }

    /// Moves the newest entry with this content to the top (used after pasting it from
    /// the app). Returns false when there is none.
    pub fn bring_to_top(&mut self, content: &Content) -> bool {
        let Some(pos) = self.history.iter().position(|c| &c.content == content) else {
            return false;
        };
        if let Some(mut c) = self.history.remove(pos) {
            c.created = now();
            self.history.push_front(c);
        }
        true
    }

    pub fn trim(&mut self) {
        let limit = self.settings.history_limit;
        while self.history.len() > limit {
            if let Some(old) = self.history.pop_back() {
                self.forget_image(&old.content);
            }
        }
    }

    pub fn remove_clip(&mut self, id: u64) {
        if let Some(pos) = self.history.iter().position(|c| c.id == id)
            && let Some(old) = self.history.remove(pos)
        {
            self.forget_image(&old.content);
        }
    }

    pub fn clear_history(&mut self) {
        let old: Vec<Clip> = self.history.drain(..).collect();
        for c in old {
            self.forget_image(&c.content);
        }
    }

    /// Deletes an image file when nothing points to it any more.
    fn forget_image(&self, content: &Content) {
        if let Some(file) = content.image_file() {
            let used = self
                .history
                .iter()
                .any(|c| c.content.image_file() == Some(file))
                || self
                    .favorites
                    .iter()
                    .any(|f| f.content.image_file() == Some(file));
            if !used && let Ok(path) = image_path(file) {
                let _ = fs::remove_file(path);
            }
        }
    }

    pub fn remove_favorite(&mut self, id: u64) {
        if let Some(pos) = self.favorites.iter().position(|f| f.id == id) {
            let f = self.favorites.remove(pos);
            self.forget_image(&f.content);
        }
    }

    pub fn is_favorite(&self, content: &Content) -> bool {
        self.favorites.iter().any(|f| &f.content == content)
    }

    pub fn group(&self, id: u64) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }

    pub fn favorites_in(&self, group: u64) -> Vec<&Favorite> {
        self.favorites.iter().filter(|f| f.group == group).collect()
    }

    /// Moves a favorite into `group`, placed before favorite `before` (or last).
    pub fn move_favorite(&mut self, id: u64, group: u64, before: Option<u64>) {
        let Some(pos) = self.favorites.iter().position(|f| f.id == id) else {
            return;
        };
        let mut f = self.favorites.remove(pos);
        f.group = group;
        let at = before
            .and_then(|b| self.favorites.iter().position(|x| x.id == b))
            .unwrap_or(self.favorites.len());
        self.favorites.insert(at, f);
    }

    /// Favorites in a group and in all of its sub-groups.
    pub fn favorite_count(&self, id: u64) -> usize {
        self.favorites_in(id).len()
            + self
                .children(Some(id))
                .into_iter()
                .map(|c| self.favorite_count(c))
                .sum::<usize>()
    }

    /// Ids of the groups directly under `parent` (None = top level), in list order.
    pub fn children(&self, parent: Option<u64>) -> Vec<u64> {
        self.groups
            .iter()
            .filter(|g| g.parent == parent)
            .map(|g| g.id)
            .collect()
    }

    /// "Parent › Child" name, used where groups are listed flat.
    pub fn group_path(&self, id: u64) -> String {
        let mut names = Vec::new();
        let mut visited = HashSet::new();
        let mut cur = Some(id);
        while let Some(g) = cur
            .filter(|&id| visited.insert(id))
            .and_then(|id| self.group(id))
        {
            names.push(g.name.as_str());
            cur = g.parent;
        }
        names.reverse();
        names.join(" › ")
    }

    /// Removes a group, its sub-groups and every favorite in them.
    pub fn remove_group(&mut self, id: u64) {
        for child in self.children(Some(id)) {
            self.remove_group(child);
        }
        let favs: Vec<u64> = self
            .favorites
            .iter()
            .filter(|f| f.group == id)
            .map(|f| f.id)
            .collect();
        for f in favs {
            self.remove_favorite(f);
        }
        self.groups.retain(|g| g.id != id);
    }

    pub fn demo_history(&mut self) {
        let samples = [
            ("https://example.com/report/2026-q4", "Safari"),
            (
                "Meeting notes: budget approved, delivery date December 12.",
                "Notes",
            ),
            ("cargo build --release", "Terminal"),
            ("Hello, I've attached the file. Have a good day.", "Mail"),
            ("#FF8C42", "Figma"),
            ("SELECT id, name FROM users WHERE active = 1;", "TablePlus"),
            ("+90 555 000 00 00", "Contacts"),
            ("Copied text appears here.", "TextEdit"),
            ("fn main() { println!(\"Hello\"); }", "Xcode"),
            ("Order number: 2026-118734", "Chrome"),
            ("Istanbul, Turkey", "Maps"),
        ];
        let t = now();
        for (i, (text, app)) in samples.iter().enumerate().rev() {
            let content = Content::Text {
                text: (*text).into(),
            };
            let id = self.new_id();
            self.history.push_front(Clip {
                id,
                hash: fnv1a(text.as_bytes()),
                lower: clip_search_text(&content, Some(app)),
                content,
                created: t - (i as i64) * 340 - 20,
                source: Some((*app).into()),
                ..Clip::default()
            });
        }
    }
}

/// A settings file to move CopyWheel to another Mac or keep as a backup: settings,
/// shortcuts, favorite groups and favorites (pictures inside). The history stays out.
#[derive(Serialize, Deserialize)]
pub struct Backup {
    pub app: String,
    pub version: u32,
    pub settings: Settings,
    pub groups: Vec<Group>,
    pub favorites: Vec<Favorite>,
    /// Picture favorites: file name -> PNG bytes in base64.
    #[serde(default)]
    pub images: std::collections::BTreeMap<String, String>,
}

/// What an import changed, for the message under the buttons.
#[derive(Debug, PartialEq)]
pub struct ImportResult {
    pub groups_added: usize,
    pub favorites_added: usize,
}

impl Store {
    pub fn backup(&self) -> Backup {
        use base64::Engine;
        let mut images = std::collections::BTreeMap::new();
        for f in &self.favorites {
            if let Some(file) = f.content.image_file()
                && let Ok(bytes) = image_path(file).and_then(fs::read)
            {
                images.insert(
                    file.to_string(),
                    base64::engine::general_purpose::STANDARD.encode(bytes),
                );
            }
        }
        Backup {
            app: crate::DISPLAY_NAME.into(),
            version: 1,
            settings: self.settings.clone(),
            groups: self.groups.clone(),
            favorites: self.favorites.clone(),
            images,
        }
    }

    pub fn export_config(&self, path: &std::path::Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&self.backup()).map_err(|e| e.to_string())?;
        write_private_file(path, &bytes).map_err(|e| e.to_string())
    }

    pub fn import_config(&mut self, path: &std::path::Path) -> Result<ImportResult, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let invalid = || format!("Invalid {} settings file.", crate::DISPLAY_NAME);
        let b: Backup = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if !matches!(b.app.as_str(), crate::DISPLAY_NAME | "ClipWheel") {
            return Err(invalid());
        }
        Ok(self.merge_backup(b))
    }

    /// Settings come from the file; groups and favorites are added to what is here
    /// (a group with the same name is filled, nothing is deleted).
    pub fn merge_backup(&mut self, b: Backup) -> ImportResult {
        use base64::Engine;
        let mut out = ImportResult {
            groups_added: 0,
            favorites_added: 0,
        };
        self.settings = Settings {
            wheel_key_v2: true,
            ..b.settings
        };
        self.settings.history_limit = self.settings.history_limit.clamp(HISTORY_MIN, HISTORY_MAX);

        let mut ids = HashMap::new();
        for g in &b.groups {
            let key = g.name.trim().to_lowercase();
            let id = match self
                .groups
                .iter_mut()
                .find(|x| x.name.trim().to_lowercase() == key)
            {
                Some(x) => {
                    if x.hotkey.is_empty() {
                        x.hotkey = g.hotkey.clone();
                    }
                    x.id
                }
                None => {
                    let id = self.new_id();
                    self.groups.push(Group {
                        id,
                        name: g.name.clone(),
                        color: g.color,
                        hotkey: g.hotkey.clone(),
                        parent: None,
                    });
                    out.groups_added += 1;
                    id
                }
            };
            ids.insert(g.id, id);
        }
        for f in b.favorites {
            let Some(&group) = ids.get(&f.group) else {
                continue;
            };
            if let Some(file) = f.content.image_file() {
                let Some(data) = b.images.get(file) else {
                    continue;
                };
                let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) else {
                    continue;
                };
                if save_image(file, &bytes).is_err() {
                    continue;
                }
            }
            if self
                .favorites
                .iter()
                .any(|x| x.group == group && x.content == f.content)
            {
                continue;
            }
            let id = self.new_id();
            self.favorites.push(Favorite { id, group, ..f });
            out.favorites_added += 1;
        }
        self.reindex();
        out
    }
}

/// What a history entry is searched in: its text and the app it came from, like the
/// original's `title`, `contentExcerpt` and `sourceAppLocalizedName` (function 0x10009235c).
pub fn clip_search_text(content: &Content, source: Option<&str>) -> String {
    match source {
        Some(s) => format!("{}\n{}", content.search_text(), s.to_lowercase()),
        None => content.search_text(),
    }
}

/// Scores a query against lowercased search text. Higher scores rank first.
pub fn score(hay_lower: &str, query: &str) -> Option<i32> {
    let query = query.trim().to_lowercase();
    let query = query.as_str();
    if query.is_empty() {
        return Some(0);
    }
    if hay_lower.starts_with(query) {
        return Some(600);
    }
    if hay_lower.contains(query) {
        let mut word_start = true;
        for (index, ch) in hay_lower.char_indices() {
            if word_start && hay_lower[index..].starts_with(query) {
                return Some(500);
            }
            word_start = !ch.is_alphanumeric();
        }
        return Some(400);
    }

    let plain_query = alphanumeric(query);
    if !plain_query.is_empty() && alphanumeric(hay_lower).contains(&plain_query) {
        return Some(300);
    }
    if query
        .split_whitespace()
        .all(|term| hay_lower.contains(term))
    {
        return Some(200);
    }

    let mut remaining = query.chars();
    let mut next = remaining.next();
    let mut bonus: i32 = 0;
    let mut word_start = true;
    let mut previous_matched = false;
    for ch in hay_lower.chars() {
        let matched = next == Some(ch);
        if matched {
            let points =
                2 * i32::from(previous_matched) + i32::from(word_start && ch.is_alphanumeric());
            bonus = bonus.saturating_add(points);
            next = remaining.next();
            if next.is_none() {
                // Keep even the strongest subsequence below the AND tier.
                return Some(100 + bonus.min(99));
            }
        }
        word_start = !ch.is_alphanumeric();
        previous_matched = matched;
    }
    None
}

/// Scores a favorite using its group path and indexed title and content.
pub fn score_favorite(group_path: &str, favorite_lower: &str, query: &str) -> Option<i32> {
    if query.trim().is_empty() {
        return Some(0);
    }
    if group_path.is_empty() {
        return score(favorite_lower, query);
    }
    let searchable = format!("{} {favorite_lower}", group_path.to_lowercase());
    score(&searchable, query)
}

fn alphanumeric(text: &str) -> String {
    text.chars().filter(|ch| ch.is_alphanumeric()).collect()
}

/// Returns the usage count weighted by a seven-day half-life.
pub fn frecency(uses: u32, last_used: i64, now: i64) -> f64 {
    let age = now.saturating_sub(last_used).max(0) as f64;
    f64::from(uses) * 0.5_f64.powf(age / (7.0 * 86_400.0))
}

pub fn ago(created: i64) -> String {
    let age = now().saturating_sub(created).max(0);
    let days = age / 86_400;
    if age < 60 {
        "now".into()
    } else if age < 3_600 {
        format!("{}m", age / 60)
    } else if days < 1 {
        format!("{}h", age / 3_600)
    } else if days < 7 {
        format!("{days}d")
    } else if days < 30 {
        format!("{}w", days / 7)
    } else if days < 365 {
        format!("{}mo", days / 30)
    } else {
        format!("{}y", days / 365)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(lower: &str, terms: &[String]) -> bool {
        score(lower, &terms.join(" ")).is_some()
    }

    fn matches_favorite(group_path: &str, favorite_lower: &str, query: &[String]) -> bool {
        score_favorite(group_path, favorite_lower, &query.join(" ")).is_some()
    }

    /// The query as one lower-case phrase (outer spaces removed), or nothing when empty.
    fn terms(query: &str) -> Vec<String> {
        let q = query.trim();
        if q.is_empty() {
            Vec::new()
        } else {
            vec![q.to_lowercase()]
        }
    }

    #[test]
    fn group_summary_counts_nested_favorites_and_resolves_the_full_path() {
        let mut store = Store::default();
        for id in 1..=11 {
            store.groups.push(Group {
                id,
                name: id.to_string(),
                color: PALETTE[0],
                hotkey: String::new(),
                parent: (id > 1).then_some(id - 1),
            });
        }
        for group in [1, 2, 11] {
            store.favorites.push(Favorite {
                group,
                ..Favorite::default()
            });
        }
        assert_eq!(store.favorite_count(2), 2);
        assert_eq!(
            store.group_path(11),
            "1 › 2 › 3 › 4 › 5 › 6 › 7 › 8 › 9 › 10 › 11"
        );
    }

    #[test]
    fn corrupt_data_is_preserved_and_starts_empty() {
        let dir = std::env::temp_dir().join(format!("copywheel_corrupt_{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let data = dir.join("data.json");
        let original = b"{invalid JSON";
        fs::write(&data, original).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&data, fs::Permissions::from_mode(0o644)).unwrap();

        let store = Store::load_from(&dir, false);
        assert!(!data.exists());
        assert!(store.history.is_empty());
        assert!(store.favorites.is_empty());
        assert!(store.load_notice.is_some());

        let backup = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("data.json.corrupt-")
            })
            .unwrap();
        assert_eq!(fs::read(&backup).unwrap(), original);
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
            0o600
        );

        store.save_to(&dir);
        assert_eq!(fs::read(&backup).unwrap(), original);
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&data).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(serde_json::from_slice::<Store>(&fs::read(&data).unwrap()).is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn legacy_data_moves_to_copywheel_with_its_images() {
        let base = std::env::temp_dir().join(format!("copywheel_migration_{}", std::process::id()));
        let legacy = base.join("ClipWheel");
        std::fs::create_dir_all(legacy.join("images")).unwrap();
        std::fs::write(legacy.join("data.json"), b"test data").unwrap();
        std::fs::write(legacy.join("images/test.png"), b"test image").unwrap();

        let current = copywheel_data_dir(&base);
        assert_eq!(current, base.join("CopyWheel"));
        assert_eq!(
            std::fs::read(current.join("data.json")).unwrap(),
            b"test data"
        );
        assert_eq!(
            std::fs::read(current.join("images/test.png")).unwrap(),
            b"test image"
        );
        assert!(!legacy.exists());

        #[cfg(unix)]
        let images = current.join("images");
        #[cfg(unix)]
        let image = images.join("test.png");
        let temp = current.join("data.json.tmp");
        fs::write(&temp, b"old temp data").unwrap();
        #[cfg(unix)]
        for dir in [&current, &images] {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(unix)]
        for file in [&image, &temp] {
            fs::set_permissions(file, fs::Permissions::from_mode(0o644)).unwrap();
        }
        prepare_data_dir(&current).unwrap();
        #[cfg(unix)]
        for dir in [&current, &images] {
            assert_eq!(
                fs::metadata(dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        #[cfg(unix)]
        for file in [&image, &temp] {
            assert_eq!(
                fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        std::fs::create_dir(&legacy).unwrap();
        std::fs::write(legacy.join("data.json"), b"old data").unwrap();
        assert_eq!(copywheel_data_dir(&base), current);
        assert_eq!(
            std::fs::read(legacy.join("data.json")).unwrap(),
            b"old data"
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    fn add(s: &mut Store, t: &str) -> bool {
        s.add_clip(Content::Text { text: t.into() }, fnv1a(t.as_bytes()), None)
    }

    #[test]
    fn history_keeps_limit() {
        let mut s = Store::default();
        s.settings.history_limit = HISTORY_MIN;
        for i in 0..160 {
            add(&mut s, &format!("copy {i}"));
        }
        assert_eq!(s.history.len(), HISTORY_MIN);
    }

    #[test]
    fn duplicate_rules_follow_the_original() {
        // default: ignored only when it equals the newest entry
        let mut s = Store::default();
        assert_eq!(s.settings.duplicates, Duplicates::NotWhenOnTop);
        add(&mut s, "a");
        add(&mut s, "b");
        assert!(!add(&mut s, "b"));
        assert!(add(&mut s, "a"));
        assert_eq!(s.history.len(), 3);

        s.settings.duplicates = Duplicates::Never;
        assert!(!add(&mut s, "b"));
        assert_eq!(s.history.len(), 3);

        s.settings.duplicates = Duplicates::Allow;
        assert!(add(&mut s, "a"));
        assert_eq!(s.history.len(), 4);
    }

    #[test]
    fn import_merges_groups_and_keeps_existing() {
        let mut a = Store::first_run();
        a.settings.history_limit = 800;
        a.groups[0].hotkey = "Ctrl+Alt+1".into();
        let gid = a.new_id();
        a.groups.push(Group {
            id: gid,
            name: "Server".into(),
            color: PALETTE[3],
            hotkey: String::new(),
            parent: None,
        });
        let fid = a.new_id();
        a.favorites.push(Favorite {
            id: fid,
            group: gid,
            title: "SSH".into(),
            content: Content::Text {
                text: "ssh x".into(),
            },
            ..Favorite::default()
        });
        let text = serde_json::to_string(&a.backup()).unwrap();

        let mut b = Store::first_run();
        let before = b.favorites.len();
        let r = b.merge_backup(serde_json::from_str(&text).unwrap());
        assert_eq!(
            r,
            ImportResult {
                groups_added: 1,
                favorites_added: 1
            }
        );
        assert_eq!(b.favorites.len(), before + 1);
        assert_eq!(b.settings.history_limit, 800);
        assert_eq!(b.groups[0].hotkey, "Ctrl+Alt+1");
        // importing again adds nothing
        let r = b.merge_backup(serde_json::from_str(&text).unwrap());
        assert_eq!(
            r,
            ImportResult {
                groups_added: 0,
                favorites_added: 0
            }
        );

        let path = std::env::temp_dir().join(format!("copywheel_test_{}.json", std::process::id()));
        fs::write(&path, b"old backup").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        a.export_config(&path).unwrap();
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let mut c = Store::first_run();
        let r = c.import_config(&path).unwrap();
        assert_eq!(
            r,
            ImportResult {
                groups_added: 1,
                favorites_added: 1
            }
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn used_entry_moves_to_top() {
        let mut s = Store::default();
        add(&mut s, "a");
        add(&mut s, "b");
        assert!(s.bring_to_top(&Content::Text { text: "a".into() }));
        assert_eq!(s.history[0].content, Content::Text { text: "a".into() });
        assert_eq!(s.history.len(), 2);
    }

    #[test]
    fn search_matches_whole_phrase_and_source_app() {
        let t = terms("  Budget APPROVAL ");
        assert!(matches("meeting: budget approval", &t));
        assert!(matches("approval pending on the budget", &t));
        assert!(!matches("budget revision", &t));
        let lower = clip_search_text(
            &Content::Text {
                text: "hello".into(),
            },
            Some("Safari"),
        );
        assert!(matches(&lower, &terms("safari")));
        assert!(matches("anything", &terms("")));
    }

    #[test]
    fn move_favorite_changes_group_and_order() {
        let mut s = Store::first_run();
        let g2 = s.new_id();
        s.groups.push(Group {
            id: g2,
            name: "B".into(),
            color: PALETTE[1],
            hotkey: String::new(),
            parent: None,
        });
        let ids: Vec<u64> = s.favorites.iter().map(|f| f.id).collect();
        let (a, b) = (ids[0], ids[1]);
        s.move_favorite(a, g2, None);
        assert_eq!(s.favorites.last().unwrap().id, a);
        assert_eq!(s.favorites.last().unwrap().group, g2);
        s.move_favorite(a, g2, Some(b));
        let pos = |id| s.favorites.iter().position(|f| f.id == id).unwrap();
        assert!(pos(a) + 1 == pos(b));
    }

    #[test]
    fn search_matches_punctuation_variants_and_unordered_terms() {
        let hay = "paket kur (rhel) dnf install -y ipa-client";
        assert!(matches(hay, &terms("paket kur rhel")));
        assert!(matches(hay, &terms("ipa client")));
        assert!(matches(hay, &terms("rhel paket")));
        assert!(matches(hay, &terms("kur paket")));
        assert!(terms("   ").is_empty());
    }

    #[test]
    fn search_matches_subsequences() {
        assert!(matches("paket kur (rhel)", &terms("pkr")));
        assert!(matches_favorite("Linux", "paket kur (rhel)", &terms("pkr")));
    }

    #[test]
    fn search_scores_all_six_tiers_in_order() {
        let query = "  Paket KUR ";
        let scores = [
            "paket kur",
            "install paket kur",
            "unpaket kur",
            "paket (kur)",
            "kur paket",
            "package toolkit: kernel update report",
        ]
        .map(|hay| score(hay, query).expect(hay));
        assert!(
            scores.windows(2).all(|pair| pair[0] > pair[1]),
            "{scores:?}"
        );
        assert_eq!(score("anything", "   "), Some(0));
        assert_eq!(score("paket kur (rhel)", "xyz"), None);
    }

    #[test]
    fn search_matches_punctuation_in_mixed_case_content() {
        let hay = Content::Text {
            text: "Paket kur (RHEL)".into(),
        }
        .search_text();
        assert!(score(&hay, "paket kur rhel").is_some());
    }

    #[test]
    fn subsequence_rewards_runs_and_word_starts() {
        assert!(score("pk-er", "pkr").unwrap() > score("p-k-er", "pkr").unwrap());
        assert!(
            score("paket kur (rhel)", "pkr").unwrap() > score("apaketakur arhel", "pkr").unwrap()
        );
    }

    #[test]
    fn favorite_search_combines_group_title_and_command() {
        let group = "DNS › BIND";
        let favorite = "cache statistics rndc stats";
        assert!(score_favorite(group, favorite, "DNS cache").is_some());
        assert!(matches_favorite(group, favorite, &terms("dns cache")));
        assert!(matches_favorite(
            group,
            favorite,
            &terms("stats bind cache")
        ));
        assert!(!matches_favorite(group, favorite, &terms("dns restart")));
        assert!(!matches_favorite(group, favorite, &terms("|")));
        assert!(matches_favorite("Linux", "grep error | sort", &terms("|")));
    }

    #[test]
    fn frecency_halves_each_week() {
        let now = 30 * 86_400;
        assert_eq!(frecency(8, now, now), 8.0);
        assert_eq!(frecency(8, now - 7 * 86_400, now), 4.0);
        assert_eq!(frecency(8, now - 14 * 86_400, now), 2.0);
    }

    #[test]
    fn old_data_defaults_usage_and_snippet_values() {
        let json = r#"{
            "history": [{"id": 1, "content": {"type": "text", "text": "copy"}, "hash": 42, "created": 2}],
            "favorites": [{"id": 3, "group": 4, "title": "Example", "content": {"type": "text", "text": "snippet"}}]
        }"#;
        let mut store: Store = serde_json::from_str(json).unwrap();
        assert_eq!((store.history[0].uses, store.history[0].last_used), (0, 0));
        assert_eq!(
            (store.favorites[0].uses, store.favorites[0].last_used),
            (0, 0)
        );
        assert!(store.snippet_values.is_empty());

        let before = now();
        store.touch_clip(1);
        store.touch_clip(1);
        store.touch_favorite(3);
        store.snippet_values.insert("name".into(), "value".into());

        let saved: Store = serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        assert_eq!(saved.history[0].uses, 2);
        assert_eq!(saved.favorites[0].uses, 1);
        assert!((before..=now()).contains(&saved.history[0].last_used));
        assert!((before..=now()).contains(&saved.favorites[0].last_used));
        assert_eq!(saved.snippet_values["name"], "value");
    }

    #[test]
    fn ago_uses_compact_english_units() {
        let now = now();
        for (age, label) in [
            (0, "now"),
            (5 * 60, "5m"),
            (42 * 60, "42m"),
            (3 * 3_600, "3h"),
            (2 * 86_400, "2d"),
            (21 * 86_400, "3w"),
            (150 * 86_400, "5mo"),
            (365 * 86_400, "1y"),
        ] {
            assert_eq!(ago(now - age), label);
        }
    }

    #[test]
    fn legacy_data_without_theme_defaults_to_system() {
        let saved: Store = serde_json::from_str(r#"{"settings":{"history_limit":750}}"#).unwrap();
        assert_eq!(saved.settings.history_limit, 750);
        assert_eq!(saved.settings.theme, Theme::System);
        assert_eq!(
            serde_json::to_value(&saved.settings).unwrap()["theme"],
            "system"
        );
    }

    #[test]
    fn default_shortcuts_parse() {
        use global_hotkey::hotkey::HotKey;
        let s = Settings::default();
        assert!(s.hotkey_panel.parse::<HotKey>().is_ok());
        assert!(s.hotkey_wheel.parse::<HotKey>().is_ok());
        assert!(s.hotkey_favorites.parse::<HotKey>().is_ok());
        assert_ne!(s.hotkey_panel, s.hotkey_wheel);
        assert_ne!(s.hotkey_wheel, s.hotkey_favorites);
        assert_ne!(s.hotkey_panel, s.hotkey_favorites);
        assert!("Ctrl+Alt+Space".parse::<HotKey>().is_ok());
    }

    #[test]
    fn settings_deserialize_with_and_without_launch_at_login() {
        let legacy: Settings = serde_json::from_str(r#"{"history_limit": 250}"#).unwrap();
        assert_eq!(legacy.history_limit, 250);
        assert!(legacy.launch_at_login);

        let disabled: Settings = serde_json::from_str(r#"{"launch_at_login": false}"#).unwrap();
        assert!(!disabled.launch_at_login);

        let enabled: Settings = serde_json::from_str(r#"{"launch_at_login": true}"#).unwrap();
        assert!(enabled.launch_at_login);
    }
}
