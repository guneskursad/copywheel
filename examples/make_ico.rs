//! Generates the Windows `.ico` icon from the CopyWheel logo art.
//! Usage: cargo run --release --example make_ico -- [assets/windows/copywheel.ico]

#[path = "../src/art.rs"]
mod art;

use image::ImageEncoder;
use std::path::PathBuf;

const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

fn encode_dib(size: u32, rgba: &[u8]) -> Vec<u8> {
    let n = size as usize;
    let and_stride = n.div_ceil(32) * 4;
    let xor_len = n * n * 4;
    let and_len = and_stride * n;
    let mut out = Vec::with_capacity(40 + xor_len + and_len);

    // BITMAPINFOHEADER (40 bytes)
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(size as i32).to_le_bytes()); // biWidth
    out.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // biHeight (XOR + AND)
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    out.extend_from_slice(&((xor_len + and_len) as u32).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    out.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    // XOR mask: bottom-up BGRA
    for y in (0..n).rev() {
        for x in 0..n {
            let i = (y * n + x) * 4;
            out.push(rgba[i + 2]);
            out.push(rgba[i + 1]);
            out.push(rgba[i]);
            out.push(rgba[i + 3]);
        }
    }

    // AND mask: bottom-up 1bpp padded to 4-byte rows (1 = transparent)
    for y in (0..n).rev() {
        let mut row = vec![0u8; and_stride];
        for x in 0..n {
            let alpha = rgba[(y * n + x) * 4 + 3];
            if alpha == 0 {
                row[x / 8] |= 1 << (7 - (x % 8));
            }
        }
        out.extend_from_slice(&row);
    }

    out
}

fn encode_png(size: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(rgba, size, size, image::ExtendedColorType::Rgba8)
        .expect("failed to encode PNG icon frame");
    out
}

fn build_ico() -> Vec<u8> {
    let entries: Vec<(u32, Vec<u8>)> = SIZES
        .into_iter()
        .map(|size| {
            let rgba = art::logo_rgba(size as usize, art::Style::Tile);
            let data = if size >= 256 {
                encode_png(size, &rgba)
            } else {
                encode_dib(size, &rgba)
            };
            (size, data)
        })
        .collect();

    let count = entries.len() as u16;
    let mut offset = 6u32 + u32::from(count) * 16;
    let total_len = offset as usize + entries.iter().map(|(_, d)| d.len()).sum::<usize>();
    let mut ico = Vec::with_capacity(total_len);

    // ICONDIR
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&count.to_le_bytes());

    // ICONDIRENTRY list
    for (size, data) in &entries {
        let dim = if *size >= 256 { 0u8 } else { *size as u8 };
        ico.push(dim);
        ico.push(dim);
        ico.push(0); // color count
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bit count
        ico.extend_from_slice(&(data.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }

    for (_, data) in entries {
        ico.extend_from_slice(&data);
    }

    ico
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/windows/copywheel.ico"));
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).expect("failed to create output directory");
    }
    let ico = build_ico();
    std::fs::write(&out, ico).expect("failed to write Windows icon");
}
