//! Background clipboard watcher: notices new copies and sends them to the app.

use crate::platform;
use crate::store::{self, Content, fnv1a};
use image::ImageEncoder;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub enum Msg {
    Clip(Content, u64, Option<String>),
    Hotkey(u32),
    Menu(String),
    #[cfg(windows)]
    TrayToggle,
}

pub fn spawn(tx: Sender<Msg>, ctx: eframe::egui::Context, skip_concealed: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut cb = match arboard::Clipboard::new() {
            Ok(cb) => cb,
            Err(_) => return,
        };
        let mut last_count = platform::change_count();
        let mut last_text_hash: Option<u64> = None;
        loop {
            std::thread::sleep(Duration::from_millis(350));
            let count = platform::change_count();
            // Where there is no change counter (other systems), compare the text each time.
            if count >= 0 && count == last_count {
                continue;
            }
            last_count = count;
            if skip_concealed.load(Ordering::Relaxed) && platform::is_concealed() {
                continue;
            }
            let source = platform::frontmost_app_name();
            if let Ok(text) = cb.get_text() {
                if platform::change_count() != count || text.trim().is_empty() {
                    continue;
                }
                let hash = fnv1a(text.as_bytes());
                if count < 0 && last_text_hash == Some(hash) {
                    continue;
                }
                last_text_hash = Some(hash);
                let _ = tx.send(Msg::Clip(Content::Text { text }, hash, source));
                ctx.request_repaint();
                continue;
            }
            if count < 0 {
                continue;
            }
            if let Ok(img) = cb.get_image() {
                if platform::change_count() != count {
                    continue;
                }
                let hash = fnv1a(&img.bytes);
                let file = format!("{hash:016x}.png");
                let Ok(path) = store::image_path(&file) else {
                    continue;
                };
                let (w, h) = (img.width as u32, img.height as u32);
                if !path.is_file() {
                    let Some(buf) = image::RgbaImage::from_raw(w, h, img.bytes.into_owned()) else {
                        continue;
                    };
                    let mut png = Vec::new();
                    if image::codecs::png::PngEncoder::new(&mut png)
                        .write_image(buf.as_raw(), w, h, image::ExtendedColorType::Rgba8)
                        .is_err()
                        || store::save_image(&file, &png).is_err()
                    {
                        continue;
                    }
                }
                let _ = tx.send(Msg::Clip(Content::Image { file, w, h }, hash, source));
                ctx.request_repaint();
            }
        }
    });
}

/// Puts content on the clipboard.
pub fn set_clipboard(content: &Content) -> bool {
    let Ok(mut cb) = arboard::Clipboard::new() else {
        return false;
    };
    match content {
        Content::Text { text } => cb.set_text(text.clone()).is_ok(),
        Content::Image { file, .. } => {
            let Ok(path) = store::image_path(file) else {
                return false;
            };
            let Ok(img) = image::open(path) else {
                return false;
            };
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            cb.set_image(arboard::ImageData {
                width: w as usize,
                height: h as usize,
                bytes: std::borrow::Cow::Owned(rgba.into_raw()),
            })
            .is_ok()
        }
    }
}
