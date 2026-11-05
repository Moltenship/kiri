use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hasher;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::DropdownMenu;
use gpui_kit::component::popover::{Popover, PopoverState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Selectable, Sizable, StyledExt, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::doc::{self, Ctx, Handle, HIGHLIGHTS, Object, P, PALETTE, Part, Prim, R, Shape, Style, Tool};
use crate::paint::{self, View};
use crate::text_edit::{self as te, TextEdit};
use crate::tiled::{PxRect, TiledImage};
use crate::settings::ColorFormat;
use crate::{export, mac, pixels, settings};

actions!(
    editor,
    [
        CopyImage,
        CopyAndClose,
        CopyObjectOrClose,
        Save,
        SaveAs,
        Pin,
        Print,
        AddCapture,
        Close,
        Undo,
        Redo,
        Delete,
        Duplicate,
        Paste,
        Confirm,
        Cancel,
        SelectAll,
        ToolSelect,
        ToolArrow,
        ToolText,
        ToolRect,
        ToolOval,
        ToolLine,
        ToolPen,
        ToolHighlighter,
        ToolCounter,
        ToolRedact,
        ToolSpotlight,
        ToolMagnifier,
        ZoomIn,
        ZoomOut,
        ZoomFit,
        ZoomActual,
        ZoomSelection,
        SelectionTopLeft,
        SelectionBottomRight,
        Autoadjust,
        GrowSelection,
        ShrinkSelection,
        RecognizeText,
        ImprintSize,
        ResetCrop,
        CopyColor,
        CopyTextColor,
        CopyAverageColor,
        ToggleUnit,
        ToggleBackdrop,
        Rasterize
    ]
);

fn editor_in(window: AnyWindowHandle, cx: &App) -> Option<Entity<Editor>> {
    let root = window.downcast::<gpui_kit::base::Root>()?.read(cx).ok()?;
    root.view().clone().downcast::<Editor>().ok()
}

/// Drops a finished Add Capture onto the editor that asked for it.
pub fn add_capture(window: AnyWindowHandle, image: TiledImage, scale: f32, cx: &mut App) {
    let Some(editor) = editor_in(window, cx) else { return };
    let bgra = image.read(image.full());
    let _ = window.update(cx, |_, window, cx| {
        window.activate_window();
        editor.update(cx, |e, cx| e.insert_image(image.width, image.height, bgra, scale, None, window, cx));
    });
}

const TOOL_ACTIONS: [(Tool, fn() -> Box<dyn Action>); 12] = [
    (Tool::Select, || Box::new(ToolSelect)),
    (Tool::Arrow, || Box::new(ToolArrow)),
    (Tool::Text, || Box::new(ToolText)),
    (Tool::Rect, || Box::new(ToolRect)),
    (Tool::Oval, || Box::new(ToolOval)),
    (Tool::Line, || Box::new(ToolLine)),
    (Tool::Pen, || Box::new(ToolPen)),
    (Tool::Highlighter, || Box::new(ToolHighlighter)),
    (Tool::Counter, || Box::new(ToolCounter)),
    (Tool::Redact, || Box::new(ToolRedact)),
    (Tool::Spotlight, || Box::new(ToolSpotlight)),
    (Tool::Magnifier, || Box::new(ToolMagnifier)),
];

/// Asks once, since ⌘Q out of habit would otherwise kill the hotkeys along with the window.
fn quit(cx: &mut App) {
    use settings::CmdQ;
    let close = |w: AnyWindowHandle, cx: &mut App| {
        let _ = w.update(cx, |_, window, _| window.remove_window());
    };
    let Some(w) = cx.active_window() else { return cx.quit() };
    match settings::get(cx).cmd_q {
        CmdQ::Quit => cx.quit(),
        CmdQ::CloseWindow => close(w, cx),
        CmdQ::Ask => {
            let answer = w.update(cx, |_, window, cx| {
                window.prompt(
                    PromptLevel::Info,
                    "Quit Kiri?",
                    Some("Hotkeys and the menu bar icon stop working until you open Kiri again. Closing the window keeps them. You can change this in Settings › General."),
                    &["Quit", "Close Window"],
                    cx,
                )
            });
            let Ok(answer) = answer else { return cx.quit() };
            cx.spawn(async move |cx| {
                let Ok(i) = answer.await else { return };
                cx.update(|cx| {
                    let choice = if i == 0 { CmdQ::Quit } else { CmdQ::CloseWindow };
                    settings::update(cx, |s| s.cmd_q = choice);
                    match choice {
                        CmdQ::Quit => cx.quit(),
                        _ => close(w, cx),
                    }
                });
            })
            .detach();
        }
    }
}

pub fn bind_keys(cx: &mut App) {
    cx.on_action(|_: &OpenSettings, cx| crate::settings_window::open(cx));
    cx.on_action(|_: &Quit, cx| cx.defer(quit));
    cx.on_action(|_: &OpenFile, cx| crate::open_file(cx));
    cx.on_action(|_: &OpenClipboard, cx| crate::open_clipboard(cx));
    // Deferred: the window that dispatched the action is borrowed until it returns.
    cx.on_action(|_: &CloseAll, cx| {
        cx.defer(|cx| {
            for w in cx.windows() {
                if editor_in(w, cx).is_some() {
                    let _ = w.update(cx, |_, window, _| window.remove_window());
                }
            }
        })
    });
    cx.bind_keys([
        KeyBinding::new(crate::keys("cmd-,"), OpenSettings, None),
        KeyBinding::new(crate::keys("shift-cmd-o"), OpenFile, None),
        KeyBinding::new(crate::keys("shift-cmd-v"), OpenClipboard, None),
        KeyBinding::new(crate::platform_text("alt-cmd-w", "ctrl-shift-w"), CloseAll, None),
    ]);
    // Windows quits from the tray; Alt+F4 closes the window.
    if !cfg!(windows) {
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    }
    let base = Some("Editor");
    let idle = Some("Editor && !text");
    let tools = Some("Editor && !text && !selection");
    let sel = Some("Editor && !text && selection");
    let mut keys = vec![
        KeyBinding::new(crate::keys("cmd-c"), CopyObjectOrClose, idle),
        KeyBinding::new(crate::keys("shift-cmd-c"), CopyImage, base),
        KeyBinding::new(crate::keys("cmd-s"), Save, base),
        KeyBinding::new(crate::keys("shift-cmd-s"), SaveAs, base),
        KeyBinding::new(crate::keys("cmd-p"), Pin, base),
        KeyBinding::new(crate::keys("shift-cmd-p"), Print, base),
        KeyBinding::new(crate::keys("cmd-w"), Close, base),
        KeyBinding::new(crate::keys("cmd-z"), Undo, base),
        KeyBinding::new(crate::keys("shift-cmd-z"), Redo, base),
        KeyBinding::new("backspace", Delete, idle),
        KeyBinding::new("delete", Delete, idle),
        KeyBinding::new(crate::keys("cmd-d"), Duplicate, idle),
        KeyBinding::new(crate::keys("cmd-v"), Paste, idle),
        KeyBinding::new("enter", Confirm, idle),
        KeyBinding::new("escape", Cancel, base),
        KeyBinding::new(crate::keys("cmd-a"), SelectAll, idle),
        KeyBinding::new(crate::keys("cmd-="), ZoomIn, base),
        KeyBinding::new(crate::keys("cmd-+"), ZoomIn, base),
        KeyBinding::new("w", ZoomIn, tools),
        KeyBinding::new(crate::keys("cmd--"), ZoomOut, base),
        KeyBinding::new("q", ZoomOut, tools),
        KeyBinding::new("q", SelectionTopLeft, sel),
        KeyBinding::new("w", SelectionBottomRight, sel),
        KeyBinding::new(crate::keys("cmd-1"), ZoomFit, base),
        KeyBinding::new(crate::keys("cmd-0"), ZoomActual, base),
        KeyBinding::new(crate::keys("cmd-2"), ZoomSelection, base),
        KeyBinding::new("a", Autoadjust, sel),
        KeyBinding::new("shift-]", GrowSelection, sel),
        KeyBinding::new("}", GrowSelection, sel),
        KeyBinding::new("shift-[", ShrinkSelection, sel),
        KeyBinding::new("{", ShrinkSelection, sel),
        KeyBinding::new(crate::keys("cmd-o"), RecognizeText, idle),
        KeyBinding::new("tab", CopyColor, idle),
        KeyBinding::new("shift-tab", CopyTextColor, idle),
        KeyBinding::new("c", CopyAverageColor, sel),
        KeyBinding::new("p", ToggleUnit, idle),
        KeyBinding::new("k", ToggleBackdrop, tools),
    ];
    if cfg!(windows) {
        keys.push(KeyBinding::new("ctrl-y", Redo, base));
        keys.push(KeyBinding::new("f11", EnterFullScreen, base));
    }
    macro_rules! tool_keys {
        ($($tool:ident => $action:ident),*) => {
            $(keys.push(KeyBinding::new(Tool::$tool.key(), $action, tools));)*
        };
    }
    tool_keys!(Select => ToolSelect, Arrow => ToolArrow, Text => ToolText, Rect => ToolRect,
        Oval => ToolOval, Line => ToolLine, Pen => ToolPen, Highlighter => ToolHighlighter,
        Counter => ToolCounter, Redact => ToolRedact, Spotlight => ToolSpotlight, Magnifier => ToolMagnifier);
    cx.bind_keys(keys);
}

const TOOLBAR_H: f32 = 52.;
/// What Windows' caption buttons and room to drag add to the toolbar, net of the room macOS keeps for its own.
const CAPTION_W: f32 = if cfg!(windows) { 168. } else { 0. };
const FIT_MARGIN: f32 = 28.;
/// Canvas padding once objects grow it, in points.
const GROW_PAD: f32 = 20.;
const HANDLE: f32 = 5.;
/// How far from a selection edge it can still be grabbed, in points.
const GRAB: f32 = 5.;
/// How close an edge must come to a snap line, in screen points.
const SNAP: f32 = 5.;

actions!(kiri, [OpenSettings, Quit, OpenFile, OpenClipboard, CloseAll]);
// A placeholder: AppKit takes the item over (see `mac::adopt_full_screen_item`).
actions!(kiri, [EnterFullScreen]);

/// Editors make Kiri a regular app with a menu bar; with none open it goes back to the tray.
#[derive(Default)]
struct OpenEditors(usize);

impl Global for OpenEditors {}

fn app_menus() -> Vec<Menu> {
    // gpui shows an action's first binding as the key equivalent. Actions bound to a bare
    // letter must be unavailable while typing (see `render`), or their items would eat it.
    vec![
        Menu::new("Kiri").items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Quit Kiri", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Copy Image", CopyImage),
            MenuItem::action("Save Image", Save),
            MenuItem::action("Save Image As…", SaveAs),
            MenuItem::action("Pin to Screen", Pin),
            MenuItem::action("Print…", Print),
            MenuItem::separator(),
            MenuItem::action("Open File…", OpenFile),
            MenuItem::action("Open From Clipboard", OpenClipboard),
            MenuItem::separator(),
            MenuItem::action("Close", Close),
            MenuItem::action("Close All", CloseAll),
        ]),
        Menu::new("Edit").items([
            MenuItem::action("Undo", Undo),
            MenuItem::action("Redo", Redo),
            MenuItem::action("Paste", Paste),
            MenuItem::separator(),
            MenuItem::action("Recognize Text or QR", RecognizeText),
            MenuItem::action("Reset Crop", ResetCrop),
            MenuItem::action("Rasterize Annotations", Rasterize),
        ]),
        Menu::new("Draw").items(TOOL_ACTIONS.iter().map(|(tool, action)| MenuItem::Action {
            name: tool.name().into(),
            action: action(),
            os_action: None,
            checked: false,
            disabled: false,
        }).chain([MenuItem::action("Backdrop", ToggleBackdrop), MenuItem::action("Add Capture", AddCapture)])),
        Menu::new("Zoom").items([
            MenuItem::action("Zoom In", ZoomIn),
            MenuItem::action("Zoom Out", ZoomOut),
            MenuItem::separator(),
            MenuItem::action("Zoom to Fit", ZoomFit),
            MenuItem::action("Zoom to 100%", ZoomActual),
            MenuItem::separator(),
            MenuItem::action("Zoom to Selection", ZoomSelection),
            MenuItem::action("Selection Top Left", SelectionTopLeft),
            MenuItem::action("Selection Bottom Right", SelectionBottomRight),
            MenuItem::separator(),
            MenuItem::action(crate::platform_text("Enter Full Screen", "Full Screen"), EnterFullScreen),
        ]),
    ]
}

pub fn open(image: TiledImage, scale: f32, display: Option<u32>, file: Option<PathBuf>, cx: &mut App) {
    open_with(image, scale, display, cx, move |editor, _, _| editor.file = file);
}

/// What the last closed editor held, for Reopen. The image waits as a PNG so the
/// resident app doesn't keep a screenshot's worth of pixels.
struct Closed {
    png: Vec<u8>,
    objects: Rc<Vec<Object>>,
    crop: Option<R>,
    file: Option<PathBuf>,
    saved_to: Option<PathBuf>,
    backdrop: Option<crate::backdrop::Backdrop>,
}

#[derive(Default)]
struct LastClosed(Option<Closed>);

impl Global for LastClosed {}

pub fn reopen_last(cx: &mut App) {
    let Some(last) = cx.default_global::<LastClosed>().0.take() else {
        crate::toast::show("No screenshot to reopen yet", cx);
        return;
    };
    let Some((image, scale)) = export::decode(&last.png) else { return };
    let Closed { objects, crop, file, saved_to, backdrop, .. } = last;
    reopen(Document { image, scale, objects, crop, file, saved_to, backdrop }, cx);
}

/// An editor's content with its objects still editable, for a pin's Edit.
pub struct Document {
    image: TiledImage,
    scale: f32,
    objects: Rc<Vec<Object>>,
    crop: Option<R>,
    file: Option<PathBuf>,
    saved_to: Option<PathBuf>,
    backdrop: Option<crate::backdrop::Backdrop>,
}

pub fn reopen(doc: Document, cx: &mut App) {
    let Document { image, scale, objects, crop, file, saved_to, backdrop } = doc;
    open_with(image, scale, None, cx, move |editor, window, cx| {
        let snapshot = Snapshot { image: editor.image.clone(), objects, crop };
        editor.restore(snapshot);
        editor.file = file;
        editor.saved_to = saved_to;
        editor.set_backdrop(backdrop.map(|b| crate::backdrop::Backdrop { panel_open: false, ..b }), window, cx);
    });
}

fn remember_closed(editor: &Editor, cx: &mut App) {
    let (image, scale) = (editor.image.clone(), editor.scale);
    let closed = Closed {
        png: Vec::new(),
        objects: editor.objects.clone(),
        crop: editor.crop,
        file: editor.file.clone(),
        saved_to: editor.saved_to.clone(),
        backdrop: editor.backdrop.clone(),
    };
    cx.spawn(async move |cx| {
        let png = cx
            .background_executor()
            .spawn(async move { export::encode_bgra(image.width, image.height, image.read(image.full()), scale) })
            .await;
        cx.update(|cx| cx.set_global(LastClosed(Some(Closed { png, ..closed }))));
    })
    .detach();
}

fn open_with(
    image: TiledImage,
    scale: f32,
    display: Option<u32>,
    cx: &mut App,
    setup: impl FnOnce(&mut Editor, &mut Window, &mut Context<Editor>) + 'static,
) {
    let display_id = display.map(|d| DisplayId::new(d as u64));
    let (dw, dh) = display.map(mac::display_points).unwrap_or((1440., 900.));
    let (iw, ih) = (image.width as f32 / scale, image.height as f32 / scale);
    let win = size(
        px((iw + FIT_MARGIN * 2.).min(dw * 0.86).max(940. + CAPTION_W)),
        px((ih + TOOLBAR_H + FIT_MARGIN * 2.).clamp(480., dh * 0.86)),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            display_id, win, cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("Kiri".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(18.), px(19.))),
        }),
        app_owns_titlebar_drag: true,
        window_min_size: Some(size(px(900. + CAPTION_W), px(380.))),
        display_id,
        focus: true,
        show: true,
        ..Default::default()
    };
    let opened = crate::open_window(options, cx, |window, cx| {
        let view = cx.new(|cx| {
            let mut editor = Editor::new(image, scale, window, cx);
            setup(&mut editor, window, cx);
            editor
        });
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
        mac::set_on_top(window, settings::get(cx).editor_on_top);
        view
    });
    if let Err(err) = opened {
        eprintln!("kiri: editor window failed: {err:#}");
    }
    cx.activate(true);
}

/// Settings shares the editors' level, or an editor on top would cover it.
pub fn apply_on_top(cx: &mut App) {
    let on = settings::get(cx).editor_on_top;
    for w in cx.windows() {
        if editor_in(w, cx).is_some() || cx.try_global::<crate::settings_window::Open>().is_some_and(|o| o.0 == w) {
            let _ = w.update(cx, |_, window, _| mac::set_on_top(window, on));
        }
    }
}

#[derive(Clone)]
struct Snapshot {
    image: TiledImage,
    objects: Rc<Vec<Object>>,
    crop: Option<R>,
}

#[derive(Clone, Copy)]
enum SelGrab {
    Move,
    Handle(u8),
}

enum Drag {
    Pan { last: Point<Pixels> },
    Create { id: u64, start: P },
    /// `slip` is how far snapping has pulled the object off the cursor, so it lets go again.
    Move { id: u64, part: Part, last: P, slip: P },
    Handle { id: u64, handle: Handle },
    Select { start: P },
    /// `off` is where the grab sits relative to the grabbed edge, so the edge doesn't jump.
    Selection { grab: SelGrab, last: P, off: P },
    /// Selecting characters in the label being edited.
    Text,
}


#[derive(Clone, Copy, PartialEq)]
enum Axis {
    Horizontal,
    Vertical,
}

/// The visible image with every pixel repeated `factor` times, so high zoom shows crisp pixels:
/// GPUI only samples textures linearly.
struct Crisp {
    at: PxRect,
    factor: u32,
    image: Arc<RenderImage>,
}

/// Zoom, in device pixels per image pixel, from which the image is drawn crisp.
const CRISP_FROM: f32 = 3.;

fn crisp_image(image: &TiledImage, view: View, bounds: Bounds<Pixels>, mag: f32, cache: &RefCell<Option<Crisp>>, stale: &RefCell<Vec<Arc<RenderImage>>>) -> Option<(PxRect, Arc<RenderImage>)> {
    let factor = mag.ceil() as u32;
    // Round out to a grid, so small pans reuse the copy.
    let grid = (256 / factor).max(2) as f32;
    let (a, b) = (view.to_image(bounds.origin), view.to_image(bounds.bottom_right()));
    let x0 = ((a.0 / grid).floor() * grid).clamp(0., image.width as f32);
    let y0 = ((a.1 / grid).floor() * grid).clamp(0., image.height as f32);
    let x1 = ((b.0 / grid).ceil() * grid).clamp(0., image.width as f32);
    let y1 = ((b.1 / grid).ceil() * grid).clamp(0., image.height as f32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let at = PxRect { x: x0 as u32, y: y0 as u32, w: (x1 - x0) as u32, h: (y1 - y0) as u32 };
    let mut cache = cache.borrow_mut();
    if let Some(c) = cache.as_ref()
        && c.at == at
        && c.factor == factor
    {
        return Some((at, c.image.clone()));
    }
    let src = image.read(at);
    let (w, f) = (at.w as usize, factor as usize);
    let row = w * f * 4;
    let mut out = vec![0u8; row * at.h as usize * f];
    for y in 0..at.h as usize {
        let first = y * f * row;
        for x in 0..w {
            let px = &src[(y * w + x) * 4..][..4];
            for i in 0..f {
                out[first + (x * f + i) * 4..][..4].copy_from_slice(px);
            }
        }
        for i in 1..f {
            out.copy_within(first..first + row, first + i * row);
        }
    }
    let image = Arc::new(crate::tiled::frame_image(at.w * factor, at.h * factor, out));
    if let Some(old) = cache.replace(Crisp { at, factor, image: image.clone() }) {
        stale.borrow_mut().push(old.image);
    }
    Some((at, image))
}

/// What a cached patch was built from; a mismatch rebuilds it.
#[derive(PartialEq)]
enum Source {
    Pixels(doc::PixelOp, [i32; 4]),
    Highlight(u64),
    Image(*const doc::BitmapData, u32),
}

struct CachedPatch {
    key: Source,
    at: R,
    image: Arc<RenderImage>,
}

#[derive(Clone)]
struct DragOut(WeakEntity<Editor>);

struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

pub struct Editor {
    image: TiledImage,
    scale: f32,
    objects: Rc<Vec<Object>>,
    crop: Option<R>,
    /// Grown canvas fill; `None` keeps it transparent.
    fill: Option<u32>,
    next_id: u64,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    pending: Option<Snapshot>,
    tool: Tool,
    styles: std::collections::BTreeMap<Tool, Style>,
    selected: Option<u64>,
    selection: Option<R>,
    drag: Option<Drag>,
    editing: Option<TextEdit>,
    /// The ruler axes held down; both at once measure an element's width and height.
    ruler: Vec<Axis>,
    space: bool,
    unit_px: bool,
    text_lines: Option<Rc<[R]>>,
    contrast: Cell<Option<(PxRect, Option<(u32, u32)>)>>,
    props_collapsed: bool,
    finding_text: bool,
    zoom: Option<f32>,
    pan: Point<Pixels>,
    canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    cursor: Option<P>,
    last_move: Option<Point<Pixels>>,
    modifiers: Modifiers,
    /// Where an `Option`-drag selection lands on release.
    snap: Option<R>,
    /// Lines the dragged object is snapped to: vertical or not, and where, in image pixels.
    guides: Vec<(bool, f32)>,
    patches: Rc<RefCell<HashMap<u64, CachedPatch>>>,
    stale: Rc<RefCell<Vec<Arc<RenderImage>>>>,
    crisp: Rc<RefCell<Option<Crisp>>>,
    slider: Entity<SliderState>,
    color: Entity<ColorPickerState>,
    /// Width and height fields of the resize popover, in pixels.
    resize: (Entity<InputState>, Entity<InputState>),
    /// Text, an optional button, and whether it reports a success (drawn with a check).
    toast: Option<(SharedString, Option<ToastAction>, bool)>,
    toast_task: Option<Task<()>>,
    saved_to: Option<PathBuf>,
    /// The file this editor was opened from; Save As proposes its name.
    file: Option<PathBuf>,
    pub backdrop: Option<crate::backdrop::Backdrop>,
    backdrop_panel: Option<Entity<crate::backdrop::Panel>>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

fn measure(text: &str, size: f32, bold: bool) -> f32 {
    mac::text_width(text, size, bold)
}

/// Moves the selection by `d`, kept inside the canvas. It can be larger than the canvas
/// after an undo shrinks it, and then it stays pinned to the top-left.
fn slide(r: R, d: P, c: R) -> R {
    let x = (r.x + d.0).min(c.right() - r.w).max(c.x);
    let y = (r.y + d.1).min(c.bottom() - r.h).max(c.y);
    R::new(x, y, r.w, r.h)
}

/// Where offset `i` of a label sits: the x of the caret and the top of its line, in image space.
fn label_point(o: &Object, text: &str, r: R, i: usize) -> P {
    let (pad_x, pad_y) = o.label_padding();
    let s = te::line_start(text, i);
    let line = text[..s].matches('\n').count();
    let x = r.x + pad_x + measure(&text[s..i], o.size, o.variant == 0);
    (x, r.y + pad_y + line as f32 * o.size * doc::LINE_HEIGHT)
}

/// One rectangle per line that `range` covers, for selection and composition marks.
fn label_runs(o: &Object, text: &str, r: R, range: std::ops::Range<usize>) -> Vec<R> {
    let mut out = Vec::new();
    let mut s = range.start;
    while s < range.end {
        let e = te::line_end(text, s).min(range.end);
        let (a, b) = (label_point(o, text, r, s), label_point(o, text, r, e));
        out.push(R::new(a.0, a.1, b.0 - a.0, o.size * doc::LINE_HEIGHT));
        s = e + 1;
    }
    out
}

impl Editor {
    fn new(image: TiledImage, scale: f32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if cx.default_global::<OpenEditors>().0 == 0 {
            cx.set_activation_policy(ActivationPolicy::Regular);
            cx.set_menus(app_menus());
            mac::adopt_full_screen_item("Enter Full Screen");
        }
        cx.default_global::<OpenEditors>().0 += 1;
        cx.on_release(|editor, cx| {
            remember_closed(editor, cx);
            let open = &mut cx.default_global::<OpenEditors>().0;
            *open -= 1;
            if *open == 0 {
                cx.set_activation_policy(ActivationPolicy::Accessory);
            }
        })
        .detach();
        let fill = pixels::background_color(&image, image.full());
        let slider = cx.new(|_| SliderState::new().min(0.).max(1.).step(0.01).default_value(0.3));
        let sub = cx.subscribe_in(&slider, window, |this, _, e: &SliderEvent, _, cx| {
            if let SliderEvent::Change(v) = e {
                this.set_size_fraction(v.start(), cx);
            }
            if let SliderEvent::Release(_) = e {
                this.commit();
            }
        });
        let color = cx.new(|cx| ColorPickerState::new(window, cx));
        let pick = cx.subscribe_in(&color, window, |this, _, e: &ColorPickerEvent, _, cx| {
            if let ColorPickerEvent::Change(Some(c)) = e {
                let c = c.to_rgb();
                let ch = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
                this.apply_style(|s| s.color = ch(c.r) << 16 | ch(c.g) << 8 | ch(c.b), cx);
            }
        });
        // Slider drags emit a change per step; the picker closing ends one undo step.
        let close = cx.observe(&color, |this, color, cx| {
            if !color.read(cx).is_open() {
                this.commit();
            }
        });
        let resize = (cx.new(|cx| InputState::new(window, cx)), cx.new(|cx| InputState::new(window, cx)));
        let fields = [(resize.0.clone(), true), (resize.1.clone(), false)].map(|(field, is_w)| {
            cx.subscribe_in(&field, window, move |this, _, e: &InputEvent, window, cx| match e {
                InputEvent::Change => this.link_resize_field(is_w, window, cx),
                InputEvent::PressEnter { .. } => this.resize_to_fields(cx),
                _ => {}
            })
        });
        let styles = settings::get(cx).styles.clone();
        Self {
            image,
            scale,
            objects: Rc::default(),
            crop: None,
            fill,
            next_id: 1,
            undo: Vec::new(),
            redo: Vec::new(),
            pending: None,
            tool: Tool::Arrow,
            styles,
            selected: None,
            selection: None,
            drag: None,
            editing: None,
            ruler: Vec::new(),
            space: false,
            unit_px: settings::get(cx).unit_px,
            text_lines: None,
            contrast: Cell::new(None),
            props_collapsed: false,
            finding_text: false,
            zoom: None,
            pan: Point::default(),
            canvas_bounds: Rc::default(),
            cursor: None,
            last_move: None,
            modifiers: Modifiers::default(),
            snap: None,
            guides: Vec::new(),
            patches: Rc::default(),
            stale: Rc::default(),
            crisp: Rc::default(),
            slider,
            color,
            resize,
            toast: None,
            toast_task: None,
            saved_to: None,
            file: None,
            backdrop: None,
            backdrop_panel: None,
            focus: cx.focus_handle(),
            _subscriptions: [sub, pick, close].into_iter().chain(fields).collect(),
        }
    }

    // ---- document -------------------------------------------------------

    fn ctx(&self) -> Ctx<'static> {
        Ctx {
            measure: &measure,
            scale: self.scale,
            unit_px: self.unit_px,
            text: self.text_lines.clone(),
        }
    }

    fn image_rect(&self) -> R {
        R::new(0., 0., self.image.width as f32, self.image.height as f32)
    }

    /// The crop, or the whole image.
    fn base(&self) -> R {
        self.crop.unwrap_or_else(|| self.image_rect())
    }

    /// The crop or image plus the backdrop's inset.
    fn framed(&self) -> R {
        let inset = self.backdrop.as_ref().map_or(0., |b| b.inset);
        self.base().inflate(inset * self.scale)
    }

    /// The area that gets exported: the framed image, grown to fit objects.
    fn canvas(&self) -> R {
        let limit = self.image.width.max(self.image.height) as f32;
        doc::canvas(
            self.framed(),
            &self.objects,
            &self.ctx(),
            GROW_PAD * self.scale,
            limit,
        )
    }

    /// What the view fits and centers on. Annotations are left out, so a growing canvas never moves the view.
    fn view_area(&self) -> R {
        let base = self.framed();
        match &self.backdrop {
            Some(b) => b.outer(base, self.scale),
            None => base,
        }
    }

    /// The canvas with the backdrop around it, if one is on.
    fn outer(&self) -> R {
        let c = self.canvas();
        match &self.backdrop {
            Some(b) => b.outer(c, self.scale),
            None => c,
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            image: self.image.clone(),
            objects: self.objects.clone(),
            crop: self.crop,
        }
    }

    /// Grown canvas is filled with the background of what's left after cropping, as in the alternative.
    fn refresh_fill(&mut self) {
        let b = self.base().round_out();
        let r = PxRect { x: b.x as u32, y: b.y as u32, w: b.w as u32, h: b.h as u32 };
        self.fill = pixels::background_color(&self.image, r);
    }

    fn restore(&mut self, s: Snapshot) {
        let refill = s.crop != self.crop || !s.image.same_pixels(&self.image);
        self.image = s.image;
        self.objects = s.objects;
        // A reopened document brings its own ids; new objects must not reuse them.
        self.next_id = self.next_id.max(self.objects.iter().map(|o| o.id + 1).max().unwrap_or(1));
        self.crop = s.crop;
        if refill {
            self.refresh_fill();
        }
        if self
            .selected
            .is_some_and(|id| !self.objects.iter().any(|o| o.id == id))
        {
            self.selected = None;
        }
        self.editing = None;
    }

    fn begin(&mut self) {
        if self.pending.is_none() {
            self.pending = Some(self.snapshot());
        }
    }

    /// Pushes the state from `begin` to the undo stack if anything changed.
    fn commit(&mut self) {
        let Some(before) = self.pending.take() else {
            return;
        };
        let refill = before.crop != self.crop || !before.image.same_pixels(&self.image);
        if refill {
            self.refresh_fill();
        }
        if refill || *before.objects != *self.objects {
            self.undo.push(before);
            self.redo.clear();
        }
    }

    fn edit_doc(&mut self, f: impl FnOnce(&mut Self)) {
        self.begin();
        f(self);
        self.commit();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        self.commit();
        if let Some(s) = self.undo.pop() {
            let current = self.snapshot();
            self.restore(s);
            self.redo.push(current);
            cx.notify();
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.redo.pop() {
            let current = self.snapshot();
            self.restore(s);
            self.undo.push(current);
            cx.notify();
        }
    }

    fn object(&self, id: u64) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    fn object_mut(&mut self, id: u64) -> Option<&mut Object> {
        Rc::make_mut(&mut self.objects).iter_mut().find(|o| o.id == id)
    }

    fn add(&mut self, shape: Shape, style: Style) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let size = style.size * self.scale;
        Rc::make_mut(&mut self.objects).push(Object {
            id,
            shape,
            color: style.color,
            size,
            variant: style.variant,
        });
        id
    }

    fn style(&self, tool: Tool) -> Style {
        self.styles
            .get(&tool)
            .copied()
            .unwrap_or_else(|| tool.default_style())
    }

    /// The tool whose style the property bar shows: the selected object's, or the active tool's.
    fn styled_tool(&self) -> Option<Tool> {
        match self.selected.and_then(|id| self.object(id)) {
            Some(o) => o.shape.tool(),
            None => Some(self.tool),
        }
    }

    fn current_style(&self) -> Option<Style> {
        if let Some(o) = self.selected.and_then(|id| self.object(id)) {
            o.shape.tool()?;
            return Some(Style {
                color: o.color,
                size: o.size / self.scale,
                variant: o.variant,
            });
        }
        Some(self.style(self.tool))
    }

    fn apply_style(&mut self, f: impl Fn(&mut Style), cx: &mut Context<Self>) {
        let Some(tool) = self.styled_tool() else {
            return;
        };
        let Some(mut style) = self.current_style() else {
            return;
        };
        f(&mut style);
        self.styles.insert(tool, style);
        settings::update(cx, |s| s.styles.insert(tool, style));
        if let Some(id) = self.selected {
            let scale = self.scale;
            self.begin();
            if let Some(o) = self.object_mut(id) {
                o.color = style.color;
                o.variant = style.variant;
                o.size = style.size * scale;
            }
        }
        cx.notify();
    }

    fn set_size_fraction(&mut self, t: f32, cx: &mut Context<Self>) {
        let Some((lo, hi, _)) = self.styled_tool().and_then(|t| t.size_range()) else {
            return;
        };
        let size = lo + (hi - lo) * t * t;
        self.apply_style(|s| s.size = (size * 2.).round() / 2., cx);
    }

    fn sync_slider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(tool), Some(style)) = (self.styled_tool(), self.current_style()) else {
            return;
        };
        if let Some((lo, hi, _)) = tool.size_range() {
            let t = ((style.size - lo) / (hi - lo)).clamp(0., 1.).sqrt();
            self.slider.update(cx, |s, cx| s.set_value(t, window, cx));
        }
        self.color.update(cx, |s, cx| s.set_value(rgb(style.color), window, cx));
    }

    fn set_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_text();
        self.tool = tool;
        self.selected = None;
        if tool != Tool::Select {
            self.selection = None;
        }
        self.sync_slider(window, cx);
        cx.notify();
    }

    fn select(&mut self, id: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = id;
        self.sync_slider(window, cx);
        cx.notify();
    }

    // ---- view -----------------------------------------------------------

    fn view(&self) -> View {
        compute_view(self.canvas_bounds.get(), self.view_area(), self.scale, self.zoom, self.pan)
    }

    /// Centers `p` at 1600%, close enough to place a selection corner on an exact pixel.
    fn inspect(&mut self, p: P, cx: &mut Context<Self>) {
        let b = self.canvas_bounds.get();
        let k = 16. / self.scale;
        let centered = centered_origin(b, self.view_area(), k);
        let c = b.center();
        self.pan = point(c.x - px(p.0 * k) - centered.x, c.y - px(p.1 * k) - centered.y);
        self.zoom = Some(k);
        cx.notify();
    }

    fn zoom_at(&mut self, k: f32, anchor: Point<Pixels>, cx: &mut Context<Self>) {
        let k = k.clamp(0.02, 32. / self.scale);
        let p = self.view().to_image(anchor);
        let centered = centered_origin(self.canvas_bounds.get(), self.view_area(), k);
        let origin = point(anchor.x - px(p.0 * k), anchor.y - px(p.1 * k));
        self.pan = point(origin.x - centered.x, origin.y - centered.y);
        self.zoom = Some(k);
        cx.notify();
    }

    fn step_zoom(&mut self, factor: f32, cx: &mut Context<Self>) {
        let anchor = self
            .cursor
            .map(|p| self.view().to_screen(p))
            .filter(|p| self.canvas_bounds.get().contains(p))
            .unwrap_or_else(|| self.canvas_bounds.get().center());
        self.zoom_at(self.view().k * factor, anchor, cx);
    }

    fn fit(&mut self, cx: &mut Context<Self>) {
        self.zoom = None;
        self.pan = Point::default();
        cx.notify();
    }

    /// Zoom to Fit also takes in annotations past the image edge.
    fn fit_all(&mut self, cx: &mut Context<Self>) {
        let b = self.canvas_bounds.get();
        let (all, area) = (self.outer(), self.view_area());
        let k = fit_k(b, all, self.scale);
        let (target, centered) = (centered_origin(b, all, k), centered_origin(b, area, k));
        self.zoom = Some(k);
        self.pan = point(target.x - centered.x, target.y - centered.y);
        cx.notify();
    }

    // ---- output ---------------------------------------------------------

    fn render_pixmap(&self, region: Option<R>, objects: &[Object]) -> tiny_skia::Pixmap {
        let started = Instant::now();
        let canvas = self.canvas().round_out();
        let ctx = self.ctx();
        let pixmap = match &self.backdrop {
            Some(b) if region.is_none() => {
                let inner = self.render_scene(canvas, &ctx, objects);
                b.compose(&inner, canvas, self.scale, &self.image)
            }
            _ => {
                let area = region.map_or(canvas, |r| r.intersect(&canvas).unwrap_or(canvas).round_out());
                self.render_scene(area, &ctx, objects)
            }
        };
        eprintln!(
            "kiri: render {}×{} px in {} ms",
            pixmap.width(),
            pixmap.height(),
            started.elapsed().as_millis()
        );
        pixmap
    }

    fn render_scene(&self, area: R, ctx: &Ctx, objects: &[Object]) -> tiny_skia::Pixmap {
        let crop = area
            .intersect(&self.base())
            .map(|r| r.round_out())
            .unwrap_or(R::new(0., 0., 1., 1.));
        export::render(&export::Scene {
            image: &self.image,
            crop: PxRect {
                x: crop.x as u32,
                y: crop.y as u32,
                w: crop.w as u32,
                h: crop.h as u32,
            },
            canvas: area,
            fill: self.fill,
            objects,
            cx: ctx,
        })
    }

    fn png(&self, cx: &App) -> Vec<u8> {
        let started = Instant::now();
        let (pixmap, scale) = self.export_pixmap(&self.objects, cx);
        let png = export::encode_png(&pixmap, scale);
        eprintln!(
            "kiri: PNG {} KB in {} ms",
            png.len() / 1024,
            started.elapsed().as_millis()
        );
        png
    }

    /// What Copy and Save write, honoring "downscale to 1×".
    fn export_pixmap(&self, objects: &[Object], cx: &App) -> (tiny_skia::Pixmap, f32) {
        let pixmap = self.render_pixmap(self.selection, objects);
        match settings::get(cx).downscale_retina && self.scale > 1. {
            true => (export::downscale(&pixmap, self.scale), 1.),
            false => (pixmap, self.scale),
        }
    }

    /// Saves a GIF that alternates between the image object `id` and what lies under it,
    /// as the alternative's overlay does.
    fn save_gif(&mut self, id: u64, cx: &mut Context<Self>) {
        self.finish_text();
        let under: Vec<Object> = self.objects.iter().filter(|o| o.id != id).cloned().collect();
        let frames = [self.export_pixmap(&self.objects, cx).0, self.export_pixmap(&under, cx).0];
        let path = settings::get(cx).save_path().with_extension("gif");
        let write = cx.background_spawn({
            let path = path.clone();
            async move {
                let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
                export::encode_gif(&frames, 1000).and_then(|gif| std::fs::write(&path, gif).map_err(|e| e.to_string()))
            }
        });
        self.show_toast("Saving GIF…", None, cx);
        cx.spawn(async move |this, cx| {
            let result = write.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    let folder = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
                    this.toast_done(format!("Saved GIF to {}", folder.unwrap_or_default()), Some(ToastAction::Reveal(path)), cx);
                }
                Err(err) => this.show_toast(format!("Couldn't save the GIF: {err}"), None, cx),
            });
        })
        .detach();
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        self.finish_text();
        let (pixmap, scale) = self.export_pixmap(&self.objects, cx);
        copy_later(move || export::encode_png(&pixmap, scale), cx);
    }

    fn copy_and_save_close(&mut self, copy: bool, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_text();
        let pixmap = Arc::new(self.export_pixmap(&self.objects, cx));
        if save {
            let path = self.saved_to.clone().unwrap_or_else(|| settings::get(cx).save_path());
            let write = write_later(pixmap.clone(), path, cx);
            cx.spawn(async move |_, cx| {
                let result = write.await;
                cx.update(|cx| match result {
                    Ok(path) => crate::toast::saved(path, cx),
                    Err(err) => crate::toast::show(format!("Couldn't save: {err}"), cx),
                });
            })
            .detach();
        }
        if copy {
            copy_later(move || export::encode_png(&pixmap.0, pixmap.1), cx);
        }
        window.remove_window();
    }

    /// As in the alternative, ⌘C closes the editor only when nothing is selected. A selected
    /// object is copied as a marker text, so copying anything else wins on paste; a selection
    /// copies its pixels and is dropped.
    fn copy_object_or_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.selected.and_then(|id| self.object(id)).cloned() {
            Some(object) if self.editing.is_none() => {
                let marker = format!("Kiri {}", object.shape.tool().map_or("Measurement", Tool::name));
                cx.write_to_clipboard(ClipboardItem::new_string(marker.clone()));
                cx.set_global(CopiedObject { object, marker });
            }
            None if self.selection.is_some() => {
                self.copy(cx);
                self.selection = None;
                cx.notify();
            }
            _ => self.copy_and_close(window, cx),
        }
    }

    fn copy_and_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.copy_and_save_close(true, false, window, cx);
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.finish_text();
        let path = match &self.saved_to {
            Some(p) => p.clone(),
            None => settings::get(cx).save_path(),
        };
        self.write_file(path, cx);
    }

    fn save_as(&mut self, cx: &mut Context<Self>) {
        self.finish_text();
        let (dir, name) = match self.saved_to.as_ref().or(self.file.as_ref()) {
            Some(f) => (f.parent().map(PathBuf::from).unwrap_or_default(), f.file_name().map(|n| n.to_string_lossy().into_owned())),
            None => (settings::get(cx).save_dir(), None),
        };
        let name = name.unwrap_or_else(|| {
            let path = crate::settings::timestamp_path(&dir, &settings::get(cx).file_name);
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        });
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                let _ = this.update(cx, |this, cx| this.write_file(path, cx));
            }
        })
        .detach();
    }

    /// Writes in the background. `saved_to` is set at once so a quick second Save reuses the name.
    fn write_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.saved_to = Some(path.clone());
        let write = write_later(Arc::new(self.export_pixmap(&self.objects, cx)), path, cx);
        cx.spawn(async move |this, cx| {
            let result = write.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(path) => {
                    let folder = path
                        .parent()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    this.saved_to = Some(path.clone());
                    if settings::get(cx).confirmations {
                        this.toast_done(format!("Saved to {folder}"), Some(ToastAction::Reveal(path)), cx);
                    }
                }
                Err(err) => this.show_toast(format!("Couldn't save: {err}"), None, cx),
            });
        })
        .detach();
    }


    fn export_temp(&self, cx: &App) -> Option<PathBuf> {
        write_temp(&self.png(cx), &settings::get(cx).file_name)
    }

    fn pin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_text();
        let pixmap = self.render_pixmap(self.selection, &self.objects);
        let (w, h) = (pixmap.width(), pixmap.height());
        let mut bgra = pixmap.take();
        for p in bgra.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        let image = TiledImage::from_bgra(w, h, &bgra, (w * 4) as usize);
        drop(bgra);
        let origin = window.bounds().origin;
        let doc = Document {
            image: self.image.clone(),
            scale: self.scale,
            objects: self.objects.clone(),
            crop: self.crop,
            file: self.file.clone(),
            saved_to: self.saved_to.clone(),
            backdrop: self.backdrop.clone(),
        };
        crate::pin::open(image, self.scale, origin, Some(doc), cx);
        window.remove_window();
    }

    fn show_toast(&mut self, text: impl Into<SharedString>, action: Option<ToastAction>, cx: &mut Context<Self>) {
        self.toast_with(text, action, false, cx);
    }

    fn toast_done(&mut self, text: impl Into<SharedString>, action: Option<ToastAction>, cx: &mut Context<Self>) {
        self.toast_with(text, action, true, cx);
    }

    fn toast_with(&mut self, text: impl Into<SharedString>, action: Option<ToastAction>, done: bool, cx: &mut Context<Self>) {
        let ms = if action.is_some() { 4000 } else { 2200 };
        self.toast = Some((text.into(), action, done));
        self.toast_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ms))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.toast = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn copy_text(&mut self, text: String, toast: impl Into<SharedString>, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast_done(toast, None, cx);
    }

    // ---- selection (crop) -----------------------------------------------

    fn selection_px(&self) -> Option<PxRect> {
        let r = self.selection?.intersect(&self.image_rect())?.round_out();
        (r.w >= 1. && r.h >= 1.).then(|| PxRect {
            x: r.x as u32,
            y: r.y as u32,
            w: r.w as u32,
            h: r.h as u32,
        })
    }

    fn crop_to_selection(&mut self, cx: &mut Context<Self>) {
        let Some(sel) = self.selection.take() else {
            return;
        };
        let sel = sel.round_out();
        self.edit_doc(|this| this.crop = Some(sel));
        self.fit(cx);
    }

    fn reset_crop(&mut self, cx: &mut Context<Self>) {
        if self.crop.is_some() {
            self.edit_doc(|this| this.crop = None);
            self.fit(cx);
        }
    }

    /// Crops to the content, so the backdrop's inset sits evenly around it.
    pub fn recenter(&mut self, cx: &mut Context<Self>) {
        let Some(b) = self.base().intersect(&self.image_rect()).map(|r| r.round_out()) else { return };
        let r = pixels::autoadjust(&self.image, PxRect { x: b.x as u32, y: b.y as u32, w: b.w as u32, h: b.h as u32 });
        let r = R::new(r.x as f32, r.y as f32, r.w as f32, r.h as f32);
        if r != b {
            self.edit_doc(|this| this.crop = Some(r));
            self.fit(cx);
        }
    }

    fn autoadjusted(&self) -> Option<R> {
        let r = pixels::autoadjust(&self.image, self.selection_px()?);
        Some(R::new(r.x as f32, r.y as f32, r.w as f32, r.h as f32))
    }

    fn autoadjust(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = self.autoadjusted() {
            self.selection = Some(r);
            cx.notify();
        }
    }

    fn resize_selection(&mut self, d: f32, cx: &mut Context<Self>) {
        if let Some(sel) = self.selection {
            let grown = sel.inflate(d * self.scale);
            if grown.w >= 2. && grown.h >= 2. {
                self.selection = grown.intersect(&self.canvas());
                cx.notify();
            }
        }
    }

    fn erase_selection(&mut self, cx: &mut Context<Self>) {
        let Some(sel) = self.selection.take() else {
            return;
        };
        self.edit_doc(|this| {
            this.add(
                Shape::Redact {
                    a: (sel.x, sel.y),
                    b: (sel.right(), sel.bottom()),
                },
                Style {
                    color: 0,
                    size: 1.,
                    variant: 2,
                },
            );
        });
        cx.notify();
    }

    fn imprint_size(&mut self, cx: &mut Context<Self>) {
        let Some(sel) = self.selection.take() else {
            return;
        };
        let gap = 10. * self.scale;
        let style = Style {
            color: doc::MEASURE,
            size: 1.,
            variant: 0,
        };
        self.edit_doc(|this| {
            let y = if sel.y - gap > this.image_rect().y { sel.y - gap } else { sel.bottom() + gap };
            this.add(
                Shape::Measure {
                    from: (sel.x, y),
                    to: (sel.right(), y),
                },
                style,
            );
            let x = if sel.right() + gap < this.image_rect().right() { sel.right() + gap } else { sel.x - gap };
            this.add(
                Shape::Measure {
                    from: (x, sel.y),
                    to: (x, sel.bottom()),
                },
                style,
            );
        });
        cx.notify();
    }

    fn recognize(&mut self, cx: &mut Context<Self>) {
        let rect = self.selection_px().unwrap_or(self.image.full());
        crate::ocr::recognize_and_copy(self.image.read(rect), rect.w, rect.h, cx);
    }

    // ---- text editing ---------------------------------------------------

    fn finish_text(&mut self) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        let empty = matches!(self.object(edit.id).map(|o| &o.shape), Some(Shape::Text { text, .. }) if text.trim().is_empty());
        if empty {
            Rc::make_mut(&mut self.objects).retain(|o| o.id != edit.id);
            if self.selected == Some(edit.id) {
                self.selected = None;
            }
        }
        self.commit();
    }

    fn start_text(&mut self, id: u64, cx: &mut Context<Self>) {
        self.begin();
        let caret = match self.object(id).map(|o| &o.shape) {
            Some(Shape::Text { text, .. }) => text.len(),
            _ => return,
        };
        self.selected = Some(id);
        self.editing = Some(TextEdit::new(id, caret));
        cx.notify();
    }

    /// Editing commands for the label being typed. Printable keys fall through to the
    /// input handler (`EntityInputHandler`), so input methods and dead keys compose.
    fn text_key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let Some((_, o, text)) = self.edited_text() else {
            return false;
        };
        let (size, bold, mut text) = (o.size, o.variant == 0, text.to_string());
        let edit = self.editing.as_mut().unwrap();
        let k = &e.keystroke;
        let m = k.modifiers;
        // Windows takes Ctrl for clipboard keys and word moves, and Home and End for lines.
        let cmd = m.platform || cfg!(windows) && m.control;
        let (word, line) = if cfg!(windows) { (m.control, false) } else { (m.alt, m.platform) };
        let (sel, c, t) = (edit.selection(), edit.caret, text.as_str());
        let width = |s: &str| measure(s, size, bold);
        match k.key.as_str() {
            "enter" if cmd => {
                self.finish_text();
                cx.notify();
                return true;
            }
            "enter" if !m.control => edit.replace(&mut text, sel, "\n"),
            "backspace" | "delete" => {
                let back = k.key == "backspace";
                let range = if !sel.is_empty() {
                    sel
                } else if back {
                    (if line { te::line_start(t, c) } else if word { te::prev_word(t, c) } else { te::prev_char(t, c) })..c
                } else {
                    c..if line { te::line_end(t, c) } else if word { te::next_word(t, c) } else { te::next_char(t, c) }
                };
                edit.replace(&mut text, range, "");
            }
            "left" | "right" => {
                let left = k.key == "left";
                let to = if line {
                    if left { te::line_start(t, c) } else { te::line_end(t, c) }
                } else if word {
                    if left { te::prev_word(t, c) } else { te::next_word(t, c) }
                } else if !sel.is_empty() && !m.shift {
                    if left { sel.start } else { sel.end }
                } else if left {
                    te::prev_char(t, c)
                } else {
                    te::next_char(t, c)
                };
                edit.go(to, m.shift);
            }
            "up" | "down" => {
                let down = k.key == "down";
                let to = if line { if down { t.len() } else { 0 } } else { te::vertical(t, c, down, width) };
                edit.go(to, m.shift);
            }
            "home" | "end" if cfg!(windows) && !m.control => {
                edit.go(if k.key == "home" { te::line_start(t, c) } else { te::line_end(t, c) }, m.shift)
            }
            "home" => edit.go(0, m.shift),
            "end" => edit.go(t.len(), m.shift),
            "a" if cmd => {
                edit.anchor = 0;
                edit.caret = t.len();
            }
            "c" | "x" if cmd => {
                if !sel.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(t[sel.clone()].to_string()));
                    if k.key == "x" {
                        edit.replace(&mut text, sel, "");
                    }
                }
            }
            "v" if cmd => {
                if let Some(s) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    edit.replace(&mut text, sel, &s);
                }
            }
            _ => return false,
        }
        self.set_text(text, cx);
        true
    }

    fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(id) = self.editing.as_ref().map(|e| e.id) else {
            return;
        };
        if let Some(o) = self.object_mut(id)
            && let Shape::Text { text: t, .. } = &mut o.shape
        {
            *t = text;
        }
        cx.notify();
    }

    /// An input method's UTF-16 range in the edited label, defaulting to the composition, then
    /// the selection, as bytes, with a copy of the text to change.
    fn input_range(&self, utf16: Option<Range<usize>>) -> Option<(Range<usize>, String)> {
        let (edit, _, text) = self.edited_text()?;
        let range = match utf16 {
            Some(r) => te::from_utf16(text, r.start)..te::from_utf16(text, r.end),
            None => edit.marked.clone().unwrap_or(edit.selection()),
        };
        Some((range, text.to_string()))
    }

    fn edited_text(&self) -> Option<(&TextEdit, &Object, &str)> {
        let edit = self.editing.as_ref()?;
        let o = self.object(edit.id)?;
        let Shape::Text { text, .. } = &o.shape else {
            return None;
        };
        Some((edit, o, text))
    }

    /// The caret offset in the edited label nearest to an image point.
    fn text_index_at(&self, p: P) -> Option<usize> {
        let (_, o, text) = self.edited_text()?;
        let r = o.text_box(&self.ctx())?;
        let (pad_x, pad_y) = o.label_padding();
        let lines = text.split('\n').count();
        let line = (((p.1 - r.y - pad_y) / (o.size * doc::LINE_HEIGHT)).max(0.) as usize).min(lines - 1);
        let s = text.split('\n').take(line).map(|l| l.len() + 1).sum::<usize>();
        let bold = o.variant == 0;
        Some(s + te::index_at(&text[s..te::line_end(text, s)], p.0 - r.x - pad_x, |l| measure(l, o.size, bold)))
    }

    // ---- pointer --------------------------------------------------------

    fn hit(&self, p: P) -> Option<(u64, Part)> {
        let tol = 4. / self.view().k;
        let ctx = self.ctx();
        self.objects
            .iter()
            .rev()
            .find_map(|o| o.hit(p, tol, &ctx).map(|part| (o.id, part)))
    }

    fn handle_at(&self, screen: Point<Pixels>) -> Option<(u64, Handle)> {
        let id = self.selected?;
        let o = self.object(id)?;
        let view = self.view();
        o.handles().into_iter().find_map(|(h, p)| {
            let s = view.to_screen(p);
            ((s.x - screen.x).abs() <= px(HANDLE + 3.) && (s.y - screen.y).abs() <= px(HANDLE + 3.))
                .then_some((id, h))
        })
    }

    /// As in the alternative, the selection's edges and corners resize it; no handles cover the content.
    fn selection_grab(&self, screen: Point<Pixels>, p: P) -> Option<SelGrab> {
        let sel = self.selection?;
        let r = self.view().rect(sel);
        let zone = |v: Pixels, lo: Pixels, hi: Pixels| match () {
            _ if (v - lo).abs() <= px(GRAB) => Some(0),
            _ if (v - hi).abs() <= px(GRAB) => Some(2),
            _ if v > lo && v < hi => Some(1),
            _ => None,
        };
        let i = match (zone(screen.x, r.left(), r.right())?, zone(screen.y, r.top(), r.bottom())?) {
            (0, 0) => 0,
            (1, 0) => 1,
            (2, 0) => 2,
            (2, 1) => 3,
            (2, 2) => 4,
            (1, 2) => 5,
            (0, 2) => 6,
            (0, 1) => 7,
            _ => return sel.contains(p).then_some(SelGrab::Move),
        };
        Some(SelGrab::Handle(i))
    }

    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let view = self.view();
        let p = view.to_image(e.position);
        let shift = e.modifiers.shift;

        if self.space || e.button == MouseButton::Middle {
            self.drag = Some(Drag::Pan { last: e.position });
            cx.notify();
            return;
        }
        if !self.ruler.is_empty() {
            let measures = self.ruler_measures();
            if !measures.is_empty() {
                self.edit_doc(|this| {
                    for (from, to) in measures {
                        this.add(Shape::Measure { from, to }, Style { color: doc::MEASURE, size: 1., variant: 0 });
                    }
                });
                cx.notify();
            }
            return;
        }
        if let Some(edit) = &self.editing {
            let id = edit.id;
            let inside = self
                .object(id)
                .and_then(|o| o.text_box(&self.ctx()))
                .is_some_and(|r| r.contains(p));
            if inside {
                if let Some(i) = self.text_index_at(p)
                    && let Some((_, _, text)) = self.edited_text()
                {
                    let word = te::prev_word(text, te::next_char(text, i));
                    let word = (word, te::next_word(text, word));
                    let ed = self.editing.as_mut().unwrap();
                    ed.marked = None;
                    if e.click_count == 2 {
                        (ed.anchor, ed.caret) = word;
                    } else {
                        ed.go(i, shift);
                        self.drag = Some(Drag::Text);
                    }
                    cx.notify();
                }
                return;
            }
            self.finish_text();
        }
        if let Some((id, handle)) = self.handle_at(e.position) {
            if e.modifiers.platform
                && let Some(i) = [Handle::Start, Handle::End].iter().position(|h| *h == handle)
                && matches!(self.object(id).map(|o| &o.shape), Some(Shape::Line { .. }))
            {
                self.edit_doc(|this| {
                    if let Some(Shape::Line { caps, .. }) = this.object_mut(id).map(|o| &mut o.shape) {
                        caps[i] = (caps[i] + 1) % 3;
                    }
                });
                cx.notify();
                return;
            }
            self.begin();
            self.drag = Some(Drag::Handle { id, handle });
            cx.notify();
            return;
        }
        if self.tool == Tool::Select
            && let Some(grab) = self.selection_grab(e.position, p)
        {
            let off = match (grab, self.selection) {
                (SelGrab::Handle(i), Some(sel)) => {
                    let h = doc::box_points(sel)[i as usize];
                    (p.0 - h.0, p.1 - h.1)
                }
                _ => (0., 0.),
            };
            self.drag = Some(Drag::Selection { grab, last: p, off });
            cx.notify();
            return;
        }
        if let Some((id, part)) = self.hit(p).filter(|_| self.tool != Tool::Counter) {
            if e.click_count >= 2
                && matches!(self.object(id).map(|o| &o.shape), Some(Shape::Text { .. }))
            {
                self.start_text(id, cx);
                return;
            }
            self.begin();
            let id = if e.modifiers.alt {
                let mut copy = self.object(id).cloned().expect("hit object exists");
                copy.id = self.next_id;
                self.next_id += 1;
                let new_id = copy.id;
                Rc::make_mut(&mut self.objects).push(copy);
                new_id
            } else {
                id
            };
            self.select(Some(id), window, cx);
            self.drag = Some(Drag::Move { id, part, last: p, slip: (0., 0.) });
            return;
        }
        self.selected = None;
        let style = self.style(self.tool);
        let shape = match self.tool {
            Tool::Image => return,
            Tool::Select => {
                self.selection = None;
                self.drag = Some(Drag::Select { start: p });
                cx.notify();
                return;
            }
            Tool::Arrow => Shape::Arrow {
                from: p,
                to: p,
                bend: None,
            },
            Tool::Line => Shape::Line { from: p, to: p, caps: [0, 0] },
            Tool::Rect => Shape::Rect { a: p, b: p },
            Tool::Oval => Shape::Oval { a: p, b: p },
            Tool::Redact => Shape::Redact { a: p, b: p },
            Tool::Spotlight => Shape::Spotlight { a: p, b: p },
            Tool::Pen => Shape::Pen { points: vec![p] },
            Tool::Highlighter => Shape::Highlighter { points: vec![p, p] },
            Tool::Magnifier => Shape::Magnifier { src: p, radius: 1., lens: p, lens_radius: 3. },
            Tool::Text => {
                self.begin();
                let size = style.size * self.scale;
                let at = (p.0, p.1 - size * doc::LINE_HEIGHT / 2.);
                let id = self.add(
                    Shape::Text {
                        at,
                        text: String::new(),
                    },
                    style,
                );
                self.selected = Some(id);
                self.editing = Some(TextEdit::new(id, 0));
                self.sync_slider(window, cx);
                cx.notify();
                return;
            }
            Tool::Counter => {
                let n = doc::next_counter(&self.objects);
                let angle = (self.objects.iter().rev())
                    .find_map(|o| match o.shape {
                        Shape::Counter { angle, .. } => Some(angle),
                        _ => None,
                    })
                    .unwrap_or(COUNTER_TAIL);
                self.edit_doc(|this| {
                    let id = this.add(Shape::Counter { at: p, n, angle }, style);
                    this.selected = Some(id);
                });
                self.sync_slider(window, cx);
                cx.notify();
                return;
            }
        };
        let _ = shift;
        self.begin();
        let id = self.add(shape, style);
        self.selected = Some(id);
        self.sync_slider(window, cx);
        self.drag = Some(Drag::Create { id, start: p });
        cx.notify();
    }

    /// A modifier pressed or released mid-draw reshapes the object at once, as in the alternative.
    fn modifiers_changed(&mut self, e: &ModifiersChangedEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (m, was) = (e.modifiers, std::mem::replace(&mut self.modifiers, e.modifiers));
        if matches!(self.drag, Some(Drag::Select { .. })) {
            self.snap = m.alt.then(|| self.autoadjusted()).flatten();
            cx.notify();
        }
        let changed = (m.shift, m.alt, m.platform) != (was.shift, was.alt, was.platform);
        let (Some(Drag::Create { id, start }), Some(p), true) = (self.drag.as_ref(), self.cursor, changed) else {
            return;
        };
        let (id, start, scale) = (*id, *start, self.scale);
        if let Some(o) = self.object_mut(id) {
            shape_drag(&mut o.shape, start, p, m.shift, m.alt, m.platform, scale);
        }
        cx.notify();
    }

    fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        // While a button is held, GPUI replays the last move every frame with its stale
        // modifiers, which would undo a modifier pressed since.
        if e.pressed_button.is_some() && self.last_move == Some(e.position) {
            return;
        }
        self.last_move = Some(e.position);
        let view = self.view();
        let p = view.to_image(e.position);
        self.cursor = Some(p);
        let (shift, alt, cmd) = (e.modifiers.shift, e.modifiers.alt, e.modifiers.platform);
        let canvas = self.canvas();
        let tol = SNAP / view.k;
        self.guides.clear();
        match self.drag.as_mut() {
            Some(Drag::Text) => {
                if let Some(i) = self.text_index_at(p)
                    && let Some(edit) = self.editing.as_mut()
                {
                    edit.go(i, true);
                }
            }
            Some(Drag::Pan { last }) => {
                self.pan = self.pan + (e.position - *last);
                *last = e.position;
                if self.zoom.is_none() {
                    self.zoom = Some(view.k);
                }
            }
            Some(Drag::Create { id, start }) => {
                let (id, start) = (*id, *start);
                let scale = self.scale;
                let p = if shift || cmd { p } else { self.snap_point(id, p, tol) };
                if let Some(o) = self.object_mut(id) {
                    shape_drag(&mut o.shape, start, p, shift, alt, cmd, scale);
                }
            }
            Some(Drag::Move { id, part, last, slip }) => {
                let (id, part) = (*id, *part);
                let raw = (p.0 - last.0 + slip.0, p.1 - last.1 + slip.1);
                *last = p;
                let fix = if part == Part::Body && !shift && !cmd { self.snap_move(id, raw, tol) } else { (0., 0.) };
                if let Some(Drag::Move { slip, .. }) = self.drag.as_mut() {
                    *slip = (-fix.0, -fix.1);
                }
                if let Some(o) = self.object_mut(id) {
                    o.shape.translate((raw.0 + fix.0, raw.1 + fix.1), part);
                }
            }
            Some(Drag::Handle { id, handle }) => {
                let (id, handle) = (*id, *handle);
                // Line handles sit past the ends and a highlighter's end draws on, so neither is an edge.
                let snaps = matches!(handle, Handle::Box(_) | Handle::Start | Handle::End)
                    && !self.object(id).is_some_and(|o| matches!(o.shape, Shape::Line { .. } | Shape::Highlighter { .. }));
                let p = if snaps && !shift && !cmd { self.snap_point(id, p, tol) } else { p };
                if let Some(o) = self.object_mut(id) {
                    o.drag_handle(handle, p, shift);
                }
            }
            Some(Drag::Select { start }) => {
                let b = if shift { doc::square(*start, p) } else { p };
                self.selection = R::from_points(*start, b).intersect(&canvas);
                self.snap = alt.then(|| self.autoadjusted()).flatten();
            }
            Some(Drag::Selection { grab, last, off }) => {
                if let Some(sel) = self.selection {
                    let next = match *grab {
                        SelGrab::Move => {
                            slide(sel, (p.0 - last.0, p.1 - last.1), canvas)
                        }
                        SelGrab::Handle(i) => resize_box(sel, i, (p.0 - off.0, p.1 - off.1)).intersect(&canvas).unwrap_or(sel),
                    };
                    self.selection = Some(next);
                    *last = p;
                }
            }
            None => {}
        }
        cx.notify();
    }

    fn snap_lines(&self, id: u64) -> Option<doc::SnapLines> {
        let o = self.object(id)?;
        o.snap_box()?;
        Some(doc::snap_lines(self.base(), &self.objects, o))
    }

    /// Pulls a dragged point onto nearby edges, recording the guides it lands on.
    fn snap_point(&mut self, id: u64, p: P, tol: f32) -> P {
        let Some(lines) = self.snap_lines(id) else {
            return p;
        };
        let x = doc::snap_axis(&[p.0], &lines.xs, tol);
        let y = doc::snap_axis(&[p.1], &lines.ys, tol);
        self.guides.extend(x.map(|(_, l)| (true, l)).into_iter().chain(y.map(|(_, l)| (false, l))));
        (x.map_or(p.0, |(_, l)| l), y.map_or(p.1, |(_, l)| l))
    }

    /// The extra shift that lands a moved object's edges on nearby edges.
    fn snap_move(&mut self, id: u64, d: P, tol: f32) -> P {
        let (Some(lines), Some(b)) = (self.snap_lines(id), self.object(id).and_then(Object::snap_box)) else {
            return (0., 0.);
        };
        let (l, t) = (b.x + d.0, b.y + d.1);
        let x = doc::snap_axis(&[l, l + b.w], &lines.xs, tol);
        let y = doc::snap_axis(&[t, t + b.h], &lines.ys, tol);
        self.guides.extend(x.map(|(_, l)| (true, l)).into_iter().chain(y.map(|(_, l)| (false, l))));
        (x.map_or(0., |(f, _)| f), y.map_or(0., |(f, _)| f))
    }

    fn mouse_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.guides.clear();
        match self.drag.take() {
            Some(Drag::Create { id, .. }) => {
                let trivial = self.object(id).is_some_and(|o| is_trivial(o, self.scale));
                if trivial {
                    Rc::make_mut(&mut self.objects).retain(|o| o.id != id);
                    self.selected = None;
                }
                self.commit();
            }
            Some(Drag::Move { .. } | Drag::Handle { .. }) => self.commit(),
            Some(Drag::Select { .. }) => {
                if let Some(r) = self.snap.take() {
                    self.selection = Some(r);
                }
                if self.selection.is_some_and(|s| s.w < 2. || s.h < 2.) {
                    self.selection = None;
                }
            }
            _ => {}
        }
        self.sync_slider(window, cx);
        cx.notify();
    }

    fn scroll(&mut self, e: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = e.delta.pixel_delta(px(20.));
        let wheel_zoom = matches!(e.delta, ScrollDelta::Lines(_)) && !e.modifiers.shift;
        if e.modifiers.platform || e.modifiers.control || wheel_zoom {
            let reverse = if settings::get(cx).reverse_wheel_zoom { -1. } else { 1. };
            let factor = (f32::from(delta.y) * 0.01 * reverse).exp();
            self.zoom_at(self.view().k * factor, e.position, cx);
        } else {
            if self.zoom.is_none() {
                self.zoom = Some(self.view().k);
            }
            self.pan = self.pan + delta;
            cx.notify();
        }
    }

    fn key_down(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Keys typed into the resize fields bubble up here too.
        if self.resize.0.focus_handle(cx).is_focused(window) || self.resize.1.focus_handle(cx).is_focused(window) {
            return;
        }
        if self.editing.is_some() {
            if self.text_key(e, cx) {
                cx.stop_propagation();
            }
            return;
        }
        let k = &e.keystroke;
        let m = k.modifiers;
        if m.platform || m.control {
            return;
        }
        match k.key.as_str() {
            "space" => self.space = true,
            "1" if !e.is_held => self.hold_ruler(Axis::Vertical),
            "2" if !e.is_held => self.hold_ruler(Axis::Horizontal),
            "left" | "right" | "up" | "down" => {
                let step = if m.shift { 10. } else { 1. } * self.scale;
                let d = match k.key.as_str() {
                    "left" => (-step, 0.),
                    "right" => (step, 0.),
                    "up" => (0., -step),
                    _ => (0., step),
                };
                if let Some(sel) = self.selection {
                    self.selection = Some(slide(sel, d, self.canvas()));
                } else if let Some(id) = self.selected {
                    self.edit_doc(|this| {
                        if let Some(o) = this.object_mut(id) {
                            o.shape.translate(d, Part::Body);
                        }
                    });
                } else if !e.is_held {
                    // With nothing to nudge, an arrow measures along its axis, as in the alternative.
                    self.hold_ruler(if d.1 == 0. { Axis::Horizontal } else { Axis::Vertical });
                } else {
                    return;
                }
            }
            _ => return,
        }
        let _ = window;
        cx.notify();
    }

    fn key_up(&mut self, e: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "space" => self.space = false,
            // An axis stays while any of its keys is still down: 1 or ↑↓, 2 or ←→.
            "1" | "up" | "down" if ![18, 125, 126].into_iter().any(mac::key_pressed) => {
                self.ruler.retain(|a| *a != Axis::Vertical)
            }
            "2" | "left" | "right" if ![19, 123, 124].into_iter().any(mac::key_pressed) => {
                self.ruler.retain(|a| *a != Axis::Horizontal)
            }
            _ => return,
        }
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            self.finish_text();
        } else if self.drag.is_some() {
            self.drag = None;
            if let Some(s) = self.pending.take() {
                self.restore(s);
            }
        } else if self.selection.is_some() {
            self.selection = None;
        } else if self.backdrop.as_ref().is_some_and(|b| b.panel_open) {
            if let Some(b) = &mut self.backdrop {
                b.panel_open = false;
            }
        } else {
            let s = settings::get(cx);
            let (copy, save) = (s.esc_copies, s.esc_saves);
            self.copy_and_save_close(copy, save, window, cx);
            return;
        }
        cx.notify();
    }

    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_some() {
            self.erase_selection(cx);
        } else if let Some(id) = self.selected.take() {
            self.edit_doc(|this| Rc::make_mut(&mut this.objects).retain(|o| o.id != id));
            self.set_tool(Tool::Select, window, cx);
            if doc::counters_have_gap(&self.objects) {
                self.show_toast("Counters have a gap", Some(ToastAction::Renumber), cx);
            }
        }
    }

    fn duplicate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(o) = self.selected.and_then(|id| self.object(id)).cloned() {
            self.insert_copy(o, window, cx);
        }
    }

    /// Adds `o` as a new object, nudged down and right so it doesn't hide the original.
    fn insert_copy(&mut self, mut o: Object, window: &mut Window, cx: &mut Context<Self>) -> Object {
        let d = 10. * self.scale;
        o.id = self.next_id;
        self.next_id += 1;
        o.shape.translate((d, d), Part::Body);
        let copy = o.clone();
        self.edit_doc(|this| Rc::make_mut(&mut this.objects).push(o));
        self.select(Some(copy.id), window, cx);
        copy
    }

    /// An object copied with ⌘C if the clipboard still holds its marker, else an image.
    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = cx.read_from_clipboard().and_then(|c| c.text());
        if let Some(c) = cx.try_global::<CopiedObject>()
            && text.as_ref() == Some(&c.marker)
        {
            let object = self.insert_copy(c.object.clone(), window, cx);
            // Repeated pastes cascade.
            cx.global_mut::<CopiedObject>().object = object;
            return;
        }
        self.paste_image(window, cx);
    }

    fn paste_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match mac::clipboard_image() {
            Some((w, h, bgra, ppp)) => self.insert_image(w, h, bgra, ppp, None, window, cx),
            None => self.show_toast("The clipboard has no image", None, cx),
        }
    }

    fn drop_files(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        let image = paths.paths().iter().find_map(|p| std::fs::read(p).ok().and_then(|b| crate::export::decode_rgba(&b)));
        let Some((w, h, mut bgra, ppp)) = image else {
            return self.show_toast("Couldn't open that image", None, cx);
        };
        for p in bgra.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        let at = self.view().to_image(window.mouse_position());
        self.insert_image(w, h, bgra, ppp, Some(at), window, cx);
    }

    /// Places an image at its real size, centered on `at` or else over the middle of the screenshot.
    fn insert_image(&mut self, w: u32, h: u32, bgra: Vec<u8>, ppp: f32, at: Option<P>, window: &mut Window, cx: &mut Context<Self>) {
        let k = self.scale / ppp;
        let (pw, ph) = (w as f32 * k, h as f32 * k);
        let (cx0, cy0) = at.unwrap_or_else(|| self.base().center());
        let a = (cx0 - pw / 2., cy0 - ph / 2.);
        let src = doc::Bitmap(Arc::new(doc::BitmapData { w, h, bgra }));
        let style = self.style(Tool::Image);
        self.edit_doc(|this| {
            this.selected = Some(this.add(Shape::Image { a, b: (a.0 + pw, a.1 + ph), src }, style));
        });
        self.sync_slider(window, cx);
        cx.notify();
    }

    fn rasterize(&mut self, cx: &mut Context<Self>) {
        if !self.objects.is_empty() {
            let (w, h) = self.canvas_px();
            self.bake(w, h, cx);
        }
    }

    /// Replaces the image with the canvas and its annotations, rendered at `w`×`h` pixels.
    fn bake(&mut self, w: u32, h: u32, cx: &mut Context<Self>) {
        let resized = (w, h) != self.canvas_px();
        let k = self.view().k;
        let canvas = self.canvas().round_out();
        let scene = self.render_scene(canvas, &self.ctx(), &self.objects);
        let mut bgra = match (scene.width(), scene.height()) {
            (sw, sh) if (sw, sh) == (w, h) => scene.take(),
            (sw, sh) => pixels::resample(scene.data(), sw, sh, w, h),
        };
        for p in bgra.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        self.edit_doc(|this| {
            this.image = TiledImage::from_bgra(w, h, &bgra, (w * 4) as usize);
            this.objects = Rc::default();
            this.crop = None;
        });
        self.selected = None;
        self.selection = None;
        self.patches.borrow_mut().clear();
        // Like the alternative, Resize keeps the zoom; it only recenters the smaller or larger image.
        if resized {
            self.zoom = Some(k);
            self.pan = Point::default();
        }
        cx.notify();
    }

    /// The canvas size in pixels: what Resize starts from.
    fn canvas_px(&self) -> (u32, u32) {
        let c = self.canvas().round_out();
        (c.w as u32, c.h as u32)
    }

    fn set_resize_fields(&mut self, k: f32, window: &mut Window, cx: &mut Context<Self>) {
        let (w, h) = self.canvas_px();
        let k = |v: u32| ((v as f32 * k).round() as u32).max(1).to_string();
        let (fw, fh) = self.resize.clone();
        fw.update(cx, |s, cx| s.set_value(k(w), window, cx));
        fh.update(cx, |s, cx| s.set_value(k(h), window, cx));
    }

    fn resize_fields(&self, cx: &App) -> Option<(u32, u32)> {
        let read = |f: &Entity<InputState>| f.read(cx).value().trim().parse::<u32>().ok().filter(|v| *v > 0);
        Some((read(&self.resize.0)?, read(&self.resize.1)?))
    }

    /// Keeps the other field at the canvas aspect ratio.
    fn link_resize_field(&mut self, from_w: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (w, h) = self.canvas_px();
        let (src, dst, ratio) = match from_w {
            true => (&self.resize.0, self.resize.1.clone(), h as f32 / w as f32),
            false => (&self.resize.1, self.resize.0.clone(), w as f32 / h as f32),
        };
        let Ok(v) = src.read(cx).value().trim().parse::<u32>() else { return };
        let other = ((v as f32 * ratio).round() as u32).max(1).to_string();
        dst.update(cx, |s, cx| s.set_value(other, window, cx));
    }

    fn resize_to_fields(&mut self, cx: &mut Context<Self>) {
        let Some((w, h)) = self.resize_fields(cx) else { return };
        // A runaway number would allocate gigabytes.
        if (w, h) == self.canvas_px() || w as u64 * h as u64 > 400_000_000 {
            return;
        }
        self.bake(w, h, cx);
    }

    fn toggle_unit(&mut self, cx: &mut Context<Self>) {
        self.unit_px = !self.unit_px;
        let v = self.unit_px;
        settings::update(cx, |s| s.unit_px = v);
        cx.notify();
    }

    // ---- color and ruler ------------------------------------------------

    fn pixel_at(&self, p: P) -> Option<u32> {
        if p.0 < 0. || p.1 < 0. {
            return None;
        }
        self.image
            .pixel(p.0 as u32, p.1 as u32)
            .map(|[b, g, r, _]| ((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }

    fn copy_color(&mut self, darkest: bool, cx: &mut Context<Self>) {
        let Some(p) = self.cursor else { return };
        let color = if darkest {
            let r = (20. * self.scale) as i32;
            let mut best: Option<(f32, u32)> = None;
            for dy in (-r..=r).step_by(2) {
                for dx in (-r..=r).step_by(2) {
                    if let Some(c) = self.pixel_at((p.0 + dx as f32, p.1 + dy as f32)) {
                        let l = luminance(c);
                        if best.is_none_or(|(bl, _)| l < bl) {
                            best = Some((l, c));
                        }
                    }
                }
            }
            best.map(|b| b.1)
        } else {
            self.pixel_at(p)
        };
        if let Some(c) = color {
            let text = settings::get(cx).color_format.format(c);
            self.copy_text(text.clone(), format!("Copied {text}"), cx);
        }
    }

    fn copy_average_color(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = self.selection_px().and_then(|r| pixels::average_color(&self.image, r)) {
            let text = settings::get(cx).color_format.format(c);
            self.copy_text(text.clone(), format!("Copied average {text}"), cx);
        }
    }

    /// Walks out from the cursor along `axis` until the color changes.
    fn hold_ruler(&mut self, axis: Axis) {
        if !self.ruler.contains(&axis) {
            self.ruler.push(axis);
        }
    }

    fn ruler_measures(&self) -> Vec<(P, P)> {
        self.ruler.iter().filter_map(|&axis| self.ruler_measure(axis)).collect()
    }

    fn ruler_measure(&self, axis: Axis) -> Option<(P, P)> {
        let p = self.cursor?;
        let (x, y) = (p.0.floor() as i64, p.1.floor() as i64);
        // Pixels cropped away are hidden, so the edge walk stops at the crop.
        let b = self.base().round_out();
        let (x0, y0, x1, y1) = (b.x as i64, b.y as i64, b.right() as i64, b.bottom() as i64);
        if x < x0 || y < y0 || x >= x1 || y >= y1 {
            return None;
        }
        let start = self.pixel_at((x as f32, y as f32))?;
        let step = |dx: i64, dy: i64| -> i64 {
            let mut n = 0;
            loop {
                let (nx, ny) = (x + dx * (n + 1), y + dy * (n + 1));
                if nx < x0 || ny < y0 || nx >= x1 || ny >= y1 {
                    return n;
                }
                let Some(c) = self.pixel_at((nx as f32, ny as f32)) else {
                    return n;
                };
                // Tight enough to stop at a #F7 divider on white or at the first anti-aliased glyph pixel.
                if color_diff(c, start) > 12 {
                    return n;
                }
                n += 1;
            }
        };
        Some(match axis {
            Axis::Horizontal => {
                let (l, r) = (step(-1, 0), step(1, 0));
                ((x as f32 - l as f32, y as f32 + 0.5), (x as f32 + r as f32 + 1., y as f32 + 0.5))
            }
            Axis::Vertical => {
                let (t, b) = (step(0, -1), step(0, 1));
                ((x as f32 + 0.5, y as f32 - t as f32), (x as f32 + 0.5, y as f32 + b as f32 + 1.))
            }
        })
    }

    // ---- patches --------------------------------------------------------

    /// Text-only redaction needs line boxes; OCR runs once, on first use.
    fn find_text_if_needed(&mut self, cx: &mut Context<Self>) {
        let text_mode = |tool: Tool, variant: u8| tool == Tool::Redact && matches!(variant, 1 | 3);
        if self.finding_text
            || !(text_mode(self.tool, self.style(self.tool).variant)
                || self.objects.iter().any(|o| o.shape.tool() == Some(Tool::Redact) && matches!(o.variant, 1 | 3)))
        {
            return;
        }
        self.finding_text = true;
        let full = self.image.full();
        let bgra = self.image.read(full);
        cx.spawn(async move |this, cx| {
            let boxes = cx
                .background_executor()
                .spawn(async move { crate::ocr::text_boxes(&bgra, full.w, full.h) })
                .await
                .unwrap_or_default();
            let _ = this.update(cx, |this, cx| {
                this.text_lines = Some(boxes.into_iter().map(|(x, y, w, h)| R::new(x, y, w, h)).collect());
                cx.notify();
            });
        })
        .detach();
    }

    /// Brings cached pixel patches up to date with the objects that need them.
    fn refresh_patches(&self) {
        let mut cache = self.patches.borrow_mut();
        let mut stale = self.stale.borrow_mut();
        let ctx = self.ctx();
        let mut alive = Vec::new();
        for o in self.objects.iter() {
            for prim in o.prims(&ctx) {
                if let Prim::Image { key, src, alpha, .. } = &prim {
                    // Textures depend only on the pixels and opacity; GPUI scales them.
                    let fresh = Source::Image(Arc::as_ptr(&src.0), alpha.to_bits());
                    alive.push(*key);
                    if !cache.get(key).is_some_and(|c| c.key == fresh) {
                        let image = Arc::new(crate::tiled::frame_image(src.0.w, src.0.h, straight(&src.0.bgra, *alpha)));
                        let at = R::default();
                        if let Some(old) = cache.insert(*key, CachedPatch { key: fresh, at, image }) {
                            stale.push(old.image);
                        }
                    }
                    continue;
                }
                if let Prim::Highlight { key, path, width, color, rect } = &prim {
                    alive.push(*key);
                    let mut h = std::hash::DefaultHasher::new();
                    for seg in path {
                        let (a, b, c) = match *seg {
                            doc::Seg::M(p) | doc::Seg::L(p) => (p, p, p),
                            doc::Seg::Q(a, p) => (a, p, p),
                            doc::Seg::C(a, b, p) => (a, b, p),
                            doc::Seg::Z => Default::default(),
                        };
                        h.write(&[a.0, a.1, b.0, b.1, c.0, c.1].map(f32::to_bits).map(u32::to_le_bytes).concat());
                    }
                    h.write(&[width.to_bits(), *color, self.fill.unwrap_or(u32::MAX)].map(u32::to_le_bytes).concat());
                    let fresh = Source::Highlight(h.finish());
                    if !cache.get(key).is_some_and(|c| c.key == fresh)
                        && let Some(patch) = export::highlight_patch(&self.image, self.fill, path, *width, *color, *rect)
                    {
                        let image = Arc::new(crate::tiled::frame_image(patch.w, patch.h, patch.bgra));
                        if let Some(old) = cache.insert(*key, CachedPatch { key: fresh, at: patch.at, image }) {
                            stale.push(old.image);
                        }
                    }
                    continue;
                }
                let Prim::Pixels { key, op, rect } = prim else {
                    continue;
                };
                alive.push(key);
                let r = rect.round_out();
                let src = match op {
                    doc::PixelOp::Lens { src, .. } => src.round_out(),
                    _ => R::default(),
                };
                let geometry = [r.x as i32 ^ ((src.x as i32) << 16), r.y as i32 ^ ((src.y as i32) << 16), r.w as i32 ^ ((src.w as i32) << 16), r.h as i32];
                let fresh = Source::Pixels(op, geometry);
                if cache.get(&key).is_some_and(|c| c.key == fresh) {
                    continue;
                }
                if let Some(patch) = pixels::patch(&self.image, op, rect) {
                    let image = Arc::new(crate::tiled::frame_image(patch.w, patch.h, patch.bgra));
                    if let Some(old) = cache.insert(
                        key,
                        CachedPatch {
                            key: fresh,
                            at: patch.at,
                            image,
                        },
                    ) {
                        stale.push(old.image);
                    }
                }
            }
        }
        let dead: Vec<u64> = cache.keys().filter(|k| !alive.contains(k)).copied().collect();
        for k in dead {
            if let Some(old) = cache.remove(&k) {
                stale.push(old.image);
            }
        }
    }
}

/// New counters point down and left, as in the alternative.
const COUNTER_TAIL: f32 = 3. * std::f32::consts::FRAC_PI_4;

struct CopiedObject {
    object: Object,
    marker: String,
}

impl Global for CopiedObject {}

/// GPUI blends textures with straight alpha.
fn straight(premultiplied: &[u8], alpha: f32) -> Vec<u8> {
    premultiplied
        .chunks_exact(4)
        .flat_map(|p| {
            let a = p[3] as f32;
            let k = if a > 0. { 255. / a } else { 0. };
            [(p[0] as f32 * k).min(255.) as u8, (p[1] as f32 * k).min(255.) as u8, (p[2] as f32 * k).min(255.) as u8, (a * alpha).round() as u8]
        })
        .collect()
}

/// Encodes off the main thread; Auto JPEG may change the extension.
/// Encodes run in parallel, so a save that finishes after a newer one to the same file skips its write.
fn write_later(pixmap: Arc<(tiny_skia::Pixmap, f32)>, path: PathBuf, cx: &App) -> Task<Result<PathBuf, String>> {
    static LATEST: std::sync::Mutex<Vec<(PathBuf, u64)>> = std::sync::Mutex::new(Vec::new());
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let auto = settings::get(cx).save_auto_jpeg;
    let generation = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    {
        let mut latest = LATEST.lock().unwrap_or_else(|e| e.into_inner());
        latest.retain(|(p, _)| *p != path);
        latest.push((path.clone(), generation));
    }
    cx.background_spawn(async move {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
        let (bytes, ext) = export::for_save(&pixmap.0, pixmap.1, auto);
        let mut latest = LATEST.lock().unwrap_or_else(|e| e.into_inner());
        let stale = latest.iter().any(|(p, g)| *p == path && *g != generation);
        latest.retain(|(p, g)| !(*p == path && *g == generation));
        let path = path.with_extension(ext);
        if stale {
            return Ok(path);
        }
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        mac::mark_screenshot(&path);
        Ok(path)
    })
}

/// Puts a PNG on the clipboard once `encode` finishes in the background, so the window
/// that asked can close at once.
pub fn copy_later(encode: impl FnOnce() -> Vec<u8> + Send + 'static, cx: &App) {
    let name = settings::get(cx).file_name.clone();
    let png = cx.background_spawn(async move {
        let png = encode();
        let file = write_temp(&png, &name);
        (png, file)
    });
    cx.spawn(async move |_| {
        let (png, file) = png.await;
        mac::copy_png(&png, file.as_deref());
    })
    .detach();
}

pub fn write_temp(png: &[u8], name: &str) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("Kiri");
    std::fs::create_dir_all(&dir).ok()?;
    let path = crate::settings::timestamp_path(&dir, name);
    std::fs::write(&path, png).ok()?;
    Some(path)
}

pub trait DragFileOut: StatefulInteractiveElement + Sized {
    /// Offers the file `path` writes to other apps once a drag of `T` leaves the window.
    fn drag_file_out<T: 'static>(self, path: impl Fn(&T, &App) -> Option<PathBuf> + 'static) -> Self {
        // GPUI hands a drag to the platform only on macOS.
        #[cfg(windows)]
        return self.on_drag_move(move |e: &DragMoveEvent<T>, window, cx| {
            if Bounds::new(Point::default(), window.viewport_size()).contains(&e.event.position) {
                return;
            }
            let file = path(e.drag(cx), cx);
            cx.stop_active_drag(window);
            if let Some(file) = file {
                mac::drag_file_out(file, cx);
            }
        });
        #[cfg(not(windows))]
        self.external_drag_payload(move |drag: &T, _, cx| Some(ExternalDragPayload::Files(FileDragPaths::new([(path(drag, cx)?, false)]))))
    }
}

impl<E: StatefulInteractiveElement> DragFileOut for E {}

fn luminance(c: u32) -> f32 {
    let [r, g, b] = [(c >> 16) & 255, (c >> 8) & 255, c & 255].map(|v| v as f32);
    0.299 * r + 0.587 * g + 0.114 * b
}

fn color_diff(a: u32, b: u32) -> i32 {
    (0..3)
        .map(|i| (((a >> (i * 8)) & 255) as i32 - ((b >> (i * 8)) & 255) as i32).abs())
        .sum()
}

/// `cmd` points a new arrow back at where the drag started, as the alternative does.
fn shape_drag(shape: &mut Shape, start: P, p: P, shift: bool, alt: bool, cmd: bool, scale: f32) {
    let boxed = |p: P| {
        let b = if shift { doc::square(start, p) } else { p };
        let a = if alt { (2. * start.0 - b.0, 2. * start.1 - b.1) } else { start };
        (a, b)
    };
    match shape {
        Shape::Arrow { from, to, .. } => {
            let end = if shift { doc::snap45(start, p) } else { p };
            let begin = if alt { (2. * start.0 - end.0, 2. * start.1 - end.1) } else { start };
            (*from, *to) = if cmd { (end, begin) } else { (begin, end) };
        }
        Shape::Line { from, to, .. } | Shape::Measure { from, to } => {
            *to = if shift { doc::snap45(start, p) } else { p };
            *from = if alt { (2. * start.0 - to.0, 2. * start.1 - to.1) } else { start };
        }
        Shape::Rect { a, b } | Shape::Oval { a, b } | Shape::Redact { a, b } | Shape::Spotlight { a, b } => {
            (*a, *b) = boxed(p);
        }
        Shape::Pen { points } | Shape::Highlighter { points } => {
            if shift {
                points.truncate(1);
                points.push(doc::snap45(start, p));
            } else if points.last().is_none_or(|q| (q.0 - p.0).hypot(q.1 - p.1) >= 0.75 * scale) {
                points.push(p);
            }
        }
        // As in the alternative: the drag boxes the source, and a 3× lens hangs below-right of it.
        Shape::Magnifier { src, radius, lens, lens_radius } => {
            *src = ((start.0 + p.0) / 2., (start.1 + p.1) / 2.);
            *radius = ((p.0 - start.0).abs().max((p.1 - start.1).abs()) / 2.).max(1.);
            *lens_radius = *radius * 3.;
            let d = *radius * 5.;
            *lens = (src.0 + d * 0.84, src.1 + d * 0.54);
        }
        Shape::Text { .. } | Shape::Counter { .. } | Shape::Image { .. } => {}
    }
}

fn is_trivial(o: &Object, scale: f32) -> bool {
    let min = 3. * scale;
    match &o.shape {
        Shape::Rect { a, b } | Shape::Oval { a, b } | Shape::Redact { a, b } | Shape::Spotlight { a, b } => {
            (a.0 - b.0).abs() < min || (a.1 - b.1).abs() < min
        }
        Shape::Arrow { from, to, .. } | Shape::Line { from, to, .. } | Shape::Measure { from, to } => {
            (from.0 - to.0).hypot(from.1 - to.1) < min * 2.
        }
        Shape::Pen { points } => points.len() < 2,
        Shape::Highlighter { points } => {
            let (a, b) = (points[0], *points.last().unwrap());
            points.len() <= 2 && (a.0 - b.0).hypot(a.1 - b.1) < min
        }
        Shape::Magnifier { radius, .. } => *radius < min * 2.,
        Shape::Text { .. } | Shape::Counter { .. } | Shape::Image { .. } => false,
    }
}

fn resize_box(r: R, i: u8, p: P) -> R {
    let (mut x0, mut y0, mut x1, mut y1) = (r.x, r.y, r.right(), r.bottom());
    match i {
        0 => (x0, y0) = p,
        1 => y0 = p.1,
        2 => (x1, y0) = p,
        3 => x1 = p.0,
        4 => (x1, y1) = p,
        5 => y1 = p.1,
        6 => (x0, y1) = p,
        _ => x0 = p.0,
    }
    R::from_points((x0, y0), (x1, y1))
}

fn fit_k(bounds: Bounds<Pixels>, area: R, scale: f32) -> f32 {
    let bw = f32::from(bounds.size.width) - FIT_MARGIN * 2.;
    let bh = f32::from(bounds.size.height) - FIT_MARGIN * 2.;
    (bw / area.w).min(bh / area.h).min(1. / scale).max(0.01)
}

/// As in the alternative, shapes get icons; modes an icon can't tell apart keep their names.
fn variant_icon(tool: Tool, i: usize) -> Option<&'static str> {
    let icons: &[&str] = match tool {
        Tool::Arrow => &["arrow-block", "arrow-bold", "arrow-thin", "arrow-double"],
        Tool::Text => &["text-label", "text-plain"],
        Tool::Rect => &["rect-sketch", "rect-outline", "rect-tinted", "rect-solid"],
        Tool::Oval => &["oval-sketch", "oval-outline", "oval-tinted", "oval-solid"],
        Tool::Line => &["line-solid", "line-dashed"],
        Tool::Pen => &["pen-free", "pen-rect", "pen-oval"],
        Tool::Spotlight => &["rect-outline", "oval-outline"],
        Tool::Magnifier => &["round", "square"],
        Tool::Counter => &["counter-bubble", "counter-circle"],
        _ => &[],
    };
    icons.get(i).copied()
}

fn centered_origin(bounds: Bounds<Pixels>, area: R, k: f32) -> Point<Pixels> {
    let c = bounds.center();
    let (ax, ay) = area.center();
    point(c.x - px(ax * k), c.y - px(ay * k))
}

fn compute_view(bounds: Bounds<Pixels>, area: R, scale: f32, zoom: Option<f32>, pan: Point<Pixels>) -> View {
    let k = zoom.unwrap_or_else(|| fit_k(bounds, area, scale));
    let origin = centered_origin(bounds, area, k);
    let origin = if zoom.is_some() { origin + pan } else { origin };
    View { origin, k }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.find_text_if_needed(cx);
        let theme = cx.theme();
        let mut context = KeyContext::default();
        context.add("Editor");
        if self.editing.is_some() {
            context.add("text");
        }
        if self.selection.is_some() {
            context.add("selection");
        }
        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .key_context(context)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_key_up(cx.listener(Self::key_up))
            .on_modifiers_changed(cx.listener(Self::modifiers_changed))
            // Menu items with bare-letter keys fire whenever gpui passes on a key, so their
            // actions exist only while those keys aren't text: gpui then disables the items.
            .when(self.editing.is_none() && self.focus.is_focused(window), |d| {
                d
                    .on_action(cx.listener(|this, _: &ToolSelect, w, cx| this.set_tool(Tool::Select, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolArrow, w, cx| this.set_tool(Tool::Arrow, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolText, w, cx| this.set_tool(Tool::Text, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolRect, w, cx| this.set_tool(Tool::Rect, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolOval, w, cx| this.set_tool(Tool::Oval, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolLine, w, cx| this.set_tool(Tool::Line, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolPen, w, cx| this.set_tool(Tool::Pen, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolHighlighter, w, cx| this.set_tool(Tool::Highlighter, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolCounter, w, cx| this.set_tool(Tool::Counter, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolRedact, w, cx| this.set_tool(Tool::Redact, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolSpotlight, w, cx| this.set_tool(Tool::Spotlight, w, cx)))
                    .on_action(cx.listener(|this, _: &ToolMagnifier, w, cx| this.set_tool(Tool::Magnifier, w, cx)))
                    .when_some(self.selection, |d, sel| {
                        d.on_action(cx.listener(move |this, _: &SelectionTopLeft, _, cx| this.inspect((sel.x, sel.y), cx)))
                            .on_action(cx.listener(move |this, _: &SelectionBottomRight, _, cx| {
                                this.inspect((sel.right(), sel.bottom()), cx)
                            }))
                    })
            })
            .on_action(cx.listener(|this, _: &CopyImage, _, cx| this.copy(cx)))
            .on_action(cx.listener(|this, _: &CopyAndClose, w, cx| this.copy_and_close(w, cx)))
            .on_action(cx.listener(|this, _: &CopyObjectOrClose, w, cx| this.copy_object_or_close(w, cx)))
            .on_action(cx.listener(|this, _: &Save, _, cx| this.save(cx)))
            .on_action(cx.listener(|this, _: &SaveAs, _, cx| this.save_as(cx)))
            .on_action(cx.listener(|this, _: &Pin, w, cx| this.pin(w, cx)))
            .on_action(cx.listener(|this, _: &AddCapture, w, cx| {
                this.finish_text();
                crate::add_capture(w.window_handle(), cx);
            }))
            .on_action(cx.listener(|this, _: &Print, w, cx| {
                this.finish_text();
                mac::print_png(&this.png(cx), w);
            }))
            .on_action(cx.listener(|_, _: &Close, w, _| w.remove_window()))
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.undo(cx)))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.redo(cx)))
            .on_action(cx.listener(|this, _: &Delete, window, cx| this.delete(window, cx)))
            .on_action(cx.listener(|this, _: &Duplicate, w, cx| this.duplicate(w, cx)))
            .on_action(cx.listener(|this, _: &Paste, w, cx| this.paste(w, cx)))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, w, cx| this.drop_files(paths, w, cx)))
            .on_action(cx.listener(|this, _: &Confirm, _, cx| this.crop_to_selection(cx)))
            .on_action(cx.listener(|this, _: &Cancel, w, cx| this.cancel(w, cx)))
            .on_action(cx.listener(|this, _: &SelectAll, w, cx| {
                this.set_tool(Tool::Select, w, cx);
                this.selection = Some(this.canvas());
            }))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.step_zoom(1.25, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.step_zoom(0.8, cx)))
            .on_action(cx.listener(|this, _: &ZoomFit, _, cx| this.fit_all(cx)))
            .on_action(cx.listener(|this, _: &ZoomActual, _, cx| {
                let center = this.canvas_bounds.get().center();
                this.zoom_at(1. / this.scale, center, cx);
            }))
            .on_action(cx.listener(|this, _: &ZoomSelection, _, cx| {
                if let Some(sel) = this.selection {
                    let b = this.canvas_bounds.get();
                    let k = fit_k(b, sel, 1. / 32.);
                    let centered = centered_origin(b, this.view_area(), k);
                    let target = centered_origin(b, sel, k);
                    this.zoom = Some(k);
                    this.pan = point(target.x - centered.x, target.y - centered.y);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &Autoadjust, _, cx| this.autoadjust(cx)))
            .on_action(cx.listener(|this, _: &GrowSelection, _, cx| this.resize_selection(10., cx)))
            .on_action(cx.listener(|this, _: &ShrinkSelection, _, cx| this.resize_selection(-10., cx)))
            .on_action(cx.listener(|this, _: &RecognizeText, _, cx| this.recognize(cx)))
            .on_action(cx.listener(|this, _: &ImprintSize, _, cx| this.imprint_size(cx)))
            .on_action(cx.listener(|this, _: &ResetCrop, _, cx| this.reset_crop(cx)))
            // AppKit runs the menu item on macOS.
            .when(cfg!(windows), |d| d.on_action(|_: &EnterFullScreen, window, _| window.toggle_fullscreen()))
            .on_action(cx.listener(|this, _: &Rasterize, _, cx| this.rasterize(cx)))
            .on_action(cx.listener(|this, _: &CopyColor, _, cx| this.copy_color(false, cx)))
            .on_action(cx.listener(|this, _: &CopyTextColor, _, cx| this.copy_color(true, cx)))
            .on_action(cx.listener(|this, _: &CopyAverageColor, _, cx| this.copy_average_color(cx)))
            .on_action(cx.listener(|this, _: &ToggleUnit, _, cx| this.toggle_unit(cx)))
            .on_action(cx.listener(|this, _: &ToggleBackdrop, w, cx| {
                crate::backdrop::toggle(this, w, cx);
            }))
            .child(div().flex_none().child(self.render_toolbar(window, cx)))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_canvas(cx))
                    .when_some(self.backdrop_panel.clone(), |row, panel| row.child(panel)),
            )
    }
}

impl Editor {
    pub fn set_backdrop(&mut self, b: Option<crate::backdrop::Backdrop>, window: &mut Window, cx: &mut Context<Self>) {
        let open = b.as_ref().is_some_and(|b| b.panel_open);
        if !open {
            self.backdrop_panel = None;
        } else if self.backdrop_panel.is_none() {
            let editor = cx.entity().downgrade();
            let b = b.clone().unwrap_or_default();
            let suggested = crate::backdrop::suggest(&self.image);
            self.backdrop_panel = Some(cx.new(|cx| crate::backdrop::Panel::new(editor, &b, suggested, window, cx)));
        }
        if self.backdrop.is_some() != b.is_some() {
            self.fit(cx);
        }
        self.backdrop = b;
        cx.notify();
    }

    fn render_toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let divider = || div().w(px(1.)).h(px(20.)).mx_1().bg(cx.theme().border);
        let icon_button = |id: &'static str, icon: IconName, tip: &'static str, action: Box<dyn Action>| {
            let a = action.boxed_clone();
            Button::new(id)
                .icon(Icon::new(icon))
                .ghost()
                .small()
                .tooltip_with_action(tip, action.as_ref(), Some("Editor"))
                .on_click(move |_, window, cx| window.dispatch_action(a.boxed_clone(), cx))
        };

        let tools = if self.selection.is_some() {
            h_flex()
                .p(px(3.))
                .gap(px(2.))
                .rounded(theme.radius_lg)
                .bg(theme.muted)
                .child(icon_button("s-crop", IconName::Crop, "Crop to selection", Box::new(Confirm)))
                .child(icon_button("s-auto", IconName::ScanLine, "Autoadjust to content", Box::new(Autoadjust)))
                .child(icon_button("s-grow", IconName::Maximize2, "Grow selection", Box::new(GrowSelection)))
                .child(icon_button("s-shrink", IconName::Minimize2, "Shrink selection", Box::new(ShrinkSelection)))
                .child(icon_button("s-ocr", IconName::ScanText, "Recognize text or QR", Box::new(RecognizeText)))
                .child(icon_button("s-imprint", IconName::Ruler, "Imprint selection size", Box::new(ImprintSize)))
                .child(icon_button("s-erase", IconName::Eraser, "Erase", Box::new(Delete)))
                .child(icon_button("s-cancel", IconName::X, "Cancel selection", Box::new(Cancel)))
        } else {
            h_flex()
                .p(px(3.))
                .gap(px(2.))
                .rounded(theme.radius_lg)
                .bg(theme.muted)
                .children(TOOL_ACTIONS.iter().flat_map(|(tool, action)| {
                    let tool = *tool;
                    let button = Button::new(("tool", tool as usize))
                        .icon(Icon::new(tool_icon(tool)))
                        .ghost()
                        .small()
                        .selected(self.tool == tool)
                        .tooltip_with_action(tool.name(), action().as_ref(), Some("Editor"))
                        .on_click(cx.listener(move |this, _, window, cx| this.set_tool(tool, window, cx)));
                    // The ruler is held, not picked, so its button only explains the keys.
                    std::iter::once(button.into_any_element()).chain((tool == Tool::Text).then(|| ruler_hint().into_any_element()))
                }))
                .child({
                    let focus = self.focus.clone();
                    Button::new("more").icon(Icon::new(IconName::Ellipsis)).ghost().small().tooltip("More").dropdown_menu(
                        move |menu, _, _| {
                            menu.action_context(focus.clone())
                                .menu("Add Capture", Box::new(AddCapture))
                                .menu("Paste Image", Box::new(Paste))
                        },
                    )
                })
        };

        let zoom_pct = (self.view().k * self.scale * 100.).round();
        let area = self.selection.unwrap_or_else(|| self.outer());
        let unit = if self.unit_px { 1. } else { self.scale };
        let size_caption = if self.selection.is_some() { "Selection size" } else { "Image size" };
        let size_text = format!(
            "{}×{} {}",
            (area.w / unit).round(),
            (area.h / unit).round(),
            if self.unit_px { "px" } else { "pt" }
        );
        let hover = self.cursor.and_then(|p| self.pixel_at(p));
        let contrast = self.selection_px().and_then(|r| match self.contrast.get() {
            Some((at, pair)) if at == r => pair,
            _ => {
                let pair = pixels::dominant_pair(&self.image, r);
                self.contrast.set(Some((r, pair)));
                pair
            }
        });
        let swatch = |c: u32| {
            div()
                .size(px(14.))
                .rounded_full()
                .bg(rgb(c))
                .border_1()
                .border_color(cx.theme().border)
        };
        let stat = |value: String, caption: &'static str| {
            v_flex()
                .items_end()
                .gap(px(1.))
                .child(div().text_sm().font_medium().child(value))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(caption))
        };

        let bar = h_flex()
            .h(px(TOOLBAR_H))
            .flex_none()
            .pl(px(86.))
            .pr_4()
            .gap_1()
            .items_center()
            .bg(theme.title_bar)
            .border_b_1()
            .border_color(theme.title_bar_border)
            .when(cfg!(windows), |bar| bar.child(self.menu_button()))
            .child(icon_button("copy", IconName::Copy, "Copy and close", Box::new(CopyAndClose)))
            .child(icon_button("save", IconName::Download, "Save", Box::new(Save)))
            .child(icon_button("pin", IconName::Pin, "Pin to screen", Box::new(Pin)))
            .child(
                div()
                    .id("drag-out")
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(theme.radius)
                    .cursor_grab()
                    .hover(|s| s.bg(cx.theme().secondary_hover))
                    .child(Icon::new(IconName::GripVertical).small().text_color(theme.muted_foreground))
                    .tooltip(|window, cx| Tooltip::new("Drag the image into another app").build(window, cx))
                    .on_drag(DragOut(cx.weak_entity()), |_, _, _, cx| cx.new(|_| DragGhost))
                    .drag_file_out(|drag: &DragOut, cx| drag.0.upgrade()?.read(cx).export_temp(cx)),
            )
            .child(divider())
            .child(tools)
            .child(divider())
            .child(
                Button::new("backdrop")
                    .icon(Icon::new(IconName::Image))
                    .ghost()
                    .small()
                    .selected(self.backdrop.is_some())
                    .tooltip_with_action("Backdrop", &ToggleBackdrop, Some("Editor"))
                    .on_click(cx.listener(|this, _, w, cx| crate::backdrop::toggle(this, w, cx))),
            )
            .child(
                Button::new("undo")
                    .icon(Icon::new(IconName::Undo2))
                    .ghost()
                    .small()
                    .disabled(self.undo.is_empty() && self.pending.is_none())
                    .tooltip_with_action("Undo", &Undo, Some("Editor"))
                    .on_click(cx.listener(|this, _, _, cx| this.undo(cx))),
            )
            .child(
                Button::new("redo")
                    .icon(Icon::new(IconName::Redo2))
                    .ghost()
                    .small()
                    .disabled(self.redo.is_empty())
                    .tooltip_with_action("Redo", &Redo, Some("Editor"))
                    .on_click(cx.listener(|this, _, _, cx| this.redo(cx))),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .when(cfg!(windows), |d| d.window_control_area(WindowControlArea::Drag)),
            )
            .child(
                h_flex()
                    .gap_4()
                    .when_some(contrast, |d, (a, b)| {
                        let ratio = pixels::contrast_ratio(a, b);
                        let grade = match ratio {
                            r if r >= 7. => "AAA",
                            r if r >= 4.5 => "AA",
                            r if r >= 3. => "AA Large",
                            _ => "Fail",
                        };
                        d.child(
                            h_flex()
                                .id("contrast")
                                .gap_2()
                                .tooltip(|window, cx| {
                                    Tooltip::new("WCAG 2 contrast of the two main colors in the selection. 4.5 is enough for text, 3 for large text and UI.")
                                        .build(window, cx)
                                })
                                .child(h_flex().child(swatch(a)).child(swatch(b).ml(px(-5.))))
                                .child(stat(format!("{ratio:.2} {grade}"), "Contrast")),
                        )
                    })
                    .child(
                        Popover::new("color-format")
                            .anchor(Anchor::TopRight)
                            .trigger(
                                Button::new("color")
                                    .ghost()
                                    .child(h_flex().gap_2().child(swatch(hover.unwrap_or(0x808080))).child(stat(
                                        hover.map_or("—".to_string(), |c| format!("#{c:06X}")),
                                        "Tab to copy",
                                    ))),
                            )
                            .content(|_, _, cx| {
                                let current = settings::get(cx).color_format;
                                let popover = cx.entity();
                                let muted = cx.theme().muted_foreground;
                                v_flex()
                                    .w(px(240.))
                                    .gap_1()
                                    .children(ColorFormat::OPTIONS.iter().map(|&(format, label)| {
                                        let popover = popover.clone();
                                        Button::new(("format", format as usize))
                                            .ghost()
                                            .small()
                                            .w_full()
                                            .justify_start()
                                            .selected(format == current)
                                            .label(label)
                                            .on_click(move |_, window, cx| {
                                                settings::update(cx, |s| s.color_format = format);
                                                popover.update(cx, |p, cx| p.dismiss(window, cx));
                                            })
                                    }))
                                    .child(
                                        div()
                                            .pt_1()
                                            .text_xs()
                                            .text_color(muted)
                                            .child(crate::platform_text(
                                                "Tab copies the pixel color, ⇧Tab the text color nearby, C the average of a selection.",
                                                "Tab copies the pixel color, Shift+Tab the text color nearby, C the average of a selection.",
                                            )),
                                    )
                            }),
                    )
                    .child({
                        let editor = cx.entity();
                        Popover::new("size")
                            .anchor(Anchor::TopRight)
                            .on_open_change({
                                let editor = editor.clone();
                                move |open, window, cx| {
                                    if *open {
                                        editor.update(cx, |this, cx| this.set_resize_fields(1., window, cx));
                                    }
                                }
                            })
                            .trigger(Button::new("size").ghost().child(stat(size_text, size_caption)))
                            .content(move |_, _, cx| size_popover(&editor, cx))
                    })
                    .child(
                        div()
                            .id("zoom")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.fit_all(cx)))
                            .child(stat(format!("{zoom_pct}%"), "Zoom")),
                    ),
            );
        #[cfg(windows)]
        let bar = bar.pl_3().pr_0().child(div().w_4()).child(mac::caption_buttons(window, cx));
        #[cfg(not(windows))]
        let _ = window;
        bar
    }

    /// Windows has no menu bar, so the macOS menus open from a button.
    fn menu_button(&self) -> impl IntoElement + use<> {
        let focus = self.focus.clone();
        Button::new("menu").icon(Icon::new(IconName::Menu)).ghost().small().tooltip("Menu").dropdown_menu(move |menu, window, cx| {
            let names: Vec<_> = app_menus().into_iter().map(|m| m.name).collect();
            names.into_iter().enumerate().fold(menu.action_context(focus.clone()), |menu, (i, name)| {
                let focus = focus.clone();
                menu.submenu(name, window, cx, move |menu, _, _| {
                    app_menus().swap_remove(i).items.into_iter().fold(menu.action_context(focus.clone()), |menu, item| match item {
                        MenuItem::Separator => menu.separator(),
                        MenuItem::Action { name, action, .. } => menu.menu(name, action),
                        _ => menu,
                    })
                })
            })
        })
    }

    fn render_props(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if self.selection.is_some() || !self.ruler.is_empty() {
            return None;
        }
        let tool = self.styled_tool()?;
        let style = self.current_style()?;
        if tool == Tool::Select {
            return None;
        }
        let theme = cx.theme();
        let palette: &'static [u32] = if tool == Tool::Highlighter { &HIGHLIGHTS } else { &PALETTE };
        let colors = ColorPicker::new(&self.color)
            .small()
            .anchor(Anchor::TopLeft)
            .featured_colors(palette.iter().map(|&c| rgb(c).into()).collect());
        let variants = tool.variants().iter().enumerate().map(|(i, name)| {
            let button = Button::new(("variant", i)).ghost();
            match variant_icon(tool, i) {
                Some(icon) => button.icon(Icon::empty().path(format!("variants/{icon}.svg"))).small().tooltip(*name),
                None => button.label(*name).xsmall(),
            }
                .selected(style.variant as usize == i)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.apply_style(|s| s.variant = i as u8, cx);
                    this.commit();
                }))
        });
        let size_label = format!("{} pt", style.size);
        let border = theme.border;
        let divider = || div().w(px(1.)).h(px(18.)).bg(border);
        let collapsed = self.props_collapsed;
        let toggle = Button::new("collapse-props")
            .icon(if collapsed { IconName::ChevronLeft } else { IconName::ChevronRight })
            .ghost()
            .xsmall()
            .tooltip(if collapsed { "Show style" } else { "Hide style" })
            .on_click(cx.listener(|this, _, _, cx| {
                this.props_collapsed = !this.props_collapsed;
                cx.notify();
            }));
        if collapsed {
            return Some(
                h_flex()
                    .absolute()
                    .top_3()
                    .right_3()
                    .h(px(40.))
                    .px_1()
                    .rounded(theme.radius_lg)
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(toggle),
            );
        }
        Some(
            h_flex()
                .absolute()
                .top_3()
                .right_3()
                .h(px(40.))
                .px_2()
                .gap_2()
                .rounded(theme.radius_lg)
                .bg(theme.popover)
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(toggle)
                .when(tool.uses_color(), |d| d.child(colors))
                .when(tool.size_range().is_some(), |d| {
                    d.child(div().w(px(110.)).child(Slider::new(&self.slider))).child(
                        div()
                            .w(px(38.))
                            .text_xs()
                            .text_right()
                            .text_color(theme.muted_foreground)
                            .child(size_label),
                    )
                })
                .when(!tool.variants().is_empty(), |d| {
                    d.child(divider()).child(h_flex().gap(px(2.)).children(variants))
                })
                .when_some(self.selected.filter(|_| tool == Tool::Image), |d, id| {
                    d.child(divider()).child(
                        Button::new("gif")
                            .label("GIF")
                            .ghost()
                            .xsmall()
                            .tooltip("Save a GIF that flips between this image and what's under it")
                            .on_click(cx.listener(move |this, _, _, cx| this.save_gif(id, cx))),
                    )
                }),
        )
    }

    fn render_canvas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_patches();
        let tiles: Vec<(PxRect, Arc<RenderImage>)> = self.image.tiles().map(|(r, t)| (r, t.clone())).collect();
        let objects = self.objects.clone();
        let ctx_scale = self.scale;
        let unit_px = self.unit_px;
        let text_lines = self.text_lines.clone();
        let canvas_rect = self.canvas();
        let view_area = self.view_area();
        // Pixels cropped away stay hidden even where the canvas grows over them.
        let image_rect = self.base();
        let (scale, zoom, pan) = (self.scale, self.zoom, self.pan);
        let cell = self.canvas_bounds.clone();
        let patches = self.patches.clone();
        let stale = self.stale.clone();
        let crisp = self.crisp.clone();
        let full_image = self.image.clone();
        let bg_fill = self.fill;
        let backdrop = self.backdrop.clone();
        // No handles while an object is being drawn, as in the alternative.
        let creating = matches!(self.drag, Some(Drag::Create { .. }));
        let selected = self.selected.filter(|_| !creating).and_then(|id| self.object(id)).cloned();
        let selection = self.selection;
        let snap = self.snap;
        let guides = self.guides.clone();
        let editing = self.editing.as_ref().map(|e| (e.id, e.caret, e.selection(), e.marked.clone()));
        let input = self.editing.is_some().then(|| (cx.entity(), self.focus.clone()));
        let ruler = self.ruler_measures();
        let theme = cx.theme();
        let (bg, frame) = (theme.muted, theme.border);
        // Drawn over arbitrary pixels, so it can't follow the app theme: dark lines around a
        // white fill read on light and dark screenshots alike.
        let accent = hsla(0., 0., 0.29, 1.);
        let checker_base = theme.background;
        let dragging = self.drag.as_ref().map(|_| cx.weak_entity());
        let cursor = if self.space || matches!(self.drag, Some(Drag::Pan { .. })) {
            if matches!(self.drag, Some(Drag::Pan { .. })) {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            }
        } else if self.tool == Tool::Text {
            CursorStyle::IBeam
        } else {
            let grab = match &self.drag {
                Some(Drag::Selection { grab, .. }) => Some(*grab),
                None if self.tool == Tool::Select => {
                    self.cursor.and_then(|p| self.selection_grab(self.view().to_screen(p), p))
                }
                _ => None,
            };
            match grab {
                Some(SelGrab::Handle(0 | 4)) => CursorStyle::ResizeUpLeftDownRight,
                Some(SelGrab::Handle(2 | 6)) => CursorStyle::ResizeUpRightDownLeft,
                Some(SelGrab::Handle(1 | 5)) => CursorStyle::ResizeUpDown,
                Some(SelGrab::Handle(_)) => CursorStyle::ResizeLeftRight,
                _ => CursorStyle::Crosshair,
            }
        };
        let selection_label = selection.map(|s| {
            let unit = if unit_px { 1. } else { scale };
            format!(
                "{}×{} {}",
                (s.w / unit).round(),
                (s.h / unit).round(),
                if unit_px { "px" } else { "pt" }
            )
        });

        div()
            .id("canvas")
            .flex_1()
            .h_full()
            .relative()
            .overflow_hidden()
            .bg(bg)
            .cursor(cursor)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::mouse_down))
            .when(dragging.is_none(), |d| d.on_mouse_move(cx.listener(Self::mouse_move)))
            .on_scroll(cx.listener(Self::scroll))
            .on_pinch(cx.listener(|this, e: &PinchEvent, _, cx| this.zoom_at(this.view().k * (1. + e.delta), e.position, cx)))
            .child(
                canvas(
                    move |bounds, _, _| cell.set(bounds),
                    move |bounds, _, window, cx| {
                        let started = Instant::now();
                        if let Some((editor, focus)) = input {
                            window.handle_input(&focus, ElementInputHandler::new(bounds, editor), cx);
                        }
                        // Element listeners only fire under the cursor; a drag must follow it off the canvas and out of the window.
                        if let Some(editor) = dragging {
                            let on_move = editor.clone();
                            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                                if phase == DispatchPhase::Capture {
                                    let _ = on_move.update(cx, |this, cx| this.mouse_move(e, window, cx));
                                }
                            });
                            window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                                if phase == DispatchPhase::Capture {
                                    let _ = editor.update(cx, |this, cx| this.mouse_up(window, cx));
                                }
                            });
                        }
                        for old in stale.borrow_mut().drain(..) {
                            let _ = window.drop_image(old);
                        }
                        let view = compute_view(bounds, view_area, scale, zoom, pan);
                        paint_checker(bounds, checker_base, window);
                        let ctx = Ctx {
                            measure: &measure,
                            scale: ctx_scale,
                            unit_px,
                            text: text_lines.clone(),
                        };
                        let lookup = |key: u64| patches.borrow().get(&key).map(|p| (p.at, p.image.clone()));
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            if let Some(b) = &backdrop {
                                b.paint(view, canvas_rect, scale, window);
                            } else {
                                window.paint_quad(outline(view.rect(canvas_rect).dilate(px(1.)), frame, BorderStyle::Solid));
                            }
                            let canvas_b = view.rect(canvas_rect);
                            // Only grown canvas gets the fill, so transparent captures show the checkerboard.
                            if let Some(bg_fill) = bg_fill
                                && !image_rect.contains_rect(&canvas_rect)
                            {
                                window.paint_quad(fill_quad(canvas_b, paint::color(bg_fill, 1.), backdrop.as_ref().map_or(0., |b| b.inner_radius * view.k * scale)));
                            }
                            window.with_content_mask(Some(ContentMask { bounds: canvas_b }), |window| {
                                let visible = view.rect(image_rect).intersect(&canvas_b);
                                window.with_content_mask(Some(ContentMask { bounds: visible }), |window| {
                                    let mag = view.k * window.scale_factor();
                                    if mag >= CRISP_FROM
                                        && let Some((at, image)) = crisp_image(&full_image, view, visible.intersect(&bounds), mag, &crisp, &stale)
                                    {
                                        let b = view.rect(R::new(at.x as f32, at.y as f32, at.w as f32, at.h as f32));
                                        let _ = window.paint_image(b, b, Corners::default(), image, 0, false);
                                        return;
                                    }
                                    if let Some(old) = crisp.borrow_mut().take() {
                                        stale.borrow_mut().push(old.image);
                                    }
                                    for (r, tile) in tiles {
                                        let b = view.rect(R::new(r.x as f32, r.y as f32, r.w as f32, r.h as f32));
                                        if b.intersects(&bounds) {
                                            let _ = window.paint_image(b, b, Corners::default(), tile, 0, false);
                                        }
                                    }
                                });
                                let mut dimmed = false;
                                for o in doc::paint_order(&objects) {
                                    if !dimmed
                                        && matches!(o.shape, Shape::Spotlight { .. })
                                        && let Some(dim) = doc::spotlight_dim(&objects, canvas_rect.inflate(4000.))
                                    {
                                        dimmed = true;
                                        paint::prim(&dim, view, window, cx, &lookup);
                                    }
                                    for prim in o.prims(&ctx) {
                                        paint::prim(&prim, view, window, cx, &lookup);
                                    }
                                }
                            });
                            let pink = paint::color(0xF7157A, 1.);
                            for &(vertical, at) in &guides {
                                let (a, b) = if vertical {
                                    (view.to_screen((at, canvas_rect.y)), view.to_screen((at, canvas_rect.bottom())))
                                } else {
                                    (view.to_screen((canvas_rect.x, at)), view.to_screen((canvas_rect.right(), at)))
                                };
                                let (from, to) = if vertical { (a.y, b.y) } else { (a.x, b.x) };
                                let mut t = from;
                                while t < to {
                                    let dash = if vertical {
                                        Bounds::from_corners(point(a.x - px(0.5), t), point(a.x + px(0.5), (t + px(2.5)).min(to)))
                                    } else {
                                        Bounds::from_corners(point(t, a.y - px(0.5)), point((t + px(2.5)).min(to), a.y + px(0.5)))
                                    };
                                    window.paint_quad(fill(dash, pink));
                                    t += px(4.);
                                }
                            }
                            if let Some((id, caret, sel, marked)) = editing.clone()
                                && let Some(o) = objects.iter().find(|o| o.id == id)
                                && let Shape::Text { text, .. } = &o.shape
                                && let Some(r) = o.text_box(&ctx)
                            {
                                let caret_color = if o.variant == 0 { paint::color(doc::contrast(o.color), 1.) } else { paint::color(o.color, 1.) };
                                for line in label_runs(o, text, r, sel.clone()) {
                                    window.paint_quad(fill(view.rect(line), hsla(0.58, 0.9, 0.55, 0.4)));
                                }
                                for line in marked.map(|m| label_runs(o, text, r, m)).unwrap_or_default() {
                                    let under = R::new(line.x, line.y + line.h - o.size * 0.08, line.w, 0.);
                                    window.paint_quad(fill(view.rect(under).dilate(px(0.75)), caret_color));
                                }
                                if sel.is_empty() {
                                    let (x, y) = label_point(o, text, r, caret);
                                    window.paint_quad(fill(
                                        Bounds::new(view.to_screen((x, y + o.size * 0.12)), size(px(1.5), px(o.size * 1.05 * view.k))),
                                        caret_color,
                                    ));
                                }
                                window.paint_quad(quad(view.rect(r).dilate(px(3.)), px(4.), gpui_kit::transparent_black(), px(1.), accent, BorderStyle::Dashed));
                            }
                            if let Some(o) = &selected
                                && editing.is_none()
                            {
                                let r = view.rect(o.bounds(&ctx)).dilate(px(3.));
                                if o.handles().is_empty() {
                                    window.paint_quad(quad(r, px(4.), gpui_kit::transparent_black(), px(1.), accent, BorderStyle::Dashed));
                                }
                                for (_, p) in o.handles() {
                                    paint_handle(view.to_screen(p), accent, window);
                                }
                            }
                            if let Some(sel) = selection {
                                let s = view.rect(sel);
                                let dim = hsla(0., 0., 0., 0.25);
                                let (l, t, r, b) = (s.left(), s.top(), s.right(), s.bottom());
                                let (bl, bt, br, bb) = (bounds.left(), bounds.top(), bounds.right(), bounds.bottom());
                                for rect in [
                                    Bounds::from_corners(point(bl, bt), point(br, t)),
                                    Bounds::from_corners(point(bl, b), point(br, bb)),
                                    Bounds::from_corners(point(bl, t), point(l, b)),
                                    Bounds::from_corners(point(r, t), point(br, b)),
                                ] {
                                    window.paint_quad(fill(rect, dim));
                                }
                                window.paint_quad(outline(s.dilate(px(1.)), hsla(0., 0., 0.1, 0.75), BorderStyle::Solid));
                                if let Some(snap) = snap {
                                    window.paint_quad(outline(view.rect(snap), accent, BorderStyle::Solid));
                                }
                            }
                            for (from, to) in ruler {
                                let m = Object {
                                    id: 0,
                                    shape: Shape::Measure { from, to },
                                    color: doc::MEASURE,
                                    size: 1.,
                                    variant: 0,
                                };
                                for prim in m.prims(&ctx) {
                                    paint::prim(&prim, view, window, cx, &lookup);
                                }
                            }
                        });
                        let ms = started.elapsed().as_secs_f32() * 1000.;
                        if ms > 6. {
                            eprintln!("kiri: slow canvas paint {ms:.1} ms");
                        }
                    },
                )
                .size_full(),
            )
            .children(selection_label.zip(selection).map(|(label, sel)| {
                // As in the alternative: the size centered under the selection, the hint under it.
                let (s, cb) = (self.view().rect(sel), self.canvas_bounds.get());
                let top = (s.bottom() - cb.top() + px(6.)).min(cb.size.height - px(42.));
                v_flex()
                    .absolute()
                    .left(s.left() - cb.left())
                    .w(s.size.width)
                    .top(top)
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(11.))
                    .whitespace_nowrap()
                    .text_color(hsla(0., 0., 1., 0.95))
                    .child(div().px(px(5.)).py(px(1.)).rounded(px(4.)).bg(hsla(0., 0., 0.2, 0.75)).child(label))
                    .child("Hit Enter to crop")
            }))
            .children(self.render_props(cx))
            .children(self.toast.clone().map(|(text, action, done)| {
                div().absolute().bottom_5().left_0().right_0().flex().justify_center().child(
                    h_flex()
                        .id("toast")
                        .gap_2()
                        .px_3()
                        .py_1p5()
                        .rounded_full()
                        .bg(cx.theme().popover)
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_lg()
                        .text_sm()
                        .when(done, |d| d.child(Icon::new(IconName::Check).small().text_color(cx.theme().success)))
                        .child(text)
                        .when_some(action, |d, action| {
                            let label = match action {
                                ToastAction::Reveal(_) => crate::platform_text("Show in Finder", "Show in Explorer"),
                                ToastAction::Renumber => "Renumber",
                            };
                            d.cursor_pointer()
                                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(label))
                                .on_click(cx.listener(move |this, _, _, cx| match &action {
                                    ToastAction::Reveal(path) => cx.reveal_path(path),
                                    ToastAction::Renumber => {
                                        this.edit_doc(|this| doc::renumber_counters(Rc::make_mut(&mut this.objects).as_mut_slice()));
                                        this.toast = None;
                                        cx.notify();
                                    }
                                }))
                        }),
                )
            }))
    }
}

/// Text input for the label being edited, so input methods and dead keys compose in place.
impl EntityInputHandler for Editor {
    fn text_for_range(&mut self, range: Range<usize>, adjusted: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let (_, _, text) = self.edited_text()?;
        let r = te::from_utf16(text, range.start)..te::from_utf16(text, range.end);
        *adjusted = Some(te::to_utf16(text, r.start)..te::to_utf16(text, r.end));
        Some(text[r].to_string())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let (edit, _, text) = self.edited_text()?;
        let sel = edit.selection();
        Some(UTF16Selection { range: te::to_utf16(text, sel.start)..te::to_utf16(text, sel.end), reversed: edit.caret < edit.anchor })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let (edit, _, text) = self.edited_text()?;
        let m = edit.marked.clone()?;
        Some(te::to_utf16(text, m.start)..te::to_utf16(text, m.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        if let Some(edit) = self.editing.as_mut() {
            edit.marked = None;
        }
    }

    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, new: &str, _: &mut Window, cx: &mut Context<Self>) {
        let Some((range, mut text)) = self.input_range(range) else {
            return;
        };
        if let Some(edit) = self.editing.as_mut() {
            edit.replace(&mut text, range, new);
        }
        self.set_text(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new: &str,
        new_selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((range, mut text)) = self.input_range(range) else {
            return;
        };
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        text.replace_range(range.clone(), new);
        edit.marked = (!new.is_empty()).then(|| range.start..range.start + new.len());
        let sel = new_selected.map_or(new.len()..new.len(), |r| te::from_utf16(new, r.start)..te::from_utf16(new, r.end));
        (edit.anchor, edit.caret) = (range.start + sel.start, range.start + sel.end);
        self.set_text(text, cx);
    }

    fn bounds_for_range(&mut self, range: Range<usize>, _: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let (_, o, text) = self.edited_text()?;
        let r = o.text_box(&self.ctx())?;
        let view = self.view();
        let at = label_point(o, text, r, te::from_utf16(text, range.start));
        Some(Bounds::new(view.to_screen(at), size(px(1.), px(o.size * doc::LINE_HEIGHT * view.k))))
    }

    fn character_index_for_point(&mut self, point: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let i = self.text_index_at(self.view().to_image(point))?;
        let (_, _, text) = self.edited_text()?;
        Some(te::to_utf16(text, i))
    }
}

#[derive(Clone)]
enum ToastAction {
    Reveal(PathBuf),
    Renumber,
}

const CHECKER_CELL: f32 = 12.;
const CHECKER_CELLS: u32 = 16;

/// The workspace checkerboard, drawn as one cached tile repeated across `bounds`.
fn paint_checker(bounds: Bounds<Pixels>, base: Hsla, window: &mut Window) {
    thread_local!(static TILE: RefCell<Option<((u32, u32), Arc<RenderImage>)>> = RefCell::default());
    let bgra = |c: Hsla| {
        let c = Rgba::from(c);
        [c.b, c.g, c.r].map(|v| (v * 255.).round() as u8)
    };
    let alt = Hsla { l: base.l + if base.l < 0.5 { 0.035 } else { -0.035 }, ..base };
    let (a, b) = (bgra(base), bgra(alt));
    let scale = window.scale_factor();
    let key = (u32::from_le_bytes([a[0], a[1], a[2], 0]), scale.to_bits());
    let tile = TILE.with_borrow_mut(|cached| match cached {
        Some((k, tile)) if *k == key => tile.clone(),
        _ => {
            let cell = (CHECKER_CELL * scale).round().max(1.) as u32;
            let n = cell * CHECKER_CELLS;
            let mut pixels = Vec::with_capacity((n * n * 4) as usize);
            for y in 0..n {
                for x in 0..n {
                    let c = if (x / cell + y / cell) % 2 == 0 { a } else { b };
                    pixels.extend_from_slice(&[c[0], c[1], c[2], 255]);
                }
            }
            let tile = Arc::new(crate::tiled::frame_image(n, n, pixels));
            *cached = Some((key, tile.clone()));
            tile
        }
    });
    let step = px(CHECKER_CELL * CHECKER_CELLS as f32);
    let mut y = bounds.origin.y;
    while y < bounds.bottom() {
        let mut x = bounds.origin.x;
        while x < bounds.right() {
            let b = Bounds::new(point(x, y), size(step, step));
            let _ = window.paint_image(b, b, Corners::default(), tile.clone(), 0, false);
            x += step;
        }
        y += step;
    }
}

fn fill_quad(b: Bounds<Pixels>, color: Hsla, radius: f32) -> PaintQuad {
    quad(b, px(radius), color, px(0.), gpui_kit::transparent_black(), BorderStyle::Solid)
}

fn paint_handle(p: Point<Pixels>, accent: Hsla, window: &mut Window) {
    let b = Bounds::new(p - point(px(HANDLE), px(HANDLE)), size(px(HANDLE * 2.), px(HANDLE * 2.)));
    window.paint_quad(quad(b, px(HANDLE), gpui_kit::white(), px(1.5), accent, BorderStyle::Solid));
}

fn ruler_hint() -> impl IntoElement {
    Popover::new("ruler-hint")
        .anchor(Anchor::TopLeft)
        .trigger(Button::new("ruler").icon(Icon::new(IconName::Ruler)).ghost().small().tooltip("Ruler"))
        .content(|_, _, cx| {
            let popover = cx.entity();
            v_flex()
                .w(px(260.))
                .gap_3()
                .child(div().text_sm().font_medium().child("Use the keyboard"))
                .child(div().text_sm().child(
                    "Hold 1 or 2, or an arrow key, and move the mouse over what you want to measure. Click to keep the measurement.",
                ))
                .child(
                    Button::new("got-it")
                        .primary()
                        .small()
                        .label("Got it")
                        .on_click(move |_, window, cx| popover.update(cx, |p, cx| p.dismiss(window, cx))),
                )
        })
}

fn tool_icon(tool: Tool) -> IconName {
    match tool {
        Tool::Select => IconName::SquareDashedMousePointer,
        Tool::Arrow => IconName::MoveUpRight,
        Tool::Text => IconName::Type,
        Tool::Rect => IconName::Square,
        Tool::Oval => IconName::Circle,
        Tool::Line => IconName::Slash,
        Tool::Pen => IconName::Pencil,
        Tool::Highlighter => IconName::Highlighter,
        Tool::Counter => IconName::CircleDot,
        Tool::Redact => IconName::Grid3x3,
        Tool::Spotlight => IconName::Flashlight,
        Tool::Magnifier => IconName::Search,
        Tool::Image => IconName::Square,
    }
}

const RESIZE_PRESETS: [(&str, f32); 6] = [("25%", 0.25), ("33%", 1. / 3.), ("50%", 0.5), ("1×", 1.), ("2×", 2.), ("4×", 4.)];

fn size_popover(editor: &Entity<Editor>, cx: &mut Context<PopoverState>) -> AnyElement {
    let e = editor.read(cx);
    let ((w, h), fields, unit_px, (fw, fh)) = (e.canvas_px(), e.resize_fields(cx), e.unit_px, e.resize.clone());
    let (muted, border) = (cx.theme().muted_foreground, cx.theme().border);
    let popover = cx.entity();
    let at = |v: u32, k: f32| ((v as f32 * k).round() as u32).max(1);
    v_flex()
        .w(px(300.))
        .gap_2()
        .child(
            h_flex()
                .justify_between()
                .child(div().text_sm().font_medium().child("Show size in points"))
                .child(Switch::new("points").checked(!unit_px).on_click({
                    let editor = editor.clone();
                    move |_, _, cx| editor.update(cx, |this, cx| this.toggle_unit(cx))
                })),
        )
        .child(div().text_xs().text_color(muted).child(crate::platform_text("A point is two pixels on a Retina display. P switches.", "A point is one pixel at 100% scale. P switches.")))
        .child(div().h(px(1.)).my_1().bg(border))
        .child(
            h_flex()
                .gap_2()
                .child(div().text_sm().font_medium().child("Resize image"))
                .child(div().text_xs().text_color(muted).child("merges annotations")),
        )
        .child(h_flex().gap_1().children(RESIZE_PRESETS.iter().enumerate().map(|(i, &(label, k))| {
            let editor = editor.clone();
            Button::new(("preset", i))
                .ghost()
                .small()
                .flex_1()
                .selected(fields == Some((at(w, k), at(h, k))))
                .label(label)
                .on_click(move |_, window, cx| editor.update(cx, |this, cx| this.set_resize_fields(k, window, cx)))
        })))
        .child(
            h_flex()
                .gap_2()
                .child(Input::new(&fw).small().w(px(76.)))
                .child("×")
                .child(Input::new(&fh).small().w(px(76.)))
                .child(div().text_color(muted).child("px"))
                .child(div().flex_1())
                .child(
                    Button::new("resize")
                        .primary()
                        .small()
                        .label("Resize")
                        .disabled(fields.is_none_or(|f| f == (w, h)))
                        .on_click({
                            let editor = editor.clone();
                            move |_, window, cx| {
                                editor.update(cx, |this, cx| this.resize_to_fields(cx));
                                popover.update(cx, |p, cx| p.dismiss(window, cx));
                            }
                        }),
                ),
        )
        .into_any_element()
}
