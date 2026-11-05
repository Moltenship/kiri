mod alloc;
mod backdrop;
mod doc;
mod editor;
mod export;
mod mac;
mod ocr;
mod overlay;
mod paint;
mod pin;
mod pixels;
mod preview;
mod scrolling;
mod settings;
mod settings_window;
mod single;
mod stitch;
mod text_edit;
mod tiled;
mod toast;
mod update;
mod window_capture;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::channel::mpsc;
use global_hotkey::hotkey::{Code, HotKey};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui_kit::component::Theme;
use gpui_kit::*;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::settings::Capture;
use crate::tiled::TiledImage;

gpui_kit::assets::icon_assets!(
    AppIcons,
    [
        ArrowLeftRight,
        Camera,
        Check,
        Circle,
        CircleDot,
        Command,
        Copy,
        Crop,
        Download,
        Eraser,
        Flashlight,
        Grid3x3,
        GripVertical,
        Hand,
        Highlighter,
        Image,
        Maximize2,
        Minimize2,
        MoveUpRight,
        Pen,
        Pencil,
        Pin,
        Redo2,
        Ruler,
        ScanLine,
        ScanText,
        Search,
        Settings,
        Shuffle,
        Slash,
        Square,
        SquareDashedMousePointer,
        Type,
        Undo2,
        Wrench,
        X
    ]
);

#[global_allocator]
static ALLOCATOR: alloc::PixelAwareAlloc = alloc::PixelAwareAlloc;

struct KiriAssets;

impl AssetSource for KiriAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match AppIcons.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut all = gpui_kit::assets::Assets.list(path)?;
        all.extend(AppIcons.list(path)?);
        Ok(all)
    }
}

enum Command {
    Capture(Capture, Instant),
    CheckForUpdates,
    Cancel,
    OpenFile,
    Open(PathBuf, bool),
    Paste,
    CloseAllPins,
    StressTest,
    /// A capture asked for by `kiri --capture`, which any local process can run.
    Automation(Capture, Option<single::Then>),
    ToggleLaunchAtLogin,
    Menu(String),
    Quit,
}

/// Tray and hotkeys stop working when these are dropped.
struct Resident {
    tray: TrayIcon,
    hotkeys: GlobalHotKeyManager,
    registered: Vec<HotKey>,
    bound: Arc<Mutex<HashMap<u32, Capture>>>,
    escape: HotKey,
}

impl Global for Resident {}

/// A focused overlay can still miss `Esc` while AppKit finishes closing the
/// previous one, so `Esc` is a global hotkey for exactly as long as an overlay is up.
pub fn grab_escape(on: bool, cx: &App) {
    let resident = cx.global::<Resident>();
    let result = if on {
        resident.hotkeys.register(resident.escape)
    } else {
        resident.hotkeys.unregister(resident.escape)
    };
    if let Err(err) = result {
        eprintln!("kiri: Esc hotkey: {err}");
    }
}

pub fn suspend_hotkeys(cx: &mut App) {
    let resident = cx.global_mut::<Resident>();
    let _ = resident.hotkeys.unregister_all(&resident.registered);
    resident.registered.clear();
    resident.bound.lock().unwrap().clear();
}

/// Re-reads the capture hotkeys from settings. Returns the ones the system refused.
pub fn reload_hotkeys(cx: &mut App) -> Vec<String> {
    let wanted: Vec<(Capture, String)> = Capture::ALL
        .iter()
        .filter_map(|&c| settings::get(cx).hotkey(c).map(|k| (c, k)))
        .collect();
    let resident = cx.global_mut::<Resident>();
    let _ = resident.hotkeys.unregister_all(&resident.registered);
    resident.registered.clear();
    let mut bound = resident.bound.lock().unwrap();
    bound.clear();
    let mut failed = Vec::new();
    for (capture, keys) in wanted {
        let Ok(hotkey) = keys.parse::<HotKey>() else {
            failed.push(keys);
            continue;
        };
        match resident.hotkeys.register(hotkey) {
            Ok(()) => {
                bound.insert(hotkey.id(), capture);
                resident.registered.push(hotkey);
            }
            Err(_) => failed.push(keys),
        }
    }
    failed
}

fn load_theme(cx: &mut App) {
    let set: gpui_kit::component::theme::ThemeSet =
        serde_json::from_str(include_str!("theme.json")).expect("theme.json is valid");
    let theme = Theme::global_mut(cx);
    for config in set.themes {
        let config = std::rc::Rc::new(config);
        if config.mode.is_dark() {
            theme.dark_theme = config;
        } else {
            theme.light_theme = config;
        }
    }
    Theme::sync_system_appearance(None, cx);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let (open, demo, thumbnail) = (arg("--open"), arg("--demo"), args.iter().any(|a| a == "--thumbnail"));
    let capture = match arg("--capture") {
        Some(name) => match Capture::from_cli(&name) {
            Some(c) => Some(c),
            None => {
                let names: Vec<String> = Capture::ALL.into_iter().map(Capture::cli_name).collect();
                eprintln!("kiri: unknown capture {name:?}; one of: {}", names.join(", "));
                std::process::exit(2);
            }
        },
        None => None,
    };
    let then = arg("--then").map(|list| {
        single::Then::parse(&list).unwrap_or_else(|word| {
            eprintln!("kiri: unknown --then {word:?}; a list of: {}", single::Then::WORDS.join(", "));
            std::process::exit(2);
        })
    });
    let capture = capture.map(|c| (c, then));
    if demo.is_none() && single::forward(open.as_deref().map(std::path::Path::new), capture) {
        return;
    }
    gpui_kit::application().with_assets(KiriAssets).run(move |cx| {
        gpui_kit::init(cx);
        load_theme(cx);
        cx.set_activation_policy(ActivationPolicy::Accessory);
        settings::load(cx);
        editor::bind_keys(cx);
        pin::bind_keys(cx);
        preview::bind_keys(cx);
        settings_window::bind_keys(cx);

        let (tx, mut rx) = mpsc::unbounded();
        // `--demo <capture>` or `--open <file>` runs one command at launch, for scripted checks.
        if let Some(file) = open {
            let _ = tx.unbounded_send(Command::Open(file.into(), thumbnail));
        }
        let demo = match demo.as_deref() {
            Some("area") => Some(Command::Capture(Capture::Area, Instant::now())),
            Some("screen") => Some(Command::Capture(Capture::Fullscreen, Instant::now())),
            Some("window") => Some(Command::Capture(Capture::Window, Instant::now())),
            Some("settings") => Some(Command::Capture(Capture::Settings, Instant::now())),
            Some("stress") => Some(Command::StressTest),
            _ => None,
        };
        if let Some(demo) = demo {
            let _ = tx.unbounded_send(demo);
        }
        if let Some((capture, then)) = capture {
            let _ = tx.unbounded_send(Command::Automation(capture, then));
        }
        single::listen(tx.clone());
        let resident = install_resident(tx);
        cx.set_global(resident);
        for keys in reload_hotkeys(cx) {
            eprintln!("kiri: couldn't register {keys}");
        }
        refresh_tray(cx);
        ocr::warm_up(cx);
        update::start(cx);

        cx.spawn(async move |cx| {
            while let Some(command) = rx.next().await {
                cx.update(|cx| handle(command, cx));
            }
        })
        .detach();
        eprintln!("kiri: ready");
    });
}

fn install_resident(tx: mpsc::UnboundedSender<Command>) -> Resident {
    let hotkeys = GlobalHotKeyManager::new().expect("hotkey manager");
    let escape = HotKey::new(None, Code::Escape);
    let escape_id = escape.id();
    let bound: Arc<Mutex<HashMap<u32, Capture>>> = Arc::default();
    let hotkey_tx = tx.clone();
    let lookup = bound.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state != HotKeyState::Pressed {
            return;
        }
        let command = if event.id == escape_id {
            Command::Cancel
        } else {
            match lookup.lock().unwrap().get(&event.id) {
                Some(&capture) => Command::Capture(capture, Instant::now()),
                None => return,
            }
        };
        let _ = hotkey_tx.unbounded_send(command);
    }));

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let id = event.id.0.as_str();
        let command = match id {
            "open" => Command::OpenFile,
            "paste" => Command::Paste,
            "pins" => Command::CloseAllPins,
            "login" => Command::ToggleLaunchAtLogin,
            "quit" => Command::Quit,
            "update" => Command::CheckForUpdates,
            _ => match Capture::ALL.iter().find(|c| format!("{c:?}") == id) {
                Some(&capture) => Command::Capture(capture, Instant::now()),
                None => Command::Menu(id.to_string()),
            },
        };
        let _ = tx.unbounded_send(command);
    }));

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu(&settings::Settings::default(), None)))
        .with_icon_templated(tray_glyph())
        .with_tooltip("Kiri")
        .build()
        .expect("tray icon");

    Resident {
        tray,
        hotkeys,
        registered: Vec::new(),
        bound,
        escape,
    }
}

/// Same order as the alternative: the frequent captures on top, the rest under "More".
fn tray_menu(s: &settings::Settings, ready: Option<&str>) -> Menu {
    let update_label = ready.map_or("Check for Updates…".to_string(), |v| format!("Restart to Update to {v}"));
    let item = |c: Capture| {
        let accel = s.hotkey(c).and_then(|k| k.parse().ok());
        MenuItem::with_id(format!("{c:?}"), c.label(), true, accel)
    };
    let more = Submenu::new("More", true);
    let delayed = MenuItem::with_id(
        format!("{:?}", Capture::Delayed),
        format!("Delayed Screenshot ({}s)", s.delay_seconds),
        true,
        s.hotkey(Capture::Delayed).and_then(|k| k.parse().ok()),
    );
    let delayed_area = MenuItem::with_id(
        format!("{:?}", Capture::DelayedArea),
        format!("Delayed Area Capture ({}s)", s.delay_seconds),
        true,
        s.hotkey(Capture::DelayedArea).and_then(|k| k.parse().ok()),
    );
    let _ = more.append_items(&[
        &delayed,
        &delayed_area,
        &item(Capture::RepeatArea),
        &item(Capture::ActiveWindow),
        &item(Capture::ScrollingUp),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id("open", "Open File…", true, None),
        &MenuItem::with_id("paste", "Load From Clipboard", true, None),
        &MenuItem::with_id("pins", "Close All Pins", true, None),
    ]);
    let menu = Menu::new();
    let _ = menu.append_items(&[
        &item(Capture::Reopen),
        &PredefinedMenuItem::separator(),
        &item(Capture::Area),
        &item(Capture::Fullscreen),
        &item(Capture::Window),
        &item(Capture::Scrolling),
        &item(Capture::Text),
        &more,
        &PredefinedMenuItem::separator(),
        &CheckMenuItem::with_id("login", "Launch at Login", true, settings::launch_at_login(), None),
        &item(Capture::Settings),
        &MenuItem::with_id("update", update_label, true, None),
        &MenuItem::with_id("quit", "Quit Kiri", true, None),
    ]);
    menu
}

pub fn refresh_tray(cx: &mut App) {
    let ready = cx.try_global::<update::Update>().and_then(|u| u.ready.as_ref()).map(|(v, _)| v.clone());
    let menu = tray_menu(settings::get(cx), ready.as_deref());
    cx.global::<Resident>().tray.set_menu(Some(Box::new(menu)));
}

pub fn open_file(cx: &mut App) {
    handle(Command::OpenFile, cx);
}

/// Captures an area into the editor in `editor` instead of opening a new one.
pub fn add_capture(editor: AnyWindowHandle, cx: &mut App) {
    if !mac::ensure_screen_access() {
        toast::show("Kiri needs Screen Recording permission", cx);
        return;
    }
    start_overlay(overlay::Mode::Area, Instant::now(), Some(editor), cx);
}

pub fn open_clipboard(cx: &mut App) {
    handle(Command::Paste, cx);
}

fn handle(command: Command, cx: &mut App) {
    match command {
        Command::Capture(capture, at) => {
            cx.set_global(NextThen(None));
            self::capture(capture, at, cx)
        }
        // Kiri holds the screen recording permission, so another app must not get captures silently.
        Command::Automation(capture, then) if settings::get(cx).automation => {
            cx.set_global(NextThen(then));
            self::capture(capture, Instant::now(), cx)
        }
        Command::Automation(capture, then) => toast::with_action(
            format!("An app asked for {}", capture.label().trim_end_matches('…')),
            "Allow",
            move |cx| {
                settings::update(cx, |s| s.automation = true);
                cx.set_global(NextThen(then));
                self::capture(capture, Instant::now(), cx);
            },
            cx,
        ),
        Command::Cancel => {
            overlay::close_all(cx);
            scrolling::stop(cx);
        }
        Command::OpenFile => {
            let rx = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: None,
            });
            cx.activate(true);
            cx.spawn(async move |cx| {
                if let Ok(Ok(Some(paths))) = rx.await
                    && let Some(path) = paths.into_iter().next()
                {
                    cx.update(|cx| handle(Command::Open(path, false), cx));
                }
            })
            .detach();
        }
        Command::Open(path, thumbnail) => match std::fs::read(&path).ok().and_then(|b| export::decode(&b)) {
            Some((image, scale)) if thumbnail => preview::open(image, scale, None, None, cx),
            Some((image, scale)) => editor::open(image, scale, None, Some(path), cx),
            None => toast::show("Couldn't open that image", cx),
        },
        Command::Paste => {
            let image = cx.read_from_clipboard().and_then(|item| {
                // gpui reports a copied file (Kiri's own copy carries one) as paths, without its image.
                item.entries().iter().find_map(|e| match e {
                    ClipboardEntry::Image(img) => export::decode(img.bytes()),
                    ClipboardEntry::ExternalPaths(paths) => {
                        paths.paths().iter().find_map(|p| std::fs::read(p).ok().and_then(|b| export::decode(&b)))
                    }
                    _ => None,
                })
            });
            match image {
                Some((image, scale)) => editor::open(image, scale, None, None, cx),
                None => toast::show("No image on the clipboard", cx),
            }
        }
        Command::CloseAllPins => pin::close_all(cx),
        Command::StressTest => {
            cx.spawn(async move |cx| {
                let image = cx
                    .background_executor()
                    .spawn(async { stress_image(2880, 24_000) })
                    .await;
                cx.update(|cx| editor::open(image, 2., None, None, cx));
            })
            .detach();
        }
        Command::ToggleLaunchAtLogin => {
            if let Err(err) = settings::set_launch_at_login(!settings::launch_at_login()) {
                toast::show(format!("Couldn't change login item: {err}"), cx);
            }
            refresh_tray(cx);
        }
        Command::Menu(id) => {
            pin::menu_event(&id, cx);
        }
        Command::CheckForUpdates => match cx.default_global::<update::Update>().ready.is_some() {
            true => update::restart(cx),
            false => update::check(true, cx),
        },
        Command::Quit => cx.quit(),
    }
}

fn capture(capture: Capture, at: Instant, cx: &mut App) {
    match capture {
        Capture::Settings => return settings_window::open(cx),
        Capture::Reopen => return editor::reopen_last(cx),
        _ => {}
    }
    if !mac::ensure_screen_access() {
        toast::show("Kiri needs Screen Recording permission", cx);
        return;
    }
    match capture {
        Capture::Area | Capture::Text | Capture::Scrolling | Capture::ScrollingUp => {
            let mode = match capture {
                Capture::Text => overlay::Mode::Text,
                Capture::Scrolling => overlay::Mode::Scroll { up: false },
                Capture::ScrollingUp => overlay::Mode::Scroll { up: true },
                _ => overlay::Mode::Area,
            };
            start_overlay(mode, at, None, cx);
        }
        Capture::Window => start_overlay(overlay::Mode::Window, at, None, cx),
        Capture::RepeatArea => match overlay::last_area(cx) {
            Some((display, rect)) => {
                cx.spawn(async move |cx| {
                    if let Some(shot) = mac::capture_display(display).await {
                        let image = TiledImage::from_bgra(shot.width, shot.height, &shot.bgra, (shot.width * 4) as usize);
                        drop(shot.bgra);
                        let Some(rect) = rect.clamp_to(image.width, image.height) else { return };
                        let cropped = TiledImage::from_bgra(rect.w, rect.h, &image.read(rect), (rect.w * 4) as usize);
                        cx.update(|cx| deliver(cropped, shot.scale, Some(display), cx));
                    }
                })
                .detach();
            }
            None => {
                toast::show("No area to repeat yet. Select one now", cx);
                start_overlay(overlay::Mode::Area, at, None, cx);
            }
        },
        Capture::Fullscreen => fullscreen(cx),
        Capture::Delayed | Capture::DelayedArea => {
            let seconds = settings::get(cx).delay_seconds.max(1);
            cx.spawn(async move |cx| {
                for n in (1..=seconds).rev() {
                    cx.update(|cx| toast::countdown(n, cx));
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                }
                cx.update(|cx| toast::hide(cx));
                // Lets the window server drop the countdown before the frame is taken.
                cx.background_executor().timer(Duration::from_millis(120)).await;
                match capture {
                    Capture::Delayed => cx.update(fullscreen),
                    _ => cx.update(|cx| start_overlay(overlay::Mode::Area, Instant::now(), None, cx)),
                }
            })
            .detach();
        }
        Capture::ActiveWindow => {
            let (background, color) = window_style(cx);
            cx.spawn(async move |cx| {
                let shot = match window_capture::frontmost_window().await {
                    Some(w) => window_capture::capture(w.id, background, color).await,
                    None => None,
                };
                cx.update(|cx| match shot {
                    Some(shot) => deliver_shot(shot, cx),
                    None => toast::show("No window to capture. Try Capture Screen instead", cx),
                });
            })
            .detach();
        }
        Capture::Settings | Capture::Reopen => {}
    }
}

pub fn window_style(cx: &App) -> (settings::WindowBackground, u32) {
    let s = settings::get(cx);
    (s.window_background, s.window_background_color)
}

fn start_overlay(mode: overlay::Mode, at: Instant, add_to: Option<AnyWindowHandle>, cx: &mut App) {
    let displays: Vec<u32> = cx.displays().iter().map(|d| u64::from(d.id()) as u32).collect();
    let freeze = settings::get(cx).freeze_screen;
    cx.spawn(async move |cx| {
        let windows = match mode {
            overlay::Mode::Window | overlay::Mode::Area => window_capture::list_windows().await,
            _ => Vec::new(),
        };
        let screens: Vec<_> = if freeze {
            let shots = futures::future::join_all(displays.into_iter().map(mac::capture_display)).await;
            let shots: Vec<_> = shots.into_iter().flatten().map(|s| (s.display, Some(s))).collect();
            eprintln!("kiri: captured {} display(s) in {} ms", shots.len(), at.elapsed().as_millis());
            shots
        } else {
            displays.into_iter().map(|d| (d, None)).collect()
        };
        cx.update(|cx| overlay::start(screens, mode, windows, at, add_to, cx));
    })
    .detach();
}

fn fullscreen(cx: &mut App) {
    let display = mac::display_under_cursor();
    let displays: Vec<u32> = match settings::get(cx).fullscreen_all_displays {
        true => cx.displays().iter().map(|d| u64::from(d.id()) as u32).collect(),
        false => vec![display],
    };
    cx.spawn(async move |cx| {
        let shot = if let [one] = displays[..] {
            mac::capture_display(one).await
        } else {
            let shots = futures::future::join_all(displays.into_iter().map(mac::capture_display)).await;
            let Some(shots) = shots.into_iter().collect::<Option<Vec<_>>>() else {
                cx.update(|cx| toast::show("Couldn't capture every display", cx));
                return;
            };
            let placed = shots.into_iter().map(|s| (mac::display_origin(s.display), s)).collect();
            cx.background_executor().spawn(async move { pixels::stitch(placed, display) }).await
        };
        if let Some(shot) = shot {
            cx.update(|cx| deliver_shot(shot, cx));
        }
    })
    .detach();
}

/// `gpui_kit::open_window` for a window that draws its own shape. The theme fills every
/// root, which shows as a rectangle around a rounded toast or the live screen.
pub fn open_clear_window<V: Render>(
    options: WindowOptions,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> gpui_kit::Result<(AnyWindowHandle, Entity<V>)> {
    let opened = gpui_kit::open_window(options, cx, build)?;
    if let Some(root) = opened.0.downcast::<gpui_kit::base::Root>() {
        let _ = root.update(cx, |root, _, _| root.style().refine(&StyleRefinement::default().bg(transparent_black())));
    }
    Ok(opened)
}

pub fn deliver_shot(shot: mac::Shot, cx: &mut App) {
    let image = TiledImage::from_bgra(shot.width, shot.height, &shot.bgra, (shot.width * 4) as usize);
    let (scale, display) = (shot.scale, (shot.display != 0).then_some(shot.display));
    drop(shot.bgra);
    deliver(image, scale, display, cx);
}

/// `--then` for the automated capture in progress. Any other capture clears it, so one
/// that was cancelled can't leak into the next.
#[derive(Default)]
struct NextThen(Option<single::Then>);

impl Global for NextThen {}

/// Where every capture ends: the after-capture settings, or `--then`, decide copy, save, and
/// what opens. Encoding runs off the main thread; the editor opens at once, the thumbnail once
/// the file exists.
pub fn deliver(image: TiledImage, scale: f32, display: Option<u32>, cx: &mut App) {
    let s = settings::get(cx).clone();
    let then = cx.default_global::<NextThen>().0.take().unwrap_or_else(|| {
        let thumbnail = s.after_show && s.area_preview == settings::Preview::Thumbnail;
        single::Then { edit: s.after_show && !thumbnail, thumbnail, copy: s.after_copy, save: s.after_save }
    });
    let thumbnail = then.thumbnail;
    if then.edit {
        editor::open(image.clone(), scale, display, None, cx);
    }
    if !(then.copy || then.save) {
        if thumbnail {
            preview::open(image, scale, display, None, cx);
        }
        return;
    }
    let save_to = then.save.then(|| s.save_path());
    let (auto, copy, edit, name) = (s.save_auto_jpeg, then.copy, then.edit, s.file_name.clone());
    cx.spawn(async move |cx| {
        let shot = image.clone();
        let (png, temp, saved) = cx
            .background_executor()
            .spawn(async move {
                let png = export::encode_bgra(shot.width, shot.height, shot.read(shot.full()), scale);
                let temp = copy.then(|| editor::write_temp(&png, &name)).flatten();
                let saved = save_to.and_then(|path| export::save_capture(&shot, scale, png.clone(), path, auto).ok());
                (png, temp, saved)
            })
            .await;
        cx.update(|cx| {
            if copy {
                mac::copy_png(&png, temp.as_deref());
            }
            if thumbnail {
                preview::open(image, scale, display, saved, cx);
            } else if let Some(path) = saved.filter(|_| !edit) {
                toast::saved(path, cx);
            }
        });
    })
    .detach();
}

/// A tall synthetic page: gradient bands, a 100 px grid, and a ruler stripe.
fn stress_image(w: u32, h: u32) -> TiledImage {
    let mut bgra = vec![0u8; (w * h * 4) as usize];
    for (y, row) in bgra.chunks_exact_mut((w * 4) as usize).enumerate() {
        let y = y as u32;
        let band = (y / 1000) % 6;
        let t = (y % 1000) as f32 / 1000.;
        let base: [f32; 3] = [
            [255., 240., 230.],
            [230., 245., 255.],
            [235., 255., 235.],
            [255., 250., 220.],
            [245., 235., 255.],
            [240., 240., 240.],
        ][band as usize];
        for (x, px) in row.chunks_exact_mut(4).enumerate() {
            let x = x as u32;
            let shade = 1. - 0.12 * t - 0.06 * (x as f32 / w as f32);
            let mut c = base.map(|v| (v * shade) as u8);
            if x % 100 == 0 || y % 100 == 0 {
                c = [190, 195, 205];
            }
            if x < 40 && y % 1000 < 12 {
                c = [30, 30, 30];
            }
            px.copy_from_slice(&[c[2], c[1], c[0], 255]);
        }
    }
    TiledImage::from_bgra(w, h, &bgra, (w * 4) as usize)
}

/// Viewfinder corners, drawn at 2× for the 18 pt menu bar slot.
/// The app icon reduced to a template: a selection with handles around a peak and the sun.
fn tray_glyph() -> tray_icon::Icon {
    use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};
    let n = 36;
    let mut pixmap = Pixmap::new(n, n).expect("icon pixmap");
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    // Drawn in a unit box, y up.
    let unit = Transform::from_row(n as f32, 0., 0., -(n as f32), 0., n as f32);
    let frame = PathBuilder::from_rect(Rect::from_ltrb(0.1, 0.16, 0.9, 0.84).expect("frame"));
    pixmap.stroke_path(&frame, &paint, &Stroke { width: 0.085, ..Stroke::default() }, unit, None);
    let mut pb = PathBuilder::new();
    for (x, y) in [(0.1, 0.16), (0.9, 0.16), (0.1, 0.84), (0.9, 0.84)] {
        pb.push_circle(x, y, 0.09);
    }
    pb.push_circle(0.67, 0.67, 0.07);
    pb.move_to(0.22, 0.27);
    for (x, y) in [(0.44, 0.58), (0.56, 0.43), (0.64, 0.51), (0.8, 0.27)] {
        pb.line_to(x, y);
    }
    pb.close();
    pixmap.fill_path(&pb.finish().expect("icon path"), &paint, FillRule::Winding, unit, None);
    tray_icon::Icon::from_rgba(pixmap.take(), n, n).expect("icon rgba")
}
