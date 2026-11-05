//! The settings window: each control reads and writes `settings` directly.

use std::rc::Rc;

use global_hotkey::hotkey::HotKey;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::settings::{self, Capture, ColorFormat, Preview, WindowBackground};

actions!(settings_window, [CloseSettings]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-w", CloseSettings, Some("SettingsWindow"))]);
}

pub struct Open(pub AnyWindowHandle);

impl Global for Open {}

pub fn open(cx: &mut App) {
    if let Some(handle) = cx.try_global::<Open>().map(|o| o.0)
        && handle.update(cx, |_, window, _| window.activate_window()).is_ok()
    {
        cx.activate(true);
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(820.), px(640.)), cx))),
        titlebar: Some(TitlebarOptions {
            title: Some("Kiri Settings".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(20.), px(20.))),
        }),
        window_background: WindowBackgroundAppearance::Blurred,
        window_min_size: Some(size(px(680.), px(420.))),
        focus: true,
        show: true,
        ..Default::default()
    };
    let opened = gpui_kit::open_window(options, cx, |window, cx| {
        let view = cx.new(|cx| SettingsWindow::new(window, cx));
        window.focus(&view.read(cx).focus.clone(), cx);
        crate::mac::set_on_top(window, settings::get(cx).editor_on_top);
        view
    });
    if let Ok((handle, _)) = opened {
        cx.set_global(Open(handle));
    }
    cx.activate(true);
}

pub struct SettingsWindow {
    focus: FocusHandle,
    page: usize,
    search: Entity<InputState>,
    solid: Entity<ColorPickerState>,
    _subscriptions: [Subscription; 4],
}

impl SettingsWindow {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings…"));
        let color = settings::get(cx).window_background_color;
        let solid = cx.new(|cx| ColorPickerState::new(window, cx).default_value(rgb(color)));
        let subscriptions = [
            // Closing the window mid-recording skips the recorder's focus-out, which turns them back on.
            cx.on_release(|_, cx| {
                crate::reload_hotkeys(cx);
            }),
            cx.observe_global::<settings::Current>(|_, cx| cx.notify()),
            cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe(&solid, |_, _, e: &ColorPickerEvent, cx| {
                if let ColorPickerEvent::Change(Some(c)) = e {
                    let c = c.to_rgb();
                    let ch = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
                    settings::update(cx, |s| s.window_background_color = ch(c.r) << 16 | ch(c.g) << 8 | ch(c.b));
                }
            }),
        ];
        Self { focus: cx.focus_handle(), page: 0, search, solid, _subscriptions: subscriptions }
    }
}

struct Page {
    title: &'static str,
    icon: IconName,
    groups: Vec<Group>,
}

struct Group {
    title: Option<&'static str>,
    description: Option<&'static str>,
    rows: Vec<Row>,
}

struct Row {
    title: SharedString,
    description: Option<SharedString>,
    control: AnyElement,
    disabled: bool,
}

fn group(title: impl Into<Option<&'static str>>, rows: impl IntoIterator<Item = Row>) -> Group {
    Group { title: title.into(), description: None, rows: rows.into_iter().collect() }
}

fn row(title: impl Into<SharedString>, control: impl IntoElement) -> Row {
    Row { title: title.into(), description: None, control: control.into_any_element(), disabled: false }
}

impl Row {
    fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

fn toggle(id: &'static str, get: fn(&settings::Settings) -> bool, set: fn(&mut settings::Settings, bool), cx: &App) -> AnyElement {
    Switch::new(id)
        .checked(get(settings::get(cx)))
        .on_click(move |v, _, cx| settings::update(cx, |s| set(s, *v)))
        .into_any_element()
}

fn dropdown(id: impl Into<ElementId>, labels: Vec<SharedString>, selected: Option<usize>, pick: impl Fn(usize, &mut App) + 'static) -> AnyElement {
    let pick = Rc::new(pick);
    let label = selected.and_then(|i| labels.get(i).cloned()).unwrap_or_else(|| "—".into());
    Button::new(id)
        .label(label)
        .ghost()
        .small()
        .dropdown_caret(true)
        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
            let scrollable = labels.len() > 12;
            labels
                .iter()
                .enumerate()
                .fold(menu, |menu, (i, label)| {
                    let pick = pick.clone();
                    menu.item(PopupMenuItem::new(label.clone()).checked(selected == Some(i)).on_click(move |_, _, cx| pick(i, cx)))
                })
                .scrollable(scrollable)
        })
        .into_any_element()
}

fn choice<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: &'static [(T, &'static str)],
    get: fn(&settings::Settings) -> T,
    set: fn(&mut settings::Settings, T),
    cx: &App,
) -> AnyElement {
    let now = get(settings::get(cx));
    dropdown(
        id,
        options.iter().map(|(_, l)| SharedString::from(*l)).collect(),
        options.iter().position(|(o, _)| *o == now),
        move |i, cx| settings::update(cx, |s| set(s, options[i].0)),
    )
}

/// Presets for a number, keeping a value set by hand in the settings file.
fn preset(
    id: &'static str,
    presets: &[u32],
    unit: impl Fn(u32) -> String,
    get: fn(&settings::Settings) -> u32,
    set: fn(&mut settings::Settings, u32),
    cx: &App,
) -> AnyElement {
    let now = get(settings::get(cx));
    let mut values = presets.to_vec();
    if !values.contains(&now) {
        values.push(now);
        values.sort();
    }
    dropdown(
        id,
        values.iter().map(|&v| unit(v).into()).collect(),
        values.iter().position(|&v| v == now),
        move |i, cx| settings::update(cx, |s| set(s, values[i])),
    )
}

fn button(id: &'static str, label: &'static str, on_click: impl Fn(&mut App) + 'static) -> Button {
    Button::new(id).label(label).outline().small().on_click(move |_, _, cx| on_click(cx))
}

const SAVE_FORMATS: &[(bool, &str)] = &[(false, "PNG"), (true, "Auto (PNG or JPEG)")];
const PREVIEWS: &[(Preview, &str)] = &[(Preview::Editor, "Editor"), (Preview::Thumbnail, "Thumbnail")];
const HIDE_AFTER: &[(u32, &str)] = &[(0, "Manually"), (3, "After 3 seconds"), (5, "After 5 seconds"), (10, "After 10 seconds")];
const BACKGROUNDS: &[(WindowBackground, &str)] = &[
    (WindowBackground::Transparent, "Transparent"),
    (WindowBackground::Wallpaper, "Wallpaper"),
    (WindowBackground::Solid, "Solid color"),
    (WindowBackground::TrimShadow, "Trim shadow"),
];
const CMD_Q: &[(settings::CmdQ, &str)] = &[
    (settings::CmdQ::Ask, "Ask"),
    (settings::CmdQ::Quit, "Quits Kiri"),
    (settings::CmdQ::CloseWindow, "Closes the window"),
];

fn general(cx: &App) -> Page {
    let now = settings::get(cx);
    let dir = now.save_dir();
    let dir = match std::env::var_os("HOME").and_then(|home| dir.strip_prefix(home).ok().map(|p| p.to_path_buf())) {
        Some(rest) => format!("~/{}", rest.display()),
        None => dir.display().to_string(),
    };
    let folder = button("folder", "Choose…", |cx| {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose".into()) });
        cx.spawn(async move |cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(dir) = paths.into_iter().next()
            {
                cx.update(|cx| settings::update(cx, |s| s.save_folder = Some(dir)));
            }
        })
        .detach();
    });
    let login = Switch::new("login").checked(settings::launch_at_login()).on_click(|v, window, cx| {
        if let Err(err) = settings::set_launch_at_login(*v) {
            crate::toast::show(format!("Couldn't change login item: {err}"), cx);
        }
        crate::refresh_tray(cx);
        window.refresh();
    });
    Page {
        title: "General",
        icon: IconName::Settings,
        groups: vec![
            group(None, [row("Launch at login", login)]),
            group(
                "After capture",
                [
                    row("Show", toggle("show", |s| s.after_show, |s, v| s.after_show = v, cx)),
                    row("Copy to clipboard", toggle("copy", |s| s.after_copy, |s, v| s.after_copy = v, cx)),
                    row("Save to folder", toggle("save", |s| s.after_save, |s, v| s.after_save = v, cx)),
                    row("Show in", choice("show-in", PREVIEWS, |s| s.area_preview, |s, v| s.area_preview = v, cx))
                        .description("The thumbnail is a small card with Copy, Save, Edit, and Pin."),
                    row("Hide thumbnail", choice("hide", HIDE_AFTER, |s| s.thumbnail_seconds, |s, v| s.thumbnail_seconds = v, cx))
                        .disabled(now.area_preview != Preview::Thumbnail),
                    row("⌘Q in Kiri's windows", choice("cmd-q", CMD_Q, |s| s.cmd_q, |s, v| s.cmd_q = v, cx)),
                    row("Confirm saving", toggle("confirm", |s| s.confirmations, |s, v| s.confirmations = v, cx))
                        .description("A short toast with the folder after saving. Errors always show."),
                ],
            ),
            group(
                "Saving",
                [
                    row("Save folder", folder).description(dir),
                    row("Save format", choice("format", SAVE_FORMATS, |s| s.save_auto_jpeg, |s, v| s.save_auto_jpeg = v, cx))
                        .description("Auto saves a JPEG when it is smaller, which suits photos. Transparent images stay PNG."),
                    row("Downscale Retina screenshots", toggle("downscale", |s| s.downscale_retina, |s, v| s.downscale_retina = v, cx))
                        .description("Copy and save at one pixel per point."),
                ],
            ),
        ],
    }
}

fn capture(solid: &Entity<ColorPickerState>, cx: &App) -> Page {
    let now = settings::get(cx);
    Page {
        title: "Capture",
        icon: IconName::Camera,
        groups: vec![
            group(
                "Area",
                [row("Freeze the screen while selecting", toggle("freeze", |s| s.freeze_screen, |s, v| s.freeze_screen = v, cx))
                    .description("Selects on a still of the moment you pressed the hotkey, so menus and hover states stay put.")],
            ),
            group(
                "Windows",
                [
                    row("Window background", choice("background", BACKGROUNDS, |s| s.window_background, |s, v| s.window_background = v, cx)),
                    row("Solid color", ColorPicker::new(solid).small().anchor(Anchor::TopRight))
                        .disabled(now.window_background != WindowBackground::Solid),
                ],
            ),
            group(
                "Delayed screenshot",
                [row(
                    "Delay",
                    preset("delay", &[1, 2, 3, 5, 10, 15, 30, 60], |v| format!("{v} s"), |s| s.delay_seconds, |s, v| s.delay_seconds = v, cx),
                )],
            ),
            group(
                "Scrolling capture",
                [
                    row(
                        "Max height",
                        preset(
                            "max-height",
                            &[10_000, 20_000, 50_000, 100_000],
                            |v| format!("{} px", v.to_string().as_bytes().rchunks(3).rev().map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join(" ")),
                            |s| s.scroll_max_height,
                            |s, v| s.scroll_max_height = v,
                            cx,
                        ),
                    ),
                    row("Scroll by hand", toggle("manual", |s| s.scroll_manual, |s, v| s.scroll_manual = v, cx))
                        .description("Kiri stitches while you scroll, for apps that ignore automatic scrolling. Doesn't need Accessibility."),
                    row("Reverse scroll direction", toggle("reverse", |s| s.scroll_reverse, |s, v| s.scroll_reverse = v, cx))
                        .description("For apps like Mos or Scroll Reverser that invert the wheel."),
                ],
            ),
        ],
    }
}

fn editor(cx: &App) -> Page {
    let on_top = Switch::new("on-top").checked(settings::get(cx).editor_on_top).on_click(|v, _, cx| {
        settings::update(cx, |s| s.editor_on_top = *v);
        // Deferred: this window is mid-update, and it changes level too.
        cx.defer(crate::editor::apply_on_top);
    });
    Page {
        title: "Editor",
        icon: IconName::Pen,
        groups: vec![
            group(
                "Esc closes the editor and",
                [
                    row("Copies the image", toggle("esc-copy", |s| s.esc_copies, |s, v| s.esc_copies = v, cx)),
                    row("Saves the image", toggle("esc-save", |s| s.esc_saves, |s, v| s.esc_saves = v, cx)),
                ],
            ),
            group("Window", [row("Always on top", on_top)]),
            group(
                "Canvas",
                [
                    row("Measure in pixels", toggle("unit", |s| s.unit_px, |s, v| s.unit_px = v, cx))
                        .description("Points by default. P toggles it in the editor."),
                    row("Reverse wheel zoom", toggle("wheel", |s| s.reverse_wheel_zoom, |s, v| s.reverse_wheel_zoom = v, cx)),
                    row("Color format", choice("color-format", ColorFormat::OPTIONS, |s| s.color_format, |s, v| s.color_format = v, cx))
                        .description("What Tab copies from the color under the cursor."),
                ],
            ),
        ],
    }
}

fn hotkeys(window: &mut Window, cx: &mut App) -> Page {
    let system = h_flex()
        .gap_2()
        .child(button("keyboard", "Open Keyboard Shortcuts", |cx| cx.open_url(KEYBOARD_SETTINGS)))
        .child(button("system-keys", "Use ⇧⌘3 and ⇧⌘4", |cx| {
            settings::update(cx, |s| {
                s.hotkeys.insert(Capture::Fullscreen, Some("shift+cmd+3".into()));
                s.hotkeys.insert(Capture::Area, Some("shift+cmd+4".into()));
            });
            for keys in crate::reload_hotkeys(cx) {
                crate::toast::show(format!("{} is taken by another app", pretty(&keys)), cx);
            }
            crate::refresh_tray(cx);
        }));
    let keys = Capture::ALL.into_iter().map(|capture| row(capture.label(), recorder(capture, window, cx))).collect::<Vec<_>>();
    Page {
        title: "Hotkeys",
        icon: IconName::Command,
        groups: vec![
            Group {
                description: Some("Click a shortcut, then press the new keys. Backspace clears it, Esc cancels."),
                ..group(None, keys)
            },
            group(
                "System shortcuts",
                [row("Screenshots in macOS", system)
                    .description("To use ⇧⌘3 and ⇧⌘4, turn them off in Keyboard Shortcuts › Screenshots first. macOS keeps them otherwise.")],
            ),
        ],
    }
}

const KEYBOARD_SETTINGS: &str = "x-apple.systempreferences:com.apple.Keyboard-Settings.extension";

struct Recorder {
    focus: FocusHandle,
    _restore: Subscription,
}

/// Global hotkeys are off while recording, so pressing a bound shortcut records it
/// instead of starting a capture.
fn recorder(capture: Capture, window: &mut Window, cx: &mut App) -> AnyElement {
    let state = window.use_keyed_state(("recorder", capture as usize), cx, |window, cx| {
        let focus = cx.focus_handle();
        let restore = cx.on_focus_out(&focus, window, move |_, _, _, cx| {
            for keys in crate::reload_hotkeys(cx) {
                crate::toast::show(format!("{} is taken by another app", pretty(&keys)), cx);
            }
            // macOS keeps these even when Kiri registers them, so the system screenshot UI wins.
            if let Some(keys) = settings::get(cx).hotkey(capture)
                && ["shift+cmd+3", "shift+cmd+4", "shift+cmd+5"].contains(&keys.as_str())
            {
                crate::toast::with_action(
                    format!("Turn off {} in Keyboard Shortcuts › Screenshots", pretty(&keys)),
                    "Open",
                    |cx| cx.open_url(KEYBOARD_SETTINGS),
                    cx,
                );
            }
            crate::refresh_tray(cx);
        });
        Recorder { focus, _restore: restore }
    });
    let focus = state.read(cx).focus.clone();
    let recording = focus.is_focused(window);
    let keys = settings::get(cx).hotkey(capture);
    let label = match &keys {
        _ if recording => "Press keys…".to_string(),
        Some(keys) => pretty(keys),
        None => "Record".to_string(),
    };
    let theme = cx.theme();
    div()
        .id(("hotkey", capture as usize))
        .track_focus(&focus)
        .h(px(28.))
        .min_w(px(64.))
        .px_2p5()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme.radius)
        .border_1()
        .border_color(if recording { theme.ring } else { transparent_black() })
        .bg(if keys.is_some() || recording { theme.secondary } else { transparent_black() })
        .hover(|s| s.bg(theme.secondary_hover))
        .text_sm()
        .text_color(if recording || keys.is_none() { theme.muted_foreground } else { theme.foreground })
        .cursor_pointer()
        .child(label)
        .on_click(move |_, window, cx| {
            crate::suspend_hotkeys(cx);
            focus.focus(window, cx);
        })
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            cx.stop_propagation();
            let k = &e.keystroke;
            let mut m = k.modifiers;
            let done = match k.key.as_str() {
                "escape" => true,
                "backspace" | "delete" if !(m.control || m.alt || m.shift || m.platform) => {
                    settings::update(cx, |s| s.hotkeys.insert(capture, None));
                    true
                }
                key if m.control || m.alt || m.platform || key.starts_with('f') && key.len() > 1 => {
                    let key = match unshift(key) {
                        Some(key) => {
                            m.shift = true;
                            key
                        }
                        None => key,
                    };
                    let mut parts = Vec::new();
                    for (on, name) in [(m.control, "ctrl"), (m.alt, "alt"), (m.shift, "shift"), (m.platform, "cmd")] {
                        if on {
                            parts.push(name);
                        }
                    }
                    parts.push(key);
                    let keys = parts.join("+");
                    if keys.parse::<HotKey>().is_err() {
                        crate::toast::show(format!("{} can't be a global shortcut", pretty(&keys)), cx);
                        return;
                    }
                    if !cx.all_bindings_for_input(std::slice::from_ref(k)).is_empty() {
                        crate::toast::show(format!("{} is also a Kiri shortcut. The global one wins.", pretty(&keys)), cx);
                    }
                    settings::update(cx, |s| s.hotkeys.insert(capture, Some(keys)));
                    true
                }
                _ => false,
            };
            if done {
                window.blur(cx);
            }
        })
        .into_any_element()
}

/// macOS reports Shift-9 as "(", while a global hotkey names the physical key.
fn unshift(key: &str) -> Option<&'static str> {
    const PAIRS: [(&str, &str); 21] = [
        ("!", "1"), ("@", "2"), ("#", "3"), ("$", "4"), ("%", "5"), ("^", "6"), ("&", "7"), ("*", "8"), ("(", "9"), (")", "0"),
        ("_", "-"), ("+", "="), ("{", "["), ("}", "]"), ("|", "\\"), (":", ";"), ("\"", "'"), ("<", ","), (">", "."), ("?", "/"), ("~", "`"),
    ];
    PAIRS.iter().find(|(shifted, _)| *shifted == key).map(|(_, plain)| *plain)
}

fn pretty(keys: &str) -> String {
    keys.split('+')
        .map(|part| match part {
            "ctrl" => "⌃".to_string(),
            "alt" => "⌥".to_string(),
            "shift" => "⇧".to_string(),
            "cmd" => "⌘".to_string(),
            "space" => " Space".to_string(),
            key => key.to_uppercase(),
        })
        .collect()
}

fn advanced(cx: &App) -> Page {
    let code = settings::get(cx).ocr_languages.first().cloned().unwrap_or_default();
    let languages = crate::ocr::languages();
    let language = dropdown(
        "ocr-language",
        std::iter::once(SharedString::from("Automatic")).chain(languages.iter().map(|(_, l)| SharedString::from(l.clone()))).collect(),
        Some(languages.iter().position(|(c, _)| *c == code).map_or(0, |i| i + 1)),
        |i, cx| {
            let code = i.checked_sub(1).and_then(|i| crate::ocr::languages().get(i)).map(|(c, _)| c.clone());
            settings::update(cx, |s| s.ocr_languages = code.into_iter().collect());
        },
    );
    let file = h_flex()
        .gap_2()
        .child(button("reveal", "Show in Finder", |cx| {
            let path = settings::path();
            if !path.exists() {
                settings::update(cx, |_| {});
            }
            cx.reveal_path(&path);
        }))
        .child(button("export", "Export…", |cx| {
            let rx = cx.prompt_for_new_path(&crate::mac::desktop_dir(), Some("Kiri settings.json"));
            cx.spawn(async move |cx| {
                if let Ok(Ok(Some(path))) = rx.await {
                    cx.update(|cx| {
                        if let Err(err) = settings::export(&path, cx) {
                            crate::toast::show(format!("Couldn't export: {err}"), cx);
                        }
                    });
                }
            })
            .detach();
        }))
        .child(button("import", "Import…", |cx| {
            let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
            cx.spawn(async move |cx| {
                if let Ok(Ok(Some(paths))) = rx.await
                    && let Some(path) = paths.into_iter().next()
                {
                    cx.update(|cx| match settings::import(&path, cx) {
                        Ok(()) => {
                            crate::reload_hotkeys(cx);
                            crate::refresh_tray(cx);
                        }
                        Err(err) => crate::toast::show(format!("Couldn't import: {err}"), cx),
                    });
                }
            })
            .detach();
        }));
    Page {
        title: "Advanced",
        icon: IconName::Wrench,
        groups: vec![
            group(
                "Text recognition",
                [
                    row("Primary language", language),
                    row("Remove line breaks", toggle("join-lines", |s| s.ocr_join_lines, |s, v| s.ocr_join_lines = v, cx)),
                ],
            ),
            group(
                "Automation",
                [row(
                    "Allow kiri --capture",
                    toggle("automation", |s| s.automation, |s, v| s.automation = v, cx),
                )
                .description("Lets scripts, Shortcuts, and launchers run captures, as in kiri --capture area.")],
            ),
            group("Settings file", [row("Your settings as JSON", file)]),
        ],
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pages = [general(cx), capture(&self.solid, cx), editor(cx), hotkeys(window, cx), advanced(cx)];
        let query = self.search.read(cx).value().trim().to_lowercase();
        let theme = cx.theme().clone();

        let sidebar = v_flex()
            .w(px(232.))
            .h_full()
            .flex_none()
            .px(px(10.))
            .pt(px(52.))
            .gap(px(2.))
            .bg(theme.sidebar)
            .child(
                div().mb(px(14.)).child(
                    Input::new(&self.search)
                        .appearance(false)
                        .prefix(Icon::new(IconName::Search).small().text_color(theme.muted_foreground))
                        .h(px(32.))
                        .px_3()
                        .rounded_full()
                        .bg(theme.foreground.opacity(0.08)),
                ),
            )
            .children(pages.iter().enumerate().map(|(i, page)| {
                let active = query.is_empty() && i == self.page;
                h_flex()
                    .id(("page", i))
                    .h(px(32.))
                    .px(px(6.))
                    .gap(px(8.))
                    .rounded(px(8.))
                    .text_sm()
                    .when(active, |s| s.bg(theme.sidebar_accent))
                    .hover(|s| s.bg(theme.sidebar_accent.opacity(0.6)))
                    .child(tile(page.icon))
                    .child(page.title)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.page = i;
                        this.search.update(cx, |s, cx| s.set_value("", window, cx));
                        cx.notify();
                    }))
            }));

        let groups: Vec<(Option<SharedString>, Group)> = if query.is_empty() {
            let page = pages.into_iter().nth(self.page).unwrap();
            page.groups.into_iter().map(|g| (g.title.map(SharedString::from), g)).collect()
        } else {
            pages
                .into_iter()
                .flat_map(|page| {
                    let query = query.clone();
                    page.groups.into_iter().filter_map(move |mut g| {
                        let hit = |r: &Row| {
                            r.title.to_lowercase().contains(&query) || r.description.as_ref().is_some_and(|d| d.to_lowercase().contains(&query))
                        };
                        g.rows.retain(hit);
                        let title = match g.title {
                            Some(t) => format!("{} › {t}", page.title),
                            None => page.title.to_string(),
                        };
                        (!g.rows.is_empty()).then(|| (Some(title.into()), Group { description: None, ..g }))
                    })
                })
                .collect()
        };
        let empty = groups.is_empty();

        let content = div()
            .id("settings-content")
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_y_scroll()
            .bg(theme.background)
            .child(
                v_flex()
                    .px_4()
                    .pt(px(52.))
                    .pb_6()
                    .children(groups.into_iter().enumerate().map(|(i, (title, g))| card(i, title, g, &theme)))
                    .when(empty, |s| {
                        s.child(div().pt_16().w_full().text_center().text_sm().text_color(theme.muted_foreground).child("No matching settings"))
                    }),
            );

        // The titlebar is transparent, so this band stands in for it.
        let titlebar = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(44.))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move());

        h_flex()
            .relative()
            .size_full()
            .text_color(theme.foreground)
            .key_context("SettingsWindow")
            .track_focus(&self.focus)
            .on_action(|_: &CloseSettings, window, _| window.remove_window())
            .child(sidebar)
            .child(div().w_px().h_full().bg(theme.border))
            .child(content)
            .child(titlebar)
    }
}

fn tile(icon: IconName) -> impl IntoElement {
    div()
        .size(px(22.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .border_1()
        .border_color(rgb(0x696B6E))
        .bg(linear_gradient(180., linear_color_stop(rgb(0x4A4C4F), 0.), linear_color_stop(rgb(0x313336), 1.)))
        .child(Icon::new(icon).size(px(14.)).text_color(white()))
}

fn card(i: usize, title: Option<SharedString>, group: Group, theme: &gpui_kit::component::Theme) -> impl IntoElement {
    let rows = group.rows.into_iter().enumerate().map(|(j, row)| {
        h_flex()
            .min_h(px(47.))
            .py_2()
            .gap_4()
            .justify_between()
            .when(j > 0, |s| s.border_t_1().border_color(theme.border))
            .when(row.disabled, |s| s.opacity(0.4))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().child(row.title))
                    .when_some(row.description, |s, d| s.child(div().text_xs().text_color(theme.muted_foreground).child(d))),
            )
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(row.control)
                    .when(row.disabled, |s| s.child(div().absolute().inset_0().occlude())),
            )
    });
    v_flex()
        .when(i > 0, |s| s.mt_4())
        .when_some(title, |s, t| s.child(div().px_3().pt_2().pb_2().text_sm().child(t)))
        .when_some(group.description, |s, d| s.child(div().px_3().pb_2().text_xs().text_color(theme.muted_foreground).child(d)))
        .child(v_flex().px_3().rounded(px(12.)).bg(theme.group_box).children(rows))
}
