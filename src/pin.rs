//! Pins: a finished screenshot floating above other windows as a reference.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
#[cfg(not(target_os = "linux"))]
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};

use crate::editor::DragFileOut;
use crate::mac;
use crate::tiled::TiledImage;

actions!(pin, [CopyAndClose, ClosePin, EditPin]);

pub fn bind_keys(cx: &mut App) {
    crate::bind_keys(cx, [
        KeyBinding::new(crate::keys("cmd-c"), CopyAndClose, Some("Pin")),
        KeyBinding::new("escape", ClosePin, Some("Pin")),
        KeyBinding::new(crate::keys("cmd-w"), ClosePin, Some("Pin")),
        KeyBinding::new(crate::keys("cmd-e"), EditPin, Some("Pin")),
    ]);
}

#[derive(Default)]
struct Pins {
    next: u64,
    open: HashMap<u64, (AnyWindowHandle, Entity<Pin>)>,
    watching_option: bool,
}

impl Global for Pins {}

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.;

/// `doc` is the editor content the pin came from, so Edit gets the objects back.
pub fn open(image: TiledImage, scale: f32, origin: Point<Pixels>, doc: Option<crate::editor::Document>, cx: &mut App) {
    if !crate::direct_screen() {
        return crate::toast::show("Pins aren't available on Wayland", cx);
    }
    let id = {
        let pins = cx.default_global::<Pins>();
        pins.next += 1;
        pins.next
    };
    let natural = size(px(image.width as f32 / scale), px(image.height as f32 / scale));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, natural))),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: true,
        show: true,
        is_movable: true,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let opened = crate::open_clear_window(options, cx, |window, cx| {
        let pin = cx.new(|cx| Pin {
            id,
            image,
            scale,
            doc,
            zoom: 1.,
            translucent: false,
            click_through: false,
            focus: cx.focus_handle(),
        });
        window.focus(&pin.read(cx).focus.clone(), cx);
        pin
    });
    let Ok((handle, pin)) = opened else { return };
    let _ = handle.update(cx, |_, window, _| mac::make_floating(window, mac::Level::Floating, true, false));
    cx.default_global::<Pins>().open.insert(id, (handle, pin));
}

/// Click-through pins can only be reached with ⌥ held, so the tray offers this way back too.
pub fn close_all(cx: &mut App) {
    let pins: Vec<_> = cx.default_global::<Pins>().open.drain().map(|(_, (h, _))| h).collect();
    for h in pins {
        let _ = h.update(cx, |_, window, _| window.remove_window());
    }
}

/// Routes context-menu clicks, which arrive through the app-wide menu channel.
pub fn menu_event(id: &str, cx: &mut App) {
    let Some((pin, action)) = id.strip_prefix("pin:").and_then(|rest| rest.split_once(':')) else {
        return;
    };
    let Some((handle, pin)) = pin
        .parse()
        .ok()
        .and_then(|pin: u64| cx.default_global::<Pins>().open.get(&pin).cloned())
    else {
        return;
    };
    let _ = handle.update(cx, |_, window, cx| pin.update(cx, |pin, cx| match action {
        "edit" => pin.edit(window, cx),
        "copy" => pin.copy_and_close(window, cx),
        "close" => pin.close(window, cx),
        "translucent" => {
            pin.translucent = !pin.translucent;
            mac::set_opacity(window, if pin.translucent { 0.5 } else { 1. });
        }
        "through" => {
            pin.click_through = !pin.click_through;
            mac::set_ignores_mouse(window, pin.click_through);
            if pin.click_through {
                pin.translucent = true;
                mac::set_opacity(window, 0.5);
                watch_option(cx);
            }
        }
        zoom => {
            if let Ok(z) = zoom.parse::<f32>() {
                pin.set_zoom(z / 100., window, cx);
            }
        }
    }));
}

/// While ⌥ is held, click-through pins take the mouse again, to be moved or turned back.
/// Modifier changes reach no window of a background app, so the state is polled.
fn watch_option(cx: &mut App) {
    if std::mem::replace(&mut cx.default_global::<Pins>().watching_option, true) {
        return;
    }
    cx.spawn(async move |cx| {
        let mut held = false;
        loop {
            cx.background_executor().timer(Duration::from_millis(80)).await;
            let now = mac::option_held();
            let done = cx.update(|cx| {
                let pins: Vec<_> = cx.default_global::<Pins>().open.values().cloned().collect();
                let through: Vec<_> = pins.into_iter().filter(|(_, p)| p.read(cx).click_through).collect();
                if through.is_empty() {
                    cx.default_global::<Pins>().watching_option = false;
                    return true;
                }
                if now != held {
                    for (h, _) in through {
                        let _ = h.update(cx, |_, window, _| mac::set_ignores_mouse(window, !now));
                    }
                }
                false
            });
            held = now;
            if done {
                return;
            }
        }
    })
    .detach();
}

pub struct Pin {
    id: u64,
    image: TiledImage,
    scale: f32,
    doc: Option<crate::editor::Document>,
    zoom: f32,
    translucent: bool,
    click_through: bool,
    focus: FocusHandle,
}

impl Pin {
    fn set_zoom(&mut self, zoom: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let (w, h) = (
            self.image.width as f32 / self.scale * self.zoom,
            self.image.height as f32 / self.scale * self.zoom,
        );
        mac::resize_from_top(window, w.max(24.), h.max(24.));
        cx.notify();
    }

    fn png(&self) -> Vec<u8> {
        crate::export::encode_bgra(self.image.width, self.image.height, self.image.read(self.image.full()), self.scale)
    }

    fn copy_and_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (image, scale) = (self.image.clone(), self.scale);
        crate::editor::copy_later(move || crate::export::encode_bgra(image.width, image.height, image.read(image.full()), scale), cx);
        self.close(window, cx);
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.default_global::<Pins>().open.remove(&self.id);
        window.remove_window();
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (image, scale, doc) = (self.image.clone(), self.scale, self.doc.take());
        cx.defer(move |cx| match doc {
            Some(doc) => crate::editor::reopen(doc, cx),
            None => crate::editor::open(image, scale, None, None, cx),
        });
        self.close(window, cx);
    }

    /// Linux has no native menu without GTK, and one drawn inside the pin would be cut off by it.
    #[cfg(not(target_os = "linux"))]
    fn show_menu(&self, position: Point<Pixels>, window: &mut Window) {
        let id = |action: &str| format!("pin:{}:{action}", self.id);
        let menu = Menu::new();
        let zooms: Vec<_> = [50, 100, 200]
            .map(|z| {
                let current = (self.zoom * 100.).round() as i32 == z;
                CheckMenuItem::with_id(id(&z.to_string()), format!("{z}%"), true, current, None)
            })
            .into();
        let _ = menu.append_items(&[
            &MenuItem::with_id(id("edit"), "Edit", true, None),
            &MenuItem::with_id(id("copy"), "Copy", true, None),
            &PredefinedMenuItem::separator(),
            &zooms[0],
            &zooms[1],
            &zooms[2],
            &PredefinedMenuItem::separator(),
            &CheckMenuItem::with_id(id("translucent"), "Semi-transparent", true, self.translucent, None),
            &CheckMenuItem::with_id(id("through"), crate::platform_text("Click-through (hold ⌥ to reach it)", "Click-through (hold Alt to reach it)"), true, self.click_through, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(id("close"), "Close", true, None),
        ]);
        mac::show_context_menu(window, &menu, position.x.as_f32(), position.y.as_f32());
    }
}

impl Render for Pin {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tiles: Vec<(crate::tiled::PxRect, Arc<RenderImage>)> =
            self.image.tiles().map(|(r, t)| (r, t.clone())).collect();
        let k = self.zoom / self.scale;
        let hovered = window.is_window_hovered();
        let tool = |id: &'static str, icon: IconName| {
            div()
                .id(id)
                .size(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .hover(|s| s.bg(white().opacity(0.18)))
                .child(Icon::new(icon).xsmall().text_color(white()))
        };
        let pin = div()
            .id("pin")
            .size_full()
            .key_context("Pin")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &CopyAndClose, w, cx| this.copy_and_close(w, cx)))
            .on_action(cx.listener(|this, _: &ClosePin, w, cx| this.close(w, cx)))
            .on_action(cx.listener(|this, _: &EditPin, w, cx| this.edit(w, cx)))
            .on_mouse_down(MouseButton::Left, |_, window, _| mac::start_window_move(window));
        #[cfg(not(target_os = "linux"))]
        let pin = pin.on_mouse_down(MouseButton::Right, cx.listener(|this, e: &MouseDownEvent, window, _| this.show_menu(e.position, window)));
        pin.on_scroll(cx.listener(|this, e: &ScrollEvent, window, cx| {
                let dy = e.delta.pixel_delta(px(16.)).y.as_f32();
                this.set_zoom(this.zoom * (1. + dy * 0.004), window, cx);
            }))
            .on_pinch(cx.listener(|this, e: &PinchEvent, window, cx| {
                this.set_zoom(this.zoom * (1. + e.delta), window, cx);
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        for (r, tile) in &tiles {
                            let b = Bounds::new(
                                point(bounds.origin.x + px(r.x as f32 * k), bounds.origin.y + px(r.y as f32 * k)),
                                size(px(r.w as f32 * k), px(r.h as f32 * k)),
                            );
                            let _ = window.paint_image(b, b, Corners::default(), tile.clone(), 0, false);
                        }
                    },
                )
                .size_full(),
            )
            .when(hovered && !self.click_through, |d| {
                d.child(
                    h_flex()
                        .absolute()
                        .top(px(6.))
                        .left(px(6.))
                        .p(px(2.))
                        .gap(px(1.))
                        .rounded(px(7.))
                        .bg(black().opacity(0.62))
                        .when(cfg!(windows), |d| d.occlude())
                        .child(tool("close", IconName::X).on_click(cx.listener(|this, _, w, cx| this.close(w, cx))))
                        .when(crate::editor::DRAGS_OUT, |d| d.child(
                            tool("drag", IconName::GripVertical)
                                .cursor_grab()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .drag_file_out(cx.weak_entity(), |pin: &Pin, _| pin.png()),
                        ))
                        .child(tool("edit", IconName::Pencil).on_click(cx.listener(|this, _, w, cx| this.edit(w, cx))))
                        .child(
                            tool("copy", IconName::Copy)
                                .on_click(cx.listener(|this, _, w, cx| this.copy_and_close(w, cx))),
                        )
                        .child(
                            div()
                                .px_1()
                                .text_xs()
                                .text_color(white().opacity(0.85))
                                .child(format!("{}%", (self.zoom * 100.).round())),
                        ),
                )
            })
    }
}
