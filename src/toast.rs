//! Kiri's own toast, for notes and for actions that close the window they came from.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, StyledExt, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::mac;

struct Current(Option<(AnyWindowHandle, Entity<Toast>)>);

impl Global for Current {}

pub fn show(text: impl Into<SharedString>, cx: &mut App) {
    open(text.into(), None, Style::Note, cx);
}

/// A toast with one button; clicking it runs `run` and dismisses the toast.
pub fn with_action(
    text: impl Into<SharedString>,
    label: &'static str,
    run: impl Fn(&mut App) + 'static,
    cx: &mut App,
) {
    open(text.into(), Some((label, Rc::new(run))), Style::Note, cx);
}

/// A success, marked with a check; `action` is an optional button as in `with_action`.
pub fn done(text: impl Into<SharedString>, action: Option<Action>, cx: &mut App) {
    open(text.into(), action, Style::Done, cx);
}

pub fn saved(path: PathBuf, cx: &mut App) {
    if !crate::settings::get(cx).confirmations {
        return;
    }
    let folder = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    let text = folder.map_or("Saved".into(), |f| format!("Saved to {f}"));
    done(text, Some(("Show", Rc::new(move |cx: &mut App| cx.reveal_path(&path)))), cx);
}

pub type Action = (&'static str, Rc<dyn Fn(&mut App)>);

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Note,
    Done,
    Countdown,
    Progress,
}

/// Stays up until another toast or `hide`, and updates in place; `fraction` (0 to 1) adds a bar.
pub fn progress(text: impl Into<SharedString>, fraction: Option<f32>, cx: &mut App) {
    let text = text.into();
    let shown = cx.try_global::<Current>().and_then(|c| c.0.clone()).map(|(_, toast)| toast);
    let shown = shown.filter(|toast| toast.read(cx).style == Style::Progress);
    let Some(toast) = shown.or_else(|| open(text.clone(), None, Style::Progress, cx)) else { return };
    toast.update(cx, |toast, cx| {
        toast.text = text;
        toast.fraction = fraction;
        cx.notify();
    });
}

/// The delayed-capture countdown; `hide` takes it down before the shot.
pub fn countdown(seconds: u32, cx: &mut App) {
    open(seconds.to_string().into(), None, Style::Countdown, cx);
}

pub fn hide(cx: &mut App) {
    if let Some((old, _)) = cx.try_global::<Current>().and_then(|c| c.0.clone()) {
        let _ = old.update(cx, |_, window, _| window.remove_window());
    }
    cx.set_global(Current(None));
}

fn open(text: SharedString, action: Option<Action>, style: Style, cx: &mut App) -> Option<Entity<Toast>> {
    let big = style == Style::Countdown;
    hide(cx);
    let Some(display) = cx
        .displays()
        .into_iter()
        .find(|d| u64::from(d.id()) as u32 == mac::display_under_cursor())
        .or_else(|| cx.primary_display())
    else {
        return None;
    };
    let b = display.bounds();
    let (w, h) = if big { (px(120.), px(120.)) } else { (px(420.), px(48.)) };
    let origin = if big {
        point(b.center().x - w / 2., b.center().y - h / 2.)
    } else {
        point(b.center().x - w / 2., b.bottom() - px(140.))
    };
    let clickable = action.is_some();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, size(w, h)))),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: false,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display.id()),
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let opened = crate::open_clear_window(options, cx, |_, cx| {
        cx.new(|_| Toast { text, action, style, fraction: None })
    });
    let Ok((handle, toast)) = opened else { return None };
    let _ = handle.update(cx, |_, window, _| {
        mac::make_floating(window, mac::Level::Status, false, !clickable)
    });
    cx.set_global(Current(Some((handle, toast.clone()))));
    if style == Style::Progress {
        return Some(toast);
    }
    cx.spawn(async move |cx| {
        let ms = if clickable { 4000 } else { 1800 };
        cx.background_executor().timer(Duration::from_millis(ms)).await;
        cx.update(|cx| {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            if cx.try_global::<Current>().and_then(|c| c.0.as_ref().map(|c| c.0)) == Some(handle) {
                cx.set_global(Current(None));
            }
        });
    })
    .detach();
    Some(toast)
}

struct Toast {
    text: SharedString,
    action: Option<Action>,
    style: Style,
    fraction: Option<f32>,
}

impl Render for Toast {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let action = self.action.clone();
        if self.style == Style::Countdown {
            return div().size_full().flex().items_center().justify_center().child(
                div()
                    .size(px(104.))
                    .rounded(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(hsla(0., 0., 0.08, 0.8))
                    .text_color(white())
                    .text_size(px(52.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .overflow_hidden()
                    .child(div().truncate().child(self.text.clone())),
            );
        }
        div().size_full().flex().items_center().justify_center().child(
            h_flex()
                .id("toast")
                .gap_2()
                .px_4()
                .py_2()
                .rounded_full()
                .bg(cx.theme().popover)
                .border_1()
                .border_color(cx.theme().border)
                .text_sm()
                .text_color(cx.theme().popover_foreground)
                .when(self.style == Style::Done, |d| {
                    d.child(Icon::new(IconName::Check).small().text_color(cx.theme().success))
                })
                .max_w(px(400.))
                .overflow_hidden()
                .child(div().truncate().child(self.text.clone()))
                .when_some(self.fraction, |d, fraction| {
                    d.child(
                        div()
                            .flex_none()
                            .w(px(72.))
                            .h(px(4.))
                            .rounded_full()
                            .bg(cx.theme().muted)
                            .child(div().h_full().rounded_full().bg(cx.theme().primary).w(px(72. * fraction.clamp(0., 1.)))),
                    )
                })
                .when_some(action, |d, (label, run)| {
                    d.child(
                        div()
                            .id("action")
                            .flex_none()
                            .px_2()
                            .rounded_full()
                            .cursor_pointer()
                            .text_xs()
                            .font_medium()
                            .text_color(cx.theme().primary)
                            .hover(|s| s.bg(cx.theme().muted))
                            .child(label)
                            .on_click(move |_, window, cx| {
                                window.remove_window();
                                run(cx);
                            }),
                    )
                }),
        )
    }
}
