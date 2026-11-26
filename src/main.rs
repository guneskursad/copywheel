mod app;
mod art;
mod platform;
mod snippet;
mod store;
mod watch;

use eframe::egui;

const DISPLAY_NAME: &str = "CopyWheel";

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let shot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(DISPLAY_NAME)
        .with_inner_size(if std::env::var("COPYWHEEL_START").is_ok_and(|s| s.starts_with("wheel")) {
            [app::WHEEL_W, app::WHEEL_H]
        } else {
            [app::WIN_W, app::WIN_H]
        })
        .with_icon(egui::IconData { rgba: art::logo_rgba(256, art::Style::Tile), width: 256, height: 256 })
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false);
    if shot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    } else {
        viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnTop);
    }

    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if shot.is_some() {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(DISPLAY_NAME, options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, shot)))))
}
