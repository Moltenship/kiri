//! Window capture through Windows.Graphics.Capture, which reads a window as the compositor
//! has it, covered or not. The window list comes from `EnumWindows`, front to back.

use std::ffi::c_void;
use std::time::Duration;

use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, Interface};

use super::Shot;
use crate::settings::WindowBackground;

/// An on-screen app window. `frame` is in global logical units, the window's visible bounds
/// over the scale of the monitor it is mostly on.
#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub id: u32,
    pub app: String,
    pub title: String,
    pub frame: (f32, f32, f32, f32),
}

/// Titled, visible app windows, front to back, without Kiri's own.
pub fn list_windows() -> Vec<WindowInfo> {
    let mut found: Vec<HWND> = Vec::new();
    unsafe extern "system" fn each(hwnd: HWND, found: LPARAM) -> BOOL {
        unsafe { (*(found.0 as *mut Vec<HWND>)).push(hwnd) };
        true.into()
    }
    let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut found as *mut _ as isize)) };
    found.into_iter().filter_map(info).collect()
}

/// The foreground window, or the frontmost one when that isn't capturable.
pub fn frontmost_window() -> Option<WindowInfo> {
    let front = unsafe { GetForegroundWindow() }.0 as usize as u32;
    let mut windows = list_windows();
    let i = windows.iter().position(|w| w.id == front).unwrap_or(0);
    (i < windows.len()).then(|| windows.swap_remove(i))
}

fn info(hwnd: HWND) -> Option<WindowInfo> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let ex = WINDOW_EX_STYLE(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32);
        if ex.contains(WS_EX_TOOLWINDOW) || ex.contains(WS_EX_TRANSPARENT) {
            return None;
        }
        // Store apps that aren't running and windows on other virtual desktops are cloaked.
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4);
        if cloaked != 0 {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            return None;
        }
        let mut title = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title).max(0) as usize;
        let title = String::from_utf16_lossy(&title[..len]);
        if title.is_empty() {
            return None;
        }
        let r = frame(hwnd)?;
        let scale = super::monitor_rect(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST).0 as usize as u32)?.1;
        let (w, h) = ((r.right - r.left) as f32 / scale, (r.bottom - r.top) as f32 / scale);
        // Smaller than this is an invisible helper or a sliver, not something to capture.
        if w < 24. || h < 24. {
            return None;
        }
        Some(WindowInfo {
            id: hwnd.0 as usize as u32,
            app: app_name(pid).unwrap_or_default(),
            title,
            frame: (r.left as f32 / scale, r.top as f32 / scale, w, h),
        })
    }
}

/// Visible bounds in physical pixels, without the invisible resize borders.
fn frame(hwnd: HWND) -> Option<RECT> {
    let mut r = RECT::default();
    unsafe { DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, (&mut r as *mut RECT).cast(), size_of::<RECT>() as u32) }
        .ok()?;
    (r.right > r.left && r.bottom > r.top).then_some(r)
}

fn app_name(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut path = [0u16; 1024];
        let mut len = path.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, windows::core::PWSTR(path.as_mut_ptr()), &mut len);
        let _ = windows::Win32::Foundation::CloseHandle(process);
        ok.ok()?;
        let path = std::path::PathBuf::from(String::from_utf16_lossy(&path[..len as usize]));
        Some(path.file_stem()?.to_string_lossy().into_owned())
    }
}

/// Captures one window, even if covered. Windows draws window shadows outside the window,
/// where capture doesn't reach, so the backgrounds only show through the rounded corners.
pub async fn capture(id: u32, background: WindowBackground, color: u32) -> Option<Shot> {
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(shot(HWND(id as usize as *mut c_void), background, color));
    });
    rx.await.ok().flatten()
}

fn shot(hwnd: HWND, background: WindowBackground, color: u32) -> Option<Shot> {
    let display = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) }.0 as usize as u32;
    let (m, scale) = super::monitor_rect(display)?;
    let (width, height, mut bgra) = grab(hwnd).inspect_err(|e| eprintln!("kiri: window capture failed: {e}")).ok()?;
    let under = match background {
        WindowBackground::Solid => Some(bgr(color).iter().chain(&[255]).copied().cycle().take(bgra.len()).collect()),
        WindowBackground::Wallpaper => {
            let wall = wallpaper(display)?;
            let at = frame(hwnd).map_or((0, 0), |r| (r.left - m.left, r.top - m.top));
            let (ww, wh) = (wall.width as i32, wall.height as i32);
            Some(
                (0..height as i32)
                    .flat_map(|y| (0..width as i32).map(move |x| (x, y)))
                    .flat_map(|(x, y)| {
                        let i = (((at.1 + y).clamp(0, wh - 1) * ww + (at.0 + x).clamp(0, ww - 1)) * 4) as usize;
                        [wall.bgra[i], wall.bgra[i + 1], wall.bgra[i + 2], 255]
                    })
                    .collect::<Vec<u8>>(),
            )
        }
        _ => None,
    };
    if let Some(under) = under {
        for (p, u) in bgra.chunks_exact_mut(4).zip(under.chunks_exact(4)) {
            let clear = 255 - p[3] as u32;
            for (c, u) in p.iter_mut().zip(u) {
                *c = (*c as u32 + *u as u32 * clear / 255).min(255) as u8;
            }
        }
    }
    Some(Shot { display, width, height, scale, bgra })
}

fn bgr(rgb: u32) -> [u8; 3] {
    [rgb, rgb >> 8, rgb >> 16].map(|c| (c & 255) as u8)
}

/// One frame of the window as premultiplied BGRA.
fn grab(hwnd: HWND) -> windows::core::Result<(u32, u32, Vec<u8>)> {
    unsafe {
        let mut device = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )?;
        let device = device.ok_or_else(windows::core::Error::empty)?;
        let winrt: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&device.cast::<IDXGIDevice>()?)?.cast()?;
        let item: GraphicsCaptureItem =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?.CreateForWindow(hwnd)?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            item.Size()?,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        // Without this, Windows 11 outlines the window in yellow while it is captured.
        let _ = session.SetIsBorderRequired(false);
        let (tx, rx) = std::sync::mpsc::channel();
        pool.FrameArrived(&TypedEventHandler::new(move |pool: windows::core::Ref<Direct3D11CaptureFramePool>, _| {
            if let Some(pool) = pool.as_ref() {
                let _ = tx.send(pool.TryGetNextFrame()?);
            }
            Ok(())
        }))?;
        session.StartCapture()?;
        let frame = rx.recv_timeout(Duration::from_secs(2));
        let _ = session.Close();
        let _ = pool.Close();
        let frame = frame.map_err(|_| windows::core::Error::new(windows::core::HRESULT(0x8000_000Eu32 as i32), "no frame"))?;
        let size = frame.ContentSize()?;
        let texture: ID3D11Texture2D = frame.Surface()?.cast::<IDirect3DDxgiInterfaceAccess>()?.GetInterface()?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut staging = None;
        device.CreateTexture2D(&desc, None, Some(&mut staging))?;
        let staging = staging.ok_or_else(windows::core::Error::empty)?;
        let context = device.GetImmediateContext()?;
        context.CopyResource(&staging, &texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        // The window may have shrunk since the pool was sized.
        let (w, h) = ((size.Width as u32).min(desc.Width), (size.Height as u32).min(desc.Height));
        let mut bgra = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let row = std::slice::from_raw_parts((mapped.pData as *const u8).add(y * mapped.RowPitch as usize), w as usize * 4);
            bgra.extend_from_slice(row);
        }
        context.Unmap(&staging, 0);
        Ok((w, h, bgra))
    }
}

/// The desktop picture of the display under the pointer, whole and at its pixel size.
pub async fn desktop_picture() -> Option<Shot> {
    let display = super::display_under_cursor();
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(wallpaper(display));
    });
    rx.await.ok().flatten()
}

/// The wallpaper laid out on `display` as Windows does. Span is laid out per display.
fn wallpaper(display: u32) -> Option<Shot> {
    use tiny_skia::*;
    let (m, scale) = super::monitor_rect(display)?;
    let (w, h) = ((m.right - m.left) as u32, (m.bottom - m.top) as u32);
    let mut canvas = Pixmap::new(w, h)?;
    let [b, g, r] = bgr(desktop_color());
    canvas.fill(Color::from_rgba8(r, g, b, 255));
    if let Some(image) = wallpaper_image() {
        let (iw, ih) = (image.width() as f32, image.height() as f32);
        let (sx, sy) = (w as f32 / iw, h as f32 / ih);
        let style = desktop_value("WallpaperStyle");
        let (kx, ky) = match style.as_str() {
            "2" => (sx, sy),
            "6" => (sx.min(sy), sx.min(sy)),
            "0" => (1., 1.),
            _ => (sx.max(sy), sx.max(sy)),
        };
        if style == "0" && desktop_value("TileWallpaper") == "1" {
            let paint = Paint {
                shader: Pattern::new(image.as_ref(), SpreadMode::Repeat, FilterQuality::Nearest, 1., Transform::identity()),
                ..Default::default()
            };
            canvas.fill_rect(Rect::from_xywh(0., 0., w as f32, h as f32)?, &paint, Transform::identity(), None);
        } else {
            let at = Transform::from_row(kx, 0., 0., ky, (w as f32 - iw * kx) / 2., (h as f32 - ih * ky) / 2.);
            let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() };
            canvas.draw_pixmap(0, 0, image.as_ref(), &paint, at, None);
        }
    }
    let mut bgra = canvas.take();
    for p in bgra.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    Some(Shot { display, width: w, height: h, scale, bgra })
}

/// The desktop's background color, which shows where the wallpaper doesn't reach.
fn desktop_color() -> u32 {
    let c = unsafe { windows::Win32::Graphics::Gdi::GetSysColor(windows::Win32::Graphics::Gdi::COLOR_DESKTOP) };
    (c & 255) << 16 | (c & 0xFF00) | (c >> 16 & 255)
}

fn wallpaper_image() -> Option<tiny_skia::Pixmap> {
    let mut path = [0u16; 1024];
    unsafe {
        SystemParametersInfoW(SPI_GETDESKWALLPAPER, path.len() as u32, Some(path.as_mut_ptr().cast()), Default::default())
    }
    .ok()?;
    let path = String::from_utf16_lossy(&path[..path.iter().position(|&c| c == 0)?]);
    let (w, h, mut rgba, _) = crate::export::decode_rgba(&std::fs::read(path).ok()?)?;
    for p in rgba.chunks_exact_mut(4) {
        let a = p[3] as u32;
        for c in &mut p[..3] {
            *c = (*c as u32 * a / 255) as u8;
        }
    }
    tiny_skia::Pixmap::from_vec(rgba, tiny_skia::IntSize::from_wh(w, h)?)
}

fn desktop_value(name: &str) -> String {
    use windows::Win32::System::Registry::*;
    let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
    let mut value = [0u16; 16];
    let mut size = size_of_val(&value) as u32;
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!(r"Control Panel\Desktop"),
            windows::core::PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if read.is_err() {
        return String::new();
    }
    String::from_utf16_lossy(&value[..value.iter().position(|&c| c == 0).unwrap_or(0)])
}
