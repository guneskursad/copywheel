#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod art;
mod platform;
mod snippet;
mod store;
mod watch;

use eframe::egui;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::PathBuf;

const DISPLAY_NAME: &str = "CopyWheel";

#[derive(Debug, Default, PartialEq, Eq)]
struct CliArgs {
    background: bool,
    shot: Option<PathBuf>,
}

fn parse_args<I, S>(args: I) -> CliArgs
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut cli = CliArgs::default();
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--background" => cli.background = true,
            "--screenshot" => {
                if let Some(path) = iter.next() {
                    cli.shot = Some(PathBuf::from(path.as_ref()));
                }
            }
            _ => {}
        }
    }
    cli
}

/// Acquires the per-user single-instance lock file. Returns `None` if another instance holds it.
fn acquire_instance_lock() -> Option<File> {
    let dir = store::data_dir();
    let _ = store::create_private_dir(&dir);
    let path = dir.join("copywheel.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .ok()?;
    match file.try_lock() {
        Ok(()) => Some(file),
        Err(TryLockError::WouldBlock) => None,
        Err(TryLockError::Error(_)) => Some(file),
    }
}

fn main() -> eframe::Result<()> {
    let cli = parse_args(std::env::args());
    let _instance_lock = if cli.shot.is_none() {
        match acquire_instance_lock() {
            Some(lock) => Some(lock),
            None => return Ok(()),
        }
    } else {
        None
    };
    let _instance_mutex = if cli.shot.is_none() {
        platform::create_instance_mutex()
    } else {
        None
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(DISPLAY_NAME)
        .with_inner_size(
            if std::env::var("COPYWHEEL_START").is_ok_and(|s| s.starts_with("wheel")) {
                [app::WHEEL_W, app::WHEEL_H]
            } else {
                [app::WIN_W, app::WIN_H]
            },
        )
        .with_icon(egui::IconData {
            rgba: art::logo_rgba(256, art::Style::Tile),
            width: 256,
            height: 256,
        })
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false);

    #[cfg(windows)]
    {
        viewport = viewport.with_taskbar(false);
    }

    if cli.shot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    } else {
        viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnTop);
        if cli.background {
            viewport = viewport.with_visible(false).with_active(false);
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    if cli.shot.is_some() || cli.background {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }

    let background = cli.background && cli.shot.is_none();
    let shot = cli.shot;
    eframe::run_native(
        DISPLAY_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, shot, background)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_handles_background_and_screenshot_flags() {
        assert_eq!(
            parse_args(["copywheel"]),
            CliArgs {
                background: false,
                shot: None,
            }
        );
        assert_eq!(
            parse_args(["copywheel", "--background"]),
            CliArgs {
                background: true,
                shot: None,
            }
        );
        assert_eq!(
            parse_args(["copywheel", "--screenshot", "out.png", "--background"]),
            CliArgs {
                background: true,
                shot: Some(PathBuf::from("out.png")),
            }
        );
        assert_eq!(
            parse_args(["copywheel", "--unknown", "--screenshot"]),
            CliArgs {
                background: false,
                shot: None,
            }
        );
    }
}
