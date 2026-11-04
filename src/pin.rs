//! Pins: a finished screenshot floating above other windows as a reference.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use tray_icon::menu::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem};

use crate::mac;
use crate::tiled::TiledImage;

actions!(pin, [CopyAndClose, ClosePin, EditPin]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-c", CopyAndClose, Some("Pin")),
        KeyBinding::new("escape", ClosePin, Some("Pin")),
        KeyBinding::new("cmd-w", ClosePin, Some("Pin")),
        KeyBinding::new("cmd-e", EditPin, Some("Pin")),
    ]);
}

#[derive(Default)]
struct Pins {
    next: u64,
    open: HashMap<u64, (AnyWindowHandle, Entity<Pin>)>,
}

impl Global for Pins {}

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.;

/// `doc` is the editor content the pin came from, so Edit gets the objects back.
pub fn open(image: TiledImage, scale: f32, origin: Point<Pixels>, doc: Option<crate::editor::Document>, cx: &mut App) {
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
    let opened = gpui_kit::open_window(options, cx, |window, cx| {
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

/// Pins made click-through can't be clicked again, so the tray offers this way back.
pub fn close_all(cx: &mut App) {
    let pins: Vec<_> = cx.default_global::<Pins>().open.drain().map(|(_, (h, _))| h).collect();
    for h in pins {
        let _ = h.update(cx, |_, window, _| window.remove_window());
    }
}

/// Routes context-menu clicks, which arrive through the app-wide menu channel.
pub fn menu_event(id: &str, cx: &mut App) -> bool {
    let Some(rest) = id.strip_prefix("pin:") else {
        return false;
    };
    let Some((pin, action)) = rest.split_once(':') else {
        return true;
    };
    let Some((handle, pin)) = pin
        .parse()
        .ok()
        .and_then(|pin: u64| cx.default_global::<Pins>().open.get(&pin).cloned())
    else {
        return true;
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
            pin.click_through = true;
            pin.translucent = true;
            mac::set_opacity(window, 0.5);
            mac::set_ignores_mouse(window, true);
        }
        zoom => {
            if let Ok(z) = zoom.parse::<f32>() {
                pin.set_zoom(z / 100., window, cx);
            }
        }
    }));
    true
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
            &MenuItem::with_id(id("through"), "Click-through", true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(id("close"), "Close", true, None),
        ]);
        if let Some(view) = mac::ns_view_ptr(window) {
            let at = tray_icon::menu::dpi::LogicalPosition::new(position.x.as_f32() as f64, position.y.as_f32() as f64);
            // SAFETY: the view belongs to this live window for the whole modal menu loop.
            unsafe { menu.show_context_menu_for_nsview(view, Some(at.into())) };
        }
    }
}

#[derive(Clone)]
struct DragOut(WeakEntity<Pin>);

struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
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
        div()
            .id("pin")
            .size_full()
            .key_context("Pin")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &CopyAndClose, w, cx| this.copy_and_close(w, cx)))
            .on_action(cx.listener(|this, _: &ClosePin, w, cx| this.close(w, cx)))
            .on_action(cx.listener(|this, _: &EditPin, w, cx| this.edit(w, cx)))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, e: &MouseDownEvent, window, _| this.show_menu(e.position, window)),
            )
            .on_scroll(cx.listener(|this, e: &ScrollEvent, window, cx| {
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
                        .child(tool("close", IconName::X).on_click(cx.listener(|this, _, w, cx| this.close(w, cx))))
                        .child(
                            tool("drag", IconName::GripVertical)
                                .cursor_grab()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_drag(DragOut(cx.weak_entity()), |_, _, _, cx| cx.new(|_| DragGhost))
                                .external_drag_payload(|drag: &DragOut, _, cx| {
                                    let png = drag.0.upgrade()?.read(cx).png();
                                    let path = crate::editor::write_temp(&png)?;
                                    Some(ExternalDragPayload::Files(FileDragPaths::new([(path, false)])))
                                }),
                        )
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
