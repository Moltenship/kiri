//! The after-capture thumbnail: a small floating card for one-click follow-ups.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::*;

use crate::editor::DragFileOut;
use crate::tiled::{PxRect, TiledImage};
use crate::{export, mac, settings};

actions!(preview, [CopyAndClose, Edit, Save, Pin, Recognize, Close]);

pub fn bind_keys(cx: &mut App) {
    let c = Some("Preview");
    cx.bind_keys([
        KeyBinding::new(crate::keys("cmd-c"), CopyAndClose, c),
        KeyBinding::new("enter", Edit, c),
        KeyBinding::new(crate::keys("cmd-s"), Save, c),
        KeyBinding::new(crate::keys("cmd-p"), Pin, c),
        KeyBinding::new(crate::keys("cmd-o"), Recognize, c),
        KeyBinding::new("escape", Close, c),
        KeyBinding::new(crate::keys("cmd-w"), Close, c),
    ]);
}

#[derive(Default)]
struct Stack(Vec<AnyWindowHandle>);

impl Global for Stack {}

const WIDTH: f32 = 260.;
const MAX_IMAGE_H: f32 = 170.;
const BAR_H: f32 = 40.;
const MARGIN: f32 = 16.;

pub fn open(image: TiledImage, scale: f32, display: Option<u32>, saved: Option<PathBuf>, cx: &mut App) {
    let display = display.unwrap_or_else(mac::display_under_cursor);
    let Some(screen) = cx.displays().into_iter().find(|d| u64::from(d.id()) as u32 == display) else {
        return;
    };
    let k = ((WIDTH - 16.) / image.width as f32).min(MAX_IMAGE_H / image.height as f32);
    let card = size(px(WIDTH), px(image.height as f32 * k + BAR_H + 16.));
    let stacked: f32 = cx
        .default_global::<Stack>()
        .0
        .clone()
        .iter()
        .filter_map(|h| h.update(cx, |_, w, _| w.bounds().size.height.as_f32()).ok())
        .map(|h| h + 10.)
        .sum();
    let b = screen.bounds();
    let origin = point(
        b.right() - card.width - px(MARGIN),
        b.bottom() - card.height - px(MARGIN + 60. + stacked),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, card))),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: true,
        show: true,
        is_movable: true,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(screen.id()),
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let (tw, th) = (
        ((image.width as f32 * k * 2.).round() as u32).max(1),
        ((image.height as f32 * k * 2.).round() as u32).max(1),
    );
    let thumb = if tw < image.width {
        let small = crate::pixels::shrink(&image.read(image.full()), image.width, image.height, tw, th);
        Arc::new(crate::tiled::frame_image(tw, th, small))
    } else {
        image.tiles().next().map(|(_, t)| t.clone()).expect("an image has a tile")
    };
    let opened = crate::open_clear_window(options, cx, |window, cx| {
        let view = cx.new(|cx| Preview {
            thumb,
            image,
            scale,
            display,
            saved,
            focus: cx.focus_handle(),
        });
        window.focus(&view.read(cx).focus.clone(), cx);
        view
    });
    let Ok((handle, preview)) = opened else { return };
    let _ = handle.update(cx, |_, window, _| mac::make_floating(window, mac::Level::Floating, true, false));
    cx.default_global::<Stack>().0.push(handle);
    cx.activate(true);
    let seconds = settings::get(cx).thumbnail_seconds;
    if seconds > 0 {
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(seconds as u64)).await;
            cx.update(|cx| {
                let _ = handle.update(cx, |_, window, cx| preview.update(cx, |this, cx| this.close(window, cx)));
            });
        })
        .detach();
    }
}

pub struct Preview {
    thumb: Arc<RenderImage>,
    image: TiledImage,
    scale: f32,
    display: u32,
    saved: Option<PathBuf>,
    focus: FocusHandle,
}

impl Preview {
    fn png(&self) -> Vec<u8> {
        export::encode_bgra(self.image.width, self.image.height, self.image.read(self.image.full()), self.scale)
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let me = window.window_handle();
        cx.default_global::<Stack>().0.retain(|h| *h != me);
        window.remove_window();
    }

    fn copy_and_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (image, scale) = (self.image.clone(), self.scale);
        crate::editor::copy_later(move || export::encode_bgra(image.width, image.height, image.read(image.full()), scale), cx);
        self.close(window, cx);
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self
            .saved
            .clone()
            .unwrap_or_else(|| settings::get(cx).save_path());
        match export::save_capture(&self.image, self.scale, self.png(), path, settings::get(cx).save_auto_jpeg) {
            Ok(path) => crate::toast::saved(path, cx),
            Err(err) => crate::toast::show(format!("Couldn't save: {err}"), cx),
        }
        self.close(window, cx);
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (image, scale, display) = (self.image.clone(), self.scale, self.display);
        cx.defer(move |cx| crate::editor::open(image, scale, Some(display), None, cx));
        self.close(window, cx);
    }

    fn pin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let image = self.image.clone();
        let scale = self.scale;
        let b = window.bounds();
        cx.defer(move |cx| crate::pin::open(image, scale, b.origin, None, cx));
        self.close(window, cx);
    }

    fn recognize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let PxRect { w, h, .. } = self.image.full();
        crate::ocr::recognize_and_copy(self.image.read(self.image.full()), w, h, cx);
        self.close(window, cx);
    }
}

#[derive(Clone)]
struct DragOut(WeakEntity<Preview>);

struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let thumb = self.thumb.clone();
        let (iw, ih) = (self.image.width as f32, self.image.height as f32);
        let action = |id: &'static str, icon: IconName, tip: &'static str| {
            Button::new(id).icon(Icon::new(icon)).ghost().small().tooltip(tip)
        };
        v_flex()
            .id("preview")
            .size_full()
            .p(px(8.))
            .gap(px(6.))
            .rounded(px(14.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .key_context("Preview")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &CopyAndClose, w, cx| this.copy_and_close(w, cx)))
            .on_action(cx.listener(|this, _: &Edit, w, cx| this.edit(w, cx)))
            .on_action(cx.listener(|this, _: &Save, w, cx| this.save(w, cx)))
            .on_action(cx.listener(|this, _: &Pin, w, cx| this.pin(w, cx)))
            .on_action(cx.listener(|this, _: &Recognize, w, cx| this.recognize(w, cx)))
            .on_action(cx.listener(|this, _: &Close, w, cx| this.close(w, cx)))
            .child(
                div()
                    .id("thumb")
                    .flex_1()
                    .w_full()
                    .cursor_pointer()
                    .rounded(px(8.))
                    .overflow_hidden()
                    .on_click(cx.listener(|this, e: &ClickEvent, w, cx| {
                        if e.click_count() >= 1 {
                            this.edit(w, cx);
                        }
                    }))
                    .on_drag(DragOut(cx.weak_entity()), |_, _, _, cx| cx.new(|_| DragGhost))
                    .drag_file_out(|drag: &DragOut, cx| {
                        let png = drag.0.upgrade()?.read(cx).png();
                        crate::editor::write_temp(&png, &crate::settings::get(cx).file_name)
                    })
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                let k = (bounds.size.width.as_f32() / iw).min(bounds.size.height.as_f32() / ih);
                                let ox = bounds.origin.x.as_f32() + (bounds.size.width.as_f32() - iw * k) / 2.;
                                let oy = bounds.origin.y.as_f32() + (bounds.size.height.as_f32() - ih * k) / 2.;
                                let b = Bounds::new(point(px(ox), px(oy)), size(px(iw * k), px(ih * k)));
                                let _ = window.paint_image(b, b, Corners::all(px(6.)), thumb.clone(), 0, false);
                            },
                        )
                        .size_full(),
                    ),
            )
            .child(
                h_flex()
                    .h(px(BAR_H - 8.))
                    .justify_between()
                    .child(
                        h_flex()
                            .gap(px(2.))
                            .child(action("copy", IconName::Copy, crate::platform_text("Copy (⌘C)", "Copy (Ctrl+C)")).on_click(
                                cx.listener(|this, _, w, cx| this.copy_and_close(w, cx)),
                            ))
                            .child(action("save", IconName::Download, crate::platform_text("Save (⌘S)", "Save (Ctrl+S)")).on_click(
                                cx.listener(|this, _, w, cx| this.save(w, cx)),
                            ))
                            .child(action("edit", IconName::Pencil, crate::platform_text("Edit (↩)", "Edit (Enter)")).on_click(
                                cx.listener(|this, _, w, cx| this.edit(w, cx)),
                            ))
                            .child(action("ocr", IconName::ScanText, crate::platform_text("Recognize text (⌘O)", "Recognize text (Ctrl+O)")).on_click(
                                cx.listener(|this, _, w, cx| this.recognize(w, cx)),
                            ))
                            .child(action("pin", IconName::Pin, crate::platform_text("Pin (⌘P)", "Pin (Ctrl+P)")).on_click(
                                cx.listener(|this, _, w, cx| this.pin(w, cx)),
                            )),
                    )
                    .child(action("close", IconName::X, "Close (Esc)").on_click(
                        cx.listener(|this, _, w, cx| this.close(w, cx)),
                    )),
            )
    }
}
