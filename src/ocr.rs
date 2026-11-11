pub enum Recognized {
    Barcode(String),
    Text(String),
    Nothing,
}

#[cfg(target_os = "macos")]
mod sys {
    use objc2::AnyThread;
    use objc2::rc::{Retained, autoreleasepool};
    use objc2_core_foundation::{CFData, CFRetained, CGRect};
    use objc2_core_graphics::{
        CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
        CGImageByteOrderInfo,
    };
    use objc2_foundation::{NSArray, NSDictionary, NSLocale, NSString};
    use objc2_vision::{
        VNDetectBarcodesRequest, VNImageRequestHandler, VNRecognizeTextRequest,
        VNRecognizedTextObservation, VNRequest, VNRequestTextRecognitionLevel,
    };

    use super::Recognized;

    /// `bgra` is tightly packed BGRA8 (premultiplied alpha, alpha is usually 255), width*height*4 bytes.
    /// `languages` are BCP-47 codes like "en-US", "ru-RU"; empty means automatic language detection.
    /// Blocking; Kiri calls it from a background thread.
    pub fn recognize(
        bgra: &[u8],
        width: u32,
        height: u32,
        languages: &[String],
    ) -> Result<Recognized, String> {
        autoreleasepool(|_| {
            let handler = handler(bgra, width, height)?;

            let barcodes = unsafe { VNDetectBarcodesRequest::new() };
            perform(&handler, &barcodes)?;
            let payload = unsafe { barcodes.results() }.and_then(|results| {
                results
                    .iter()
                    .filter_map(|barcode| unsafe { barcode.payloadStringValue() })
                    .map(|payload| payload.to_string())
                    .find(|payload| !payload.is_empty())
            });
            if let Some(payload) = payload {
                return Ok(Recognized::Barcode(payload));
            }

            let request = text_request(true);
            if languages.is_empty() {
                request.setAutomaticallyDetectsLanguage(true);
            } else {
                let languages: Vec<_> = languages.iter().map(|l| NSString::from_str(l)).collect();
                request.setRecognitionLanguages(&NSArray::from_retained_slice(&languages));
            }
            perform(&handler, &request)?;
            let text = lines(&request)
                .iter()
                .filter_map(|(_, line)| line.topCandidates(1).firstObject())
                .map(|candidate| candidate.string().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            Ok(if text.trim().is_empty() {
                Recognized::Nothing
            } else {
                Recognized::Text(text)
            })
        })
    }

    /// Glyph/line boxes for "blur text" redaction: rectangles in image pixel coordinates, origin top-left.
    pub fn text_boxes(
        bgra: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<(f32, f32, f32, f32)>, String> {
        autoreleasepool(|_| {
            let handler = handler(bgra, width, height)?;
            let request = text_request(false);
            request.setAutomaticallyDetectsLanguage(true);
            perform(&handler, &request)?;
            let (w, h) = (width as f64, height as f64);
            Ok(lines(&request)
                .into_iter()
                .map(|(r, _)| {
                    let top = (1.0 - r.origin.y - r.size.height) * h;
                    (
                        (r.origin.x * w) as f32,
                        top as f32,
                        (r.size.width * w) as f32,
                        (r.size.height * h) as f32,
                    )
                })
                .collect())
        })
    }

    /// Every language this Mac's Vision can read, as (code, name in the user's language). Asked once.
    pub fn languages() -> &'static [(String, String)] {
        static LANGUAGES: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
        LANGUAGES.get_or_init(|| {
            autoreleasepool(|_| {
                let Ok(codes) = (unsafe { text_request(true).supportedRecognitionLanguagesAndReturnError() }) else {
                    return Vec::new();
                };
                let locale = NSLocale::currentLocale();
                codes
                    .iter()
                    .map(|code| {
                        // Vision lists some codes, like Vietnamese's "vi-VT", that the locale has no name for.
                        let name = locale.localizedStringForLocaleIdentifier(&code).to_string();
                        let lang = NSString::from_str(code.to_string().split('-').next().unwrap_or_default());
                        let name = if name.is_empty() { locale.localizedStringForLanguageCode(&lang).map(|n| n.to_string()) } else { Some(name) };
                        (code.to_string(), name.filter(|n| !n.is_empty()).unwrap_or_else(|| code.to_string()))
                    })
                    .collect()
            })
        })
    }

    fn text_request(language_correction: bool) -> Retained<VNRecognizeTextRequest> {
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(language_correction);
        request
    }

    /// Observations with their normalized, bottom-left-origin boxes, sorted top to bottom.
    fn lines(request: &VNRecognizeTextRequest) -> Vec<(CGRect, Retained<VNRecognizedTextObservation>)> {
        let mut lines: Vec<_> = request
            .results()
            .map(|results| {
                results
                    .iter()
                    .map(|line| (unsafe { line.boundingBox() }, line))
                    .filter(|(r, _)| r.size.width.is_finite() && r.size.height.is_finite())
                    .collect()
            })
            .unwrap_or_default();
        lines.sort_by(|(a, _), (b, _)| {
            (b.origin.y + b.size.height).total_cmp(&(a.origin.y + a.size.height))
        });
        lines
    }

    fn perform(handler: &VNImageRequestHandler, request: &VNRequest) -> Result<(), String> {
        handler
            .performRequests_error(&NSArray::from_slice(&[request]))
            .map_err(|e| format!("Recognition failed: {}", e.localizedDescription()))
    }

    fn handler(
        bgra: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Retained<VNImageRequestHandler>, String> {
        let image = cg_image(bgra, width, height)?;
        Ok(unsafe {
            VNImageRequestHandler::initWithCGImage_options(
                VNImageRequestHandler::alloc(),
                &image,
                &NSDictionary::new(),
            )
        })
    }

    fn cg_image(bgra: &[u8], width: u32, height: u32) -> Result<CFRetained<CGImage>, String> {
        let (w, h) = (width as usize, height as usize);
        if w == 0 || h == 0 || w.checked_mul(h).and_then(|n| n.checked_mul(4)) != Some(bgra.len()) {
            return Err("Invalid image".into());
        }
        // Copied so Vision never reads the caller's buffer after we return.
        let provider = CGDataProvider::with_cf_data(Some(&CFData::from_bytes(bgra)));
        let space = CGColorSpace::new_device_rgb();
        unsafe {
            CGImage::new(
                w,
                h,
                8,
                32,
                w * 4,
                space.as_deref(),
                CGBitmapInfo(
                    CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0,
                ),
                provider.as_deref(),
                std::ptr::null(),
                false,
                CGColorRenderingIntent::RenderingIntentDefault,
            )
        }
        .ok_or_else(|| "Invalid image".into())
    }
}

#[cfg(windows)]
use crate::win::ocr as sys;

pub use sys::{languages, recognize, text_boxes};

/// Copies OCR text with or without line breaks; the toast offers the other one.
fn copy_text(text: String, join: bool, cx: &mut gpui_kit::App) {
    let copied = if join { text.replace('\n', " ") } else { text.clone() };
    let preview = copied.split_whitespace().collect::<Vec<_>>().join(" ");
    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(copied));
    let label = if join { "Add breaks" } else { "Cut breaks" };
    crate::toast::done(
        format!("Text copied: {preview}"),
        Some((label, std::rc::Rc::new(move |cx: &mut gpui_kit::App| copy_text(text.clone(), !join, cx)))),
        cx,
    );
}

/// The overlay's OCR capture: recognize off the main thread, copy, confirm.
pub fn recognize_and_copy(bgra: Vec<u8>, w: u32, h: u32, cx: &mut gpui_kit::App) {
    let s = crate::settings::get(cx);
    let (languages, join) = (s.ocr_languages.clone(), s.ocr_join_lines);
    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move { recognize(&bgra, w, h, &languages) })
            .await;
        cx.update(|cx| match result {
            Ok(Recognized::Barcode(code)) => {
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(code.clone()));
                let toast = format!("Code copied: {code}");
                let url = code.starts_with("http://") || code.starts_with("https://");
                let open: Option<crate::toast::Action> =
                    url.then(|| ("Open", std::rc::Rc::new(move |cx: &mut gpui_kit::App| cx.open_url(&code)) as _));
                crate::toast::done(toast, open, cx);
            }
            Ok(Recognized::Text(text)) => copy_text(text, join, cx),
            Ok(Recognized::Nothing) => crate::toast::show("No text detected", cx),
            Err(err) => crate::toast::show(err, cx),
        });
    })
    .detach();
}

/// Vision builds its model cache on first use, which takes many seconds once per
/// binary. A tiny throwaway request at launch moves that wait off the first real OCR.
pub fn warm_up(cx: &gpui_kit::App) {
    cx.background_executor()
        .spawn(async {
            let blank = vec![255u8; 64 * 32 * 4];
            let _ = recognize(&blank, 64, 32, &[]);
        })
        .detach();
}

