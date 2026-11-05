//! Printing: Windows' print dialog, then the image on one page through GDI.

use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{GlobalFree, HGLOBAL, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::Xps::{DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, PD_ENABLEPRINTHOOK, PD_NOPAGENUMS, PD_NOSELECTION, PD_RETURNDC, PD_RETURNDEFAULT,
    PD_USEDEVMODECOPIESANDCOLLATE, PRINTDLGW, PrintDlgW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos, WM_INITDIALOG,
};
use windows::core::w;

static ASKING: AtomicBool = AtomicBool::new(false);

/// Asks and prints, on its own thread: the dialog's modal loop would otherwise run Kiri's
/// window messages mid-update. The dialog never shows with a window of another thread as its
/// owner, so it goes unowned and centers itself over `over`. One dialog at a time.
pub fn print(png: &[u8], over: Option<usize>) -> Result<(), String> {
    let (w, h, rgba, scale) = crate::export::decode_rgba(png).ok_or("couldn't read the image")?;
    if ASKING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let asked = ask(over, w > h);
    ASKING.store(false, Ordering::SeqCst);
    let Some(dc) = asked? else { return Ok(()) };
    let printed = unsafe { page(dc, w, h, scale, &on_white(&rgba)) };
    let _ = unsafe { DeleteDC(dc) };
    printed
}

/// The printer's device context, or `None` when the user cancels.
fn ask(over: Option<usize>, landscape: bool) -> Result<Option<HDC>, String> {
    let mut dialog = PRINTDLGW {
        lStructSize: size_of::<PRINTDLGW>() as u32,
        Flags: PD_RETURNDEFAULT,
        lCustData: LPARAM(over.unwrap_or(0) as isize),
        lpfnPrintHook: Some(center),
        ..Default::default()
    };
    unsafe {
        // The default printer, turned to the image's orientation before the dialog shows it.
        if PrintDlgW(&mut dialog).as_bool() {
            orient(dialog.hDevMode, landscape);
        }
        dialog.Flags = PD_RETURNDC | PD_NOSELECTION | PD_NOPAGENUMS | PD_USEDEVMODECOPIESANDCOLLATE | PD_ENABLEPRINTHOOK;
        let chosen = PrintDlgW(&mut dialog).as_bool();
        for handle in [dialog.hDevMode, dialog.hDevNames] {
            if !handle.is_invalid() {
                let _ = GlobalFree(Some(handle));
            }
        }
        match (chosen, CommDlgExtendedError().0) {
            (true, _) if !dialog.hDC.is_invalid() => Ok(Some(dialog.hDC)),
            (_, 0) => Ok(None),
            (_, code) => Err(format!("the print dialog failed ({code:#x})")),
        }
    }
}

unsafe extern "system" fn center(dialog: HWND, message: u32, _: WPARAM, lparam: LPARAM) -> usize {
    if message != WM_INITDIALOG {
        return 0;
    }
    unsafe {
        let over = HWND((*(lparam.0 as *const PRINTDLGW)).lCustData.0 as *mut _);
        let (mut a, mut b) = (RECT::default(), RECT::default());
        if !over.is_invalid() && GetWindowRect(over, &mut a).is_ok() && GetWindowRect(dialog, &mut b).is_ok() {
            let x = (a.left + a.right - (b.right - b.left)) / 2;
            let y = (a.top + a.bottom - (b.bottom - b.top)) / 2;
            let _ = SetWindowPos(dialog, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
        }
        let _ = SetForegroundWindow(dialog);
    }
    0
}

unsafe fn orient(devmode: HGLOBAL, landscape: bool) {
    unsafe {
        let mode = GlobalLock(devmode) as *mut DEVMODEW;
        if mode.is_null() {
            return;
        }
        (*mode).dmFields |= DM_ORIENTATION;
        (*mode).Anonymous1.Anonymous1.dmOrientation = if landscape { DMORIENT_LANDSCAPE } else { DMORIENT_PORTRAIT } as i16;
        let _ = GlobalUnlock(devmode);
    }
}

/// Centered at its size on screen, one point to a 96th of an inch, or smaller to fit the page.
unsafe fn page(dc: HDC, w: u32, h: u32, scale: f32, bgra: &[u8]) -> Result<(), String> {
    unsafe {
        let doc = DOCINFOW { cbSize: size_of::<DOCINFOW>() as i32, lpszDocName: w!("Kiri"), ..Default::default() };
        if StartDocW(dc, &doc) <= 0 {
            return Err("the printer didn't start the document".into());
        }
        let (page_w, page_h) = (GetDeviceCaps(Some(dc), HORZRES) as f32, GetDeviceCaps(Some(dc), VERTRES) as f32);
        let dpi = GetDeviceCaps(Some(dc), LOGPIXELSX) as f32;
        let natural = dpi / 96. / scale;
        let fit = (page_w / w as f32).min(page_h / h as f32).min(natural);
        let (dw, dh) = ((w as f32 * fit).round() as i32, (h as f32 * fit).round() as i32);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        StartPage(dc);
        SetStretchBltMode(dc, HALFTONE);
        let _ = SetBrushOrgEx(dc, 0, 0, None);
        let x = ((page_w as i32) - dw) / 2;
        let y = ((page_h as i32) - dh) / 2;
        let drawn = StretchDIBits(dc, x, y, dw, dh, 0, 0, w as i32, h as i32, Some(bgra.as_ptr().cast()), &info, DIB_RGB_COLORS, SRCCOPY);
        EndPage(dc);
        EndDoc(dc);
        if drawn == 0 { Err("the printer didn't take the image".into()) } else { Ok(()) }
    }
}

/// Paper has no transparency, so transparent pixels print as the white of the page.
fn on_white(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|p| {
            let a = p[3] as u32;
            let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
            [mix(p[2]), mix(p[1]), mix(p[0]), 255]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn transparent_pixels_print_white_and_channels_swap_to_bgr() {
        assert_eq!(super::on_white(&[10, 20, 30, 255, 0, 0, 0, 0]), [30, 20, 10, 255, 255, 255, 255, 255]);
    }
}
