//! The first-run window: Screen Recording, the capture shortcuts, and the look.

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::settings::{self, Capture};

const STEPS: usize = 3;
const SCREEN_RECORDING: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

struct Open(AnyWindowHandle);

impl Global for Open {}

pub fn open(cx: &mut App) {
    if let Some(handle) = cx.try_global::<Open>().map(|o| o.0)
        && handle.update(cx, |_, window, _| window.activate_window()).is_ok()
    {
        cx.activate(true);
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(600.), px(500.)), cx))),
        titlebar: Some(TitlebarOptions {
            title: Some("Welcome to Kiri".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(16.), px(16.))),
        }),
        is_resizable: false,
        focus: true,
        show: true,
        ..Default::default()
    };
    if let Ok((handle, _)) = crate::open_window(options, cx, |window, cx| cx.new(|cx| Onboarding::new(window, cx))) {
        cx.set_global(Open(handle));
    }
    cx.activate(true);
}

struct Onboarding {
    step: usize,
    /// ⇧⌘3 and ⇧⌘4 instead of the macOS screenshots; applied by Done.
    replace_system: bool,
    screen_access: bool,
    icon: Arc<Image>,
    _subscriptions: [Subscription; 2],
}

impl Onboarding {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // macOS grants Screen Recording in System Settings, so check again on coming back.
        let activation = cx.observe_window_activation(window, |this, _, cx| {
            this.screen_access = crate::mac::has_screen_access();
            cx.notify();
        });
        // Writing the settings file, even unchanged, keeps this window from showing again.
        let release = cx.on_release(|_, cx| settings::update(cx, |_| {}));
        Self {
            step: 0,
            replace_system: settings::get(cx).hotkey(Capture::Area).as_deref() == Some("shift+cmd+4"),
            screen_access: crate::mac::has_screen_access(),
            icon: Arc::new(Image::from_bytes(ImageFormat::Png, include_bytes!("icon.png").to_vec())),
            _subscriptions: [activation, release],
        }
    }

    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let replace = self.replace_system;
        let (screen, area) = if replace { ("shift+cmd+3", "shift+cmd+4") } else { ("shift+cmd+1", "shift+cmd+2") };
        settings::update(cx, |s| {
            s.hotkeys.insert(Capture::Fullscreen, Some(screen.into()));
            s.hotkeys.insert(Capture::Area, Some(area.into()));
        });
        if replace && let Err(err) = crate::mac::release_screenshot_keys() {
            crate::toast::show(format!("Couldn't turn off the macOS shortcuts: {err}"), cx);
        }
        crate::settings_window::refresh_system_shortcuts(cx);
        if let Some(fix) = [screen, area].into_iter().find_map(|keys| crate::settings_window::system_conflict(keys, cx)) {
            crate::toast::with_action(fix, "Open", |cx| cx.open_url(crate::settings_window::KEYBOARD_SETTINGS), cx);
        }
        for keys in crate::reload_hotkeys(cx) {
            crate::toast::show(format!("{} is taken by another app", crate::settings_window::pretty(&keys)), cx);
        }
        crate::refresh_tray(cx);
        window.remove_window();
    }

    fn welcome(&self, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let permission = if self.screen_access {
            h_flex()
                .gap_1p5()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Check).small().text_color(theme.success))
                .child("Allowed")
                .into_any_element()
        } else {
            Button::new("allow")
                .label("Allow…")
                .small()
                .outline()
                .on_click(|_, _, cx| {
                    if !crate::mac::ensure_screen_access() {
                        cx.open_url(SCREEN_RECORDING);
                    }
                })
                .into_any_element()
        };
        v_flex()
            .items_center()
            .child(img(self.icon.clone()).size(px(112.)))
            .child(div().mt_3().text_2xl().font_semibold().child("Welcome to Kiri"))
            .child(
                div()
                    .mt_2()
                    .max_w(px(380.))
                    .text_center()
                    .text_color(theme.muted_foreground)
                    .child("Kiri lives in the menu bar. Press a shortcut to capture, then mark it up, copy it, or pin it."),
            )
            .child(
                h_flex()
                    .mt_8()
                    .w(px(440.))
                    .px_4()
                    .py_3()
                    .gap_4()
                    .rounded(px(12.))
                    .bg(theme.group_box)
                    .child(
                        v_flex()
                            .flex_1()
                            .child(div().text_sm().child("Screen Recording"))
                            .child(div().text_xs().text_color(theme.muted_foreground).child("Kiri needs it to capture. Nothing leaves your Mac.")),
                    )
                    .child(permission),
            )
            .into_any_element()
    }

    fn shortcuts(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let option = |id: &'static str, replace: bool, title: &'static str, keys: [(&'static str, &'static str); 2], note: &'static str| {
            let selected = self.replace_system == replace;
            v_flex()
                .id(id)
                .w(px(232.))
                .p_4()
                .gap_3()
                .rounded(px(12.))
                .bg(theme.group_box)
                .border_2()
                .border_color(if selected { theme.ring } else { transparent_black() })
                .cursor_pointer()
                .child(div().text_sm().font_medium().child(title))
                .children(keys.map(|(keys, action)| {
                    h_flex()
                        .gap_2()
                        .child(keycap(keys, &theme))
                        .child(div().text_sm().text_color(theme.muted_foreground).child(action))
                }))
                .child(div().text_xs().text_color(theme.muted_foreground).child(note))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.replace_system = replace;
                    cx.notify();
                }))
        };
        v_flex()
            .items_center()
            .child(div().text_xl().font_semibold().child("Choose the capture shortcuts"))
            .child(
                div()
                    .mt_2()
                    .text_center()
                    .text_color(theme.muted_foreground)
                    .child("Every action can have its own in Settings › Hotkeys."),
            )
            .child(
                h_flex()
                    .mt_8()
                    .w(px(480.))
                    .gap_4()
                    .items_stretch()
                    .child(option(
                        "beside",
                        false,
                        "Next to macOS",
                        [("⇧⌘2", "Capture Area"), ("⇧⌘1", "Capture Screen")],
                        "The macOS screenshot shortcuts stay as they are.",
                    ))
                    .child(option(
                        "instead",
                        true,
                        "Instead of macOS",
                        [("⇧⌘4", "Capture Area"), ("⇧⌘3", "Capture Screen")],
                        "Turns off ⇧⌘3 and ⇧⌘4 in macOS, so they open Kiri. ⇧⌘5 stays.",
                    )),
            )
            .into_any_element()
    }

    fn look(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let login = Switch::new("login").checked(settings::launch_at_login()).on_click(|v, window, cx| {
            if let Err(err) = settings::set_launch_at_login(*v) {
                crate::toast::show(format!("Couldn't change login item: {err}"), cx);
            }
            crate::refresh_tray(cx);
            window.refresh();
        });
        v_flex()
            .items_center()
            .child(div().text_xl().font_semibold().child("Make it yours"))
            .child(div().mt_2().text_color(theme.muted_foreground).child("Settings › General changes these later."))
            .child(div().mt_8().child(crate::settings_window::appearance_picker(136., cx)))
            .child(
                h_flex()
                    .mt_8()
                    .w(px(440.))
                    .px_4()
                    .py_3()
                    .rounded(px(12.))
                    .bg(theme.group_box)
                    .justify_between()
                    .child(div().text_sm().child("Launch at login"))
                    .child(login),
            )
            .into_any_element()
    }
}

fn keycap(keys: &'static str, theme: &gpui_kit::component::Theme) -> impl IntoElement {
    div()
        .min_w(px(44.))
        .h(px(24.))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .bg(theme.secondary)
        .border_1()
        .border_color(theme.border)
        .text_sm()
        .child(keys)
}

impl Render for Onboarding {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let page = match self.step {
            0 => self.welcome(cx),
            1 => self.shortcuts(cx),
            _ => self.look(cx),
        };
        let last = self.step + 1 == STEPS;
        let dots = h_flex().gap_1p5().children((0..STEPS).map(|i| {
            div().size(px(6.)).rounded_full().bg(if i == self.step { theme.foreground } else { theme.foreground.opacity(0.2) })
        }));
        let footer = h_flex()
            .px_6()
            .pb_6()
            .justify_between()
            .child(dots)
            .child(
                h_flex()
                    .gap_2()
                    .when(self.step > 0, |s| {
                        s.child(Button::new("back").label("Back").ghost().on_click(cx.listener(|this, _, _, cx| {
                            this.step -= 1;
                            cx.notify();
                        })))
                    })
                    .child(Button::new("next").label(if last { "Done" } else { "Continue" }).primary().on_click(cx.listener(
                        move |this, _, window, cx| {
                            if last {
                                this.finish(window, cx);
                            } else {
                                this.step += 1;
                                cx.notify();
                            }
                        },
                    ))),
            );
        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .h(px(44.))
                    .flex_none()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()),
            )
            .child(div().flex_1().flex().flex_col().justify_center().pb_4().child(page))
            .child(footer)
    }
}
