//! Text recognition through the `tesseract` command, in the languages installed for it, and QR
//! codes through `crate::ocr::qr`.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::ocr::Recognized;

type Box4 = (f32, f32, f32, f32);

/// Tesseract reads text about 20 px tall best, so screen text is read enlarged, up to this many pixels.
const UPSCALE: u32 = 3;
const UPSCALE_PIXELS: u64 = 20_000_000;
const MISSING: &str = "Text recognition needs Tesseract: install the tesseract-ocr package";

pub fn recognize(bgra: &[u8], width: u32, height: u32, languages: &[String]) -> Result<Recognized, String> {
    if let Some(code) = crate::ocr::qr(bgra, width, height) {
        return Ok(Recognized::Barcode(code));
    }
    let text = lines(bgra, width, height, languages)?.into_iter().map(|(_, line)| line).collect::<Vec<_>>().join("\n");
    Ok(if text.trim().is_empty() { Recognized::Nothing } else { Recognized::Text(text) })
}

/// Line boxes in image pixels, origin top-left.
pub fn text_boxes(bgra: &[u8], width: u32, height: u32) -> Result<Vec<Box4>, String> {
    Ok(lines(bgra, width, height, &[])?.into_iter().map(|(r, _)| r).collect())
}

/// Tesseract's installed languages, as (code, English name). Asked once.
pub fn languages() -> &'static [(String, String)] {
    static LANGUAGES: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    LANGUAGES.get_or_init(|| {
        let Ok(out) = Command::new("tesseract").arg("--list-langs").stderr(Stdio::null()).output() else { return Vec::new() };
        // The first line names the folder the languages are in.
        let list = String::from_utf8_lossy(&out.stdout).lines().skip(1).map(str::trim).filter(|c| !c.is_empty() && *c != "osd").map(str::to_string).collect::<Vec<_>>();
        list.into_iter().map(|code| (code.clone(), name(&code).map_or(code, str::to_string))).collect()
    })
}

fn name(code: &str) -> Option<&'static str> {
    Some(match code {
        "eng" => "English",
        "deu" => "German",
        "fra" => "French",
        "spa" => "Spanish",
        "ita" => "Italian",
        "por" => "Portuguese",
        "nld" => "Dutch",
        "pol" => "Polish",
        "rus" => "Russian",
        "ukr" => "Ukrainian",
        "tur" => "Turkish",
        "ara" => "Arabic",
        "hin" => "Hindi",
        "jpn" => "Japanese",
        "kor" => "Korean",
        "chi_sim" => "Chinese (Simplified)",
        "chi_tra" => "Chinese (Traditional)",
        _ => return None,
    })
}

/// Lines top to bottom with their boxes. An empty `languages` reads in every installed one.
fn lines(bgra: &[u8], width: u32, height: u32, languages: &[String]) -> Result<Vec<(Box4, String)>, String> {
    if width == 0 || height == 0 || bgra.len() != (width * height * 4) as usize {
        return Err("Invalid image".into());
    }
    let up = (1..=UPSCALE).rev().find(|s| (width * s) as u64 * (height * s) as u64 <= UPSCALE_PIXELS).unwrap_or(1);
    let (w, h) = (width * up, height * up);
    let mut pnm = format!("P6\n{w} {h}\n255\n").into_bytes();
    if up == 1 {
        pnm.extend(bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0]]));
    } else {
        let image = image::RgbaImage::from_raw(width, height, bgra.to_vec()).ok_or("Invalid image")?;
        let large = image::imageops::resize(&image, w, h, image::imageops::FilterType::CatmullRom);
        pnm.extend(large.into_raw().chunks_exact(4).flat_map(|p| [p[2], p[1], p[0]]));
    }
    let installed: Vec<&str> = self::languages().iter().map(|(code, _)| code.as_str()).collect();
    let chosen: Vec<&str> = languages.iter().map(String::as_str).filter(|c| installed.contains(c)).collect();
    let chosen = if chosen.is_empty() { installed } else { chosen };
    let mut command = Command::new("tesseract");
    command.args(["stdin", "stdout", "--psm", "3"]);
    if !chosen.is_empty() {
        command.arg("-l").arg(chosen.join("+"));
    }
    let mut child = command.arg("tsv").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => MISSING.to_string(),
        _ => format!("Recognition failed: {e}"),
    })?;
    let mut stdin = child.stdin.take().ok_or("Recognition failed")?;
    let feed = std::thread::spawn(move || stdin.write_all(&pnm));
    let out = child.wait_with_output().map_err(|e| format!("Recognition failed: {e}"))?;
    let _ = feed.join();
    if !out.status.success() {
        return Err(format!("Recognition failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(parse_tsv(&String::from_utf8_lossy(&out.stdout), 1. / up as f32))
}

/// Words grouped into their lines, with each line's box scaled by `k`.
fn parse_tsv(tsv: &str, k: f32) -> Vec<(Box4, String)> {
    let mut lines: Vec<((u32, u32, u32), (f32, f32, f32, f32), String)> = Vec::new();
    for row in tsv.lines().skip(1) {
        let f: Vec<&str> = row.split('\t').collect();
        let [level, _, block, par, line, _, left, top, w, h, _, text] = f[..] else { continue };
        let n = |s: &str| s.parse::<f32>().unwrap_or(0.);
        if level != "5" || text.trim().is_empty() {
            continue;
        }
        let key = (block.parse().unwrap_or(0), par.parse().unwrap_or(0), line.parse().unwrap_or(0));
        let (x0, y0, x1, y1) = (n(left), n(top), n(left) + n(w), n(top) + n(h));
        match lines.last_mut() {
            Some((at, r, words)) if *at == key => {
                *r = (r.0.min(x0), r.1.min(y0), r.2.max(x1), r.3.max(y1));
                words.push(' ');
                words.push_str(text.trim());
            }
            _ => lines.push((key, (x0, y0, x1, y1), text.trim().to_string())),
        }
    }
    let mut lines: Vec<(Box4, String)> =
        lines.into_iter().map(|(_, (x0, y0, x1, y1), text)| ((x0 * k, y0 * k, (x1 - x0) * k, (y1 - y0) * k), text)).collect();
    lines.sort_by(|(a, _), (b, _)| a.1.total_cmp(&b.1));
    lines
}

#[cfg(test)]
mod tests {
    #[test]
    fn tsv_words_become_lines_with_their_boxes() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t400\t200\t-1\t\n\
                   5\t1\t1\t1\t2\t1\t10\t60\t40\t20\t95\tsecond\n\
                   5\t1\t1\t1\t1\t1\t10\t10\t50\t20\t96\tHello\n\
                   5\t1\t1\t1\t1\t2\t70\t12\t60\t22\t96\tworld\n";
        let lines = super::parse_tsv(tsv, 0.5);
        assert_eq!(lines.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["Hello world", "second"]);
        assert_eq!(lines[0].0, (5., 5., 60., 12.));
    }
}
