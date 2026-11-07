//! Text recognition through Windows.Media.Ocr, in the languages installed in Windows, and QR
//! codes through rqrr, since Windows has no barcode reader for still images.

use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::core::HSTRING;

use crate::ocr::Recognized;

type Box4 = (f32, f32, f32, f32);

const MARGIN: u32 = 24;
/// Windows misses UI text under about 12 px tall, so small images are read enlarged, up to this many pixels.
const UPSCALE: u32 = 3;
const UPSCALE_PIXELS: u64 = 20_000_000;

/// A QR code's text if the image has one, as on macOS, else the image's text.
pub fn recognize(bgra: &[u8], width: u32, height: u32, languages: &[String]) -> Result<Recognized, String> {
    if let Some(code) = qr(bgra, width, height) {
        return Ok(Recognized::Barcode(code));
    }
    let text = lines(bgra, width, height, languages)?.into_iter().map(|(_, line)| line).collect::<Vec<_>>().join("\n");
    Ok(if text.trim().is_empty() { Recognized::Nothing } else { Recognized::Text(text) })
}

/// Dark on light, then light on dark.
fn qr(bgra: &[u8], width: u32, height: u32) -> Option<String> {
    if bgra.len() != (width as usize) * (height as usize) * 4 {
        return None;
    }
    let luma: Vec<u8> = bgra.chunks_exact(4).map(|p| ((p[2] as u32 * 77 + p[1] as u32 * 150 + p[0] as u32 * 29) >> 8) as u8).collect();
    [false, true].into_iter().find_map(|invert| {
        let w = width as usize;
        let mut image = rqrr::PreparedImage::prepare_from_greyscale(w, height as usize, |x, y| match invert {
            false => luma[y * w + x],
            true => 255 - luma[y * w + x],
        });
        image.detect_grids().iter().filter_map(|grid| grid.decode().ok()).map(|(_, text)| text).find(|text| !text.is_empty())
    })
}

/// Line boxes in image pixels, origin top-left.
pub fn text_boxes(bgra: &[u8], width: u32, height: u32) -> Result<Vec<Box4>, String> {
    Ok(lines(bgra, width, height, &[])?.into_iter().map(|(r, _)| r).collect())
}

/// Every installed recognition language, as (code, name in the user's language). Asked once.
pub fn languages() -> &'static [(String, String)] {
    static LANGUAGES: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    LANGUAGES.get_or_init(|| {
        let Ok(list) = OcrEngine::AvailableRecognizerLanguages() else { return Vec::new() };
        list.into_iter().filter_map(|l| Some((l.LanguageTag().ok()?.to_string(), l.DisplayName().ok()?.to_string()))).collect()
    })
}

/// Lines top to bottom with their boxes. An empty `languages` reads in the user's languages.
fn lines(bgra: &[u8], width: u32, height: u32, languages: &[String]) -> Result<Vec<(Box4, String)>, String> {
    let fail = |e: windows::core::Error| format!("Recognition failed: {}", e.message());
    if width == 0 || height == 0 || bgra.len() != (width * height * 4) as usize {
        return Err("Invalid image".into());
    }
    let chosen = languages
        .first()
        .and_then(|code| OcrEngine::TryCreateFromLanguage(&Language::CreateLanguage(&HSTRING::from(code)).ok()?).ok());
    let engine = match chosen {
        Some(engine) => engine,
        None => OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| "Add a language with text recognition in Windows Settings".to_string())?,
    };
    // Text touching the edge goes unread, so the image gets a margin in its corner color.
    // Larger images are read at a whole fraction of their size.
    let max = OcrEngine::MaxImageDimension().map_err(fail)?.max(2 * MARGIN + 1);
    let up = (1..=UPSCALE)
        .rev()
        .find(|s| width.max(height) * s <= max - 2 * MARGIN && (width * s) as u64 * (height * s) as u64 <= UPSCALE_PIXELS)
        .unwrap_or(1);
    let enlarged;
    let (bgra, width, height) = match up {
        1 => (bgra, width, height),
        _ => {
            let image = image::RgbaImage::from_raw(width, height, bgra.to_vec()).ok_or("Invalid image")?;
            enlarged = image::imageops::resize(&image, width * up, height * up, image::imageops::FilterType::CatmullRom).into_raw();
            (&enlarged[..], width * up, height * up)
        }
    };
    let k = width.max(height).div_ceil(max - 2 * MARGIN).max(1);
    let (w, h) = (width / k + 2 * MARGIN, height / k + 2 * MARGIN);
    let mut pixels: Vec<u8> = bgra[..4].iter().copied().cycle().take((w * h * 4) as usize).collect();
    for y in 0..height / k {
        for x in 0..width / k {
            let (from, to) = (((y * k * width + x * k) * 4) as usize, (((y + MARGIN) * w + x + MARGIN) * 4) as usize);
            pixels[to..to + 4].copy_from_slice(&bgra[from..from + 4]);
        }
    }
    let writer = DataWriter::new().map_err(fail)?;
    writer.WriteBytes(&pixels).map_err(fail)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&writer.DetachBuffer().map_err(fail)?, BitmapPixelFormat::Bgra8, w as i32, h as i32)
        .map_err(fail)?;
    let result = engine.RecognizeAsync(&bitmap).map_err(fail)?.join().map_err(fail)?;
    let k = k as f32 / up as f32;
    let mut lines = Vec::new();
    for line in result.Lines().map_err(fail)? {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for word in line.Words().map_err(fail)? {
            let r = word.BoundingRect().map_err(fail)?;
            (x0, y0) = (x0.min(r.X), y0.min(r.Y));
            (x1, y1) = (x1.max(r.X + r.Width), y1.max(r.Y + r.Height));
        }
        if x1 > x0 && y1 > y0 {
            let (x, y) = (x0 - MARGIN as f32, y0 - MARGIN as f32);
            lines.push(((x * k, y * k, (x1 - x0) * k, (y1 - y0) * k), line.Text().map_err(fail)?.to_string()));
        }
    }
    lines.sort_by(|(a, _), (b, _)| a.1.total_cmp(&b.1));
    Ok(lines)
}

#[cfg(test)]
mod tests {
    /// "kiri", as segno draws it.
    const KIRI: [&str; 21] = [
        "#######.......#######",
        "#.....#.#.###.#.....#",
        "#.###.#....##.#.###.#",
        "#.###.#...#.#.#.###.#",
        "#.###.#.##..#.#.###.#",
        "#.....#...#.#.#.....#",
        "#######.#.#.#.#######",
        ".........#.##........",
        "#.#.#.#..#.#....#..#.",
        "##.###.#.#....#..####",
        ".#######..#.#...#####",
        "#.#.#..#.#....#.....#",
        "###.#.####..#.#.##...",
        "........#.##.#.#.#.##",
        "#######..###.###...##",
        "#.....#..#####.##..#.",
        "#.###.#.#.##.###...##",
        "#.###.#.......##.###.",
        "#.###.#.###.#...###.#",
        "#.....#..#....##.#.#.",
        "#######.#...#.#.#.###",
    ];

    #[test]
    fn reads_qr_codes_dark_on_light_and_light_on_dark() {
        let (scale, border, n) = (4, 4, (21 + 8) * 4);
        for (dark, light) in [([0, 0, 0, 255], [255; 4]), ([255; 4], [40, 30, 30, 255])] {
            let bgra: Vec<u8> = (0..n * n)
                .flat_map(|i| {
                    let (x, y) = ((i % n) / scale, (i / n) / scale);
                    let inside = (border..border + 21).contains(&x) && (border..border + 21).contains(&y);
                    if inside && KIRI[y - border].as_bytes()[x - border] == b'#' { dark } else { light }
                })
                .collect();
            assert_eq!(super::qr(&bgra, n as u32, n as u32).as_deref(), Some("kiri"));
        }
    }
}
