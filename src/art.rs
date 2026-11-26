//! The CopyWheel logo as pixels: the same mark the window draws (four ring pieces with
//! round ends around a solid hub, an accent dot flying off at the top right), used for the
//! menu-bar icon, the window icon and the app icon (see `examples/app_icon.rs`).

use std::f32::consts::{PI, TAU};

/// Warm cream tile, dark ink and the orange dot.
pub const CREAM_TOP: [u8; 3] = [255, 243, 230];
pub const CREAM_BOTTOM: [u8; 3] = [253, 233, 213];
pub const INK: [u8; 3] = [43, 37, 35];
pub const DOT: [u8; 3] = [255, 84, 16];

#[derive(Clone, Copy, PartialEq)]
pub enum Style {
    /// Black on transparent, the shape only (macOS tints it for the menu bar).
    Template,
    /// Ink mark with its orange dot on a rounded cream tile (the app icon).
    Tile,
}

/// Signed distance (in 24-unit grid units) to the ring pieces and the hub.
fn mark_distance(x: f32, y: f32, stroke: f32) -> f32 {
    let (cx, cy, r) = (12.0, 12.5, 7.6);
    let half = PI / 4.0 - 0.36;
    let (dx, dy) = (x - cx, y - cy);
    let len = (dx * dx + dy * dy).sqrt();
    let a = dy.atan2(dx);
    let mut best = f32::MAX;
    for k in 0..4 {
        let mid = -PI / 2.0 + k as f32 * TAU / 4.0;
        let off = (a - mid + PI).rem_euclid(TAU) - PI;
        let d = if off.abs() <= half {
            (len - r).abs()
        } else {
            let e = mid + half * off.signum();
            let (ex, ey) = (cx + r * e.cos(), cy + r * e.sin());
            ((x - ex).powi(2) + (y - ey).powi(2)).sqrt()
        };
        best = best.min(d - stroke / 2.0);
    }
    best.min(len - 2.5)
}

fn dot_distance(x: f32, y: f32) -> f32 {
    ((x - 20.1).powi(2) + (y - 4.5).powi(2)).sqrt() - 2.7
}

/// Rounded-square tile distance in pixels for an `n`-pixel image.
fn tile_distance(px: f32, py: f32, n: f32) -> f32 {
    let inset = n * 0.098; // the margin macOS icons leave around the tile
    let half = n / 2.0 - inset;
    let radius = half * 0.45;
    let c = n / 2.0;
    let qx = (px - c).abs() - (half - radius);
    let qy = (py - c).abs() - (half - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

fn cover(d_px: f32) -> f32 {
    (0.5 - d_px).clamp(0.0, 1.0)
}

/// Straight-alpha RGBA pixels, `n` x `n`.
pub fn logo_rgba(n: usize, style: Style) -> Vec<u8> {
    let nf = n as f32;
    // How much of the image the 24-unit grid spans.
    let (grid_px, origin) = match style {
        Style::Template => (nf, 0.0),
        Style::Tile => (nf * 0.58, nf * 0.21),
    };
    let unit = grid_px / 24.0;
    let stroke = if style == Style::Template { 3.4 } else { 3.1 };
    const SS: usize = 4; // supersamples per side
    let mut px = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let mut acc = [0f32; 4]; // premultiplied r g b a
            for sy in 0..SS {
                for sx in 0..SS {
                    let fx = x as f32 + (sx as f32 + 0.5) / SS as f32;
                    let fy = y as f32 + (sy as f32 + 0.5) / SS as f32;
                    let gx = (fx - origin) / unit;
                    let gy = (fy - origin) / unit;
                    let mark = cover(mark_distance(gx, gy, stroke) * unit);
                    let dot = cover(dot_distance(gx, gy) * unit);
                    let (rgb, a) = match style {
                        Style::Template => ([0.0; 3], mark.max(dot)),
                        Style::Tile => {
                            let tile = cover(tile_distance(fx, fy, nf));
                            let t = fy / nf;
                            let mut c = [0f32; 3];
                            for i in 0..3 {
                                let bg = CREAM_TOP[i] as f32 + (CREAM_BOTTOM[i] as f32 - CREAM_TOP[i] as f32) * t;
                                let with_mark = bg + (INK[i] as f32 - bg) * mark;
                                c[i] = with_mark + (DOT[i] as f32 - with_mark) * dot;
                            }
                            (c, tile)
                        }
                    };
                    for i in 0..3 {
                        acc[i] += rgb[i] * a;
                    }
                    acc[3] += a;
                }
            }
            let s = (SS * SS) as f32;
            let a = acc[3] / s;
            let i = (y * n + x) * 4;
            if a > 0.0 {
                for c in 0..3 {
                    px[i + c] = (acc[c] / s / a).round().clamp(0.0, 255.0) as u8;
                }
            }
            px[i + 3] = (a * 255.0).round() as u8;
        }
    }
    px
}
