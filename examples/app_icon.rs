//! Writes the ClipWheel app icon as an .iconset folder (all the sizes macOS asks for).
//! `make-app.sh` runs it and turns the folder into AppIcon.icns with `iconutil`.
//! Usage: cargo run --release --example app_icon -- <folder.iconset>

#[path = "../src/art.rs"]
mod art;

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "AppIcon.iconset".into());
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out).expect("cannot make the iconset folder");
    for base in [16u32, 32, 128, 256, 512] {
        for scale in [1u32, 2] {
            let n = base * scale;
            let px = art::logo_rgba(n as usize, art::Style::Tile);
            let name = if scale == 1 {
                format!("icon_{base}x{base}.png")
            } else {
                format!("icon_{base}x{base}@2x.png")
            };
            image::save_buffer(out.join(name), &px, n, n, image::ColorType::Rgba8)
                .expect("cannot write icon");
        }
    }
    // Also a large preview beside it, handy for looking at the logo.
    let px = art::logo_rgba(1024, art::Style::Tile);
    let preview = out.with_file_name("AppIcon-preview.png");
    image::save_buffer(preview, &px, 1024, 1024, image::ColorType::Rgba8)
        .expect("cannot write preview");
}
