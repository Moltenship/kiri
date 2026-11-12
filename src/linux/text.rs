//! Text for annotations, laid out by cosmic-text as GPUI lays out its own on Linux, in the first
//! UI font the system has from the families GPUI falls back through.

use std::sync::Mutex;

use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};

const FAMILIES: [&str; 10] =
    ["IBM Plex Sans", "Ubuntu", "Adwaita Sans", "Cantarell", "Noto Sans", "DejaVu Sans", "Helvetica", "Segoe UI", "Arial", "Liberation Sans"];

struct Fonts {
    system: FontSystem,
    glyphs: SwashCache,
    family: Option<String>,
}

/// Loading the system's fonts takes a while, so it happens once, on first use.
fn with_fonts<T>(f: impl FnOnce(&mut Fonts) -> T) -> T {
    static FONTS: Mutex<Option<Fonts>> = Mutex::new(None);
    let mut fonts = FONTS.lock().unwrap();
    let fonts = fonts.get_or_insert_with(|| {
        let system = FontSystem::new();
        let has = |name: &str| system.db().faces().any(|f| f.families.iter().any(|(n, _)| n == name));
        let family = FAMILIES.iter().find(|name| has(name)).map(|name| name.to_string());
        Fonts { system, glyphs: SwashCache::new(), family }
    });
    f(fonts)
}

fn layout(fonts: &mut Fonts, text: &str, size: f32, line_height: f32, bold: bool) -> Buffer {
    let mut buffer = Buffer::new(&mut fonts.system, Metrics::new(size, size * line_height));
    let family = fonts.family.as_deref().map_or(Family::SansSerif, Family::Name);
    let attrs = Attrs::new().family(family).weight(if bold { Weight::SEMIBOLD } else { Weight::NORMAL });
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut fonts.system, false);
    buffer
}

/// Width of one line in the UI font. Labels are measured several times a frame, so widths are cached.
pub fn text_width(text: &str, size: f32, bold: bool) -> f32 {
    thread_local! {
        static WIDTHS: std::cell::RefCell<std::collections::HashMap<(String, u32, bool), f32>> = Default::default();
    }
    WIDTHS.with_borrow_mut(|cache| {
        if let Some(&w) = cache.get(&(text.to_string(), size.to_bits(), bold)) {
            return w;
        }
        if cache.len() > 4096 {
            cache.clear();
        }
        let w = with_fonts(|fonts| layout(fonts, text, size, 1., bold).layout_runs().map(|run| run.line_w).fold(0., f32::max));
        cache.insert((text.to_string(), size.to_bits(), bold), w);
        w
    })
}

/// One line of text as premultiplied RGBA, laid out in a line box of `size * line_height`
/// with the line centered in it.
pub fn text_image(text: &str, size: f32, line_height: f32, rgb: u32, bold: bool) -> Option<(u32, u32, Vec<u8>)> {
    let w = text_width(text, size, bold).ceil() as i32 + 2;
    let h = (size * line_height).ceil() as i32;
    if w <= 2 || h <= 0 {
        return None;
    }
    let mut coverage = vec![0u8; (w * h) as usize];
    with_fonts(|fonts| {
        let mut buffer = layout(fonts, text, size, line_height, bold);
        let Fonts { system, glyphs, .. } = fonts;
        buffer.draw(system, glyphs, Color::rgb(255, 255, 255), |x, y, gw, gh, color| {
            for (py, px) in (y..y + gh as i32).flat_map(|py| (x..x + gw as i32).map(move |px| (py, px))) {
                if (0..w).contains(&px) && (0..h).contains(&py) {
                    let c = &mut coverage[(py * w + px) as usize];
                    *c = (*c).max(color.a());
                }
            }
        });
    });
    let [r, g, b] = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255];
    let rgba = coverage.iter().flat_map(|&a| [r * a as u32 / 255, g * a as u32 / 255, b * a as u32 / 255, a as u32].map(|c| c as u8)).collect();
    Some((w as u32, h as u32, rgba))
}
