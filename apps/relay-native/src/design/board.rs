//! One design open on the canvas: the top bar, the canvas and what floats over it (the tool
//! dock, the selection's toolbar, the layers and the inspector, zoom), and the commands they
//! share. Input is `tools.rs`; editing a text in place is `textedit.rs`.

use super::canvas::Canvas;
use super::doc::{Change, Doc};
use super::glyphs;
use super::paint::{self, fill_of, grect, rgba, Painter, Posing, CANVAS};
use super::widgets::tip;
use crate::app::{label, Ui};
use gtk::prelude::*;
use gtk::{gdk, gio, graphene, gsk, pango};
use gtk4 as gtk;
use relay_bus::design::{Layout, Node, NodeKind, NodeOp, Scroll};
use relay_client::design::{bounds, edit, shapes};
use relay_client::design::geom::step_zoom;
use relay_client::design::layout::{self, Placed, Scrolled};
use relay_client::design::motion::{self, Clocks};
use relay_client::design::snap::Guide;
use relay_client::design::{Camera, Rect, Scene};
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

/// Selection, hover and handles.
pub const SELECT: &str = "#4C9DFF";
/// Smart guides and spacing.
pub const GUIDE: &str = "#FF3D71";
/// What an agent just changed glows briefly in the live colour.
pub const AGENT: &str = "#2EC469";
/// Clipboard type for nodes copied between designs.
pub const MIME: &str = "application/x-relay-design+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Select,
    Hand,
    Frame,
    Rect,
    Ellipse,
    Line,
    Arrow,
    Text,
    Sticky,
    Pen,
    Highlighter,
    Triangle,
    Diamond,
    Hexagon,
    Star,
    Eraser,
    /// Click to place points, click the first again to close: a shape of your own.
    Vector,
}

impl Tool {
    /// The shape tools that share the dock's shape key.
    pub const SHAPES: [Tool; 8] =
        [Tool::Rect, Tool::Ellipse, Tool::Triangle, Tool::Diamond, Tool::Hexagon, Tool::Star, Tool::Line, Tool::Arrow];

    pub fn is_shape(self) -> bool {
        Self::SHAPES.contains(&self)
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Tool::Select => "pointer",
            Tool::Hand => "hand",
            Tool::Frame => "frame",
            Tool::Rect => "rect",
            Tool::Ellipse => "ellipse",
            Tool::Line => "line",
            Tool::Arrow => "arrow",
            Tool::Text => "text",
            Tool::Sticky => "sticky",
            Tool::Pen => "pen",
            Tool::Highlighter => "highlighter",
            Tool::Triangle => "triangle",
            Tool::Diamond => "diamond",
            Tool::Hexagon => "hexagon",
            Tool::Star => "star",
            Tool::Eraser => "eraser",
            Tool::Vector => "vector",
        }
    }

    pub fn caption(self) -> &'static str {
        match self {
            Tool::Select => "Move",
            Tool::Hand => "Hand",
            Tool::Frame => "Frame",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Line => "Line",
            Tool::Arrow => "Arrow",
            Tool::Text => "Text",
            Tool::Sticky => "Sticky note",
            Tool::Pen => "Pen",
            Tool::Highlighter => "Highlighter",
            Tool::Triangle => "Triangle",
            Tool::Diamond => "Diamond",
            Tool::Hexagon => "Hexagon",
            Tool::Star => "Star",
            Tool::Eraser => "Eraser",
            Tool::Vector => "Vector pen",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::Hand => "H",
            Tool::Frame => "F",
            Tool::Rect => "R",
            Tool::Ellipse => "O",
            Tool::Line => "L",
            Tool::Arrow => "Shift L",
            Tool::Text => "T",
            Tool::Sticky => "S",
            Tool::Pen => "P",
            Tool::Highlighter => "Shift P",
            Tool::Triangle => "Shift R",
            Tool::Diamond => "",
            Tool::Hexagon => "",
            Tool::Star => "Shift S",
            Tool::Eraser => "E",
            Tool::Vector => "B",
        }
    }

    /// Its key's tooltip: "Arrow (Shift+L)", or the caption alone when it has no key.
    pub fn tip(self) -> String {
        match self.key() {
            "" => self.caption().to_string(),
            key => format!("{} ({})", self.caption(), key.replace(' ', "+")),
        }
    }
}

/// A resize handle, by the edges it moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handle {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl Handle {
    pub fn cursor(self) -> &'static str {
        match (self.left, self.right, self.top, self.bottom) {
            (true, _, true, _) | (_, true, _, true) => "nwse-resize",
            (true, _, _, true) | (_, true, true, _) => "nesw-resize",
            (true, _, _, _) | (_, true, _, _) => "ew-resize",
            _ => "ns-resize",
        }
    }
}

pub const HANDLES: [Handle; 8] = [
    Handle { left: true, right: false, top: true, bottom: false },
    Handle { left: false, right: false, top: true, bottom: false },
    Handle { left: false, right: true, top: true, bottom: false },
    Handle { left: false, right: true, top: false, bottom: false },
    Handle { left: false, right: true, top: false, bottom: true },
    Handle { left: false, right: false, top: false, bottom: true },
    Handle { left: true, right: false, top: false, bottom: true },
    Handle { left: true, right: false, top: false, bottom: false },
];

/// A pointer gesture under way. One lives at a time, so its size does not matter.
#[allow(clippy::large_enum_variant)]
pub enum Gesture {
    Idle,
    Pan { cam: Camera },
    Marquee { scope: String, a: (f64, f64), b: (f64, f64), base: Vec<String>, moved: bool },
    Move {
        ids: Vec<String>,
        start: (f64, f64),
        orig: Vec<(String, f64, f64)>,
        rects: Vec<Rect>,
        dup: bool,
        /// Alt drag: the copies made at the first move, shown but not sent until the drag ends,
        /// so the copy and its move are one undo step.
        copies: Vec<NodeOp>,
        moved: bool,
        /// A container the nodes would land in, when it is not their parent, and the place they
        /// would take there when it lays out its children.
        target: Option<String>,
        into: Option<u32>,
        /// In an auto-layout parent: the place the node would take.
        reorder: Option<(String, u32)>,
        /// The node to select if the press ends as a click.
        click: Option<String>,
        at: (f64, f64),
        /// How far the free ones show moved, snapped, in their parent's coordinates.
        shown: (f64, f64),
        /// Alt drag: the layers copied, selected again if Esc drops the copies (E-board-11).
        sources: Vec<String>,
        /// What the laid-out ones paint, drawn under the pointer as they are dragged: their
        /// layout keeps them in place until the drop (E-board-10).
        ghost: Option<gsk::RenderNode>,
    },
    /// `bounds` and the pointer are in `frame`'s coordinates when one node drawn turned is
    /// resized (its parent's; B12), else on the page; `turn` is that node's own.
    /// `kept`: the pinned layers inside the frames resized, as they were, which the preview moves
    /// as the follow rule will and puts back each update.
    Resize { handle: Handle, bounds: Rect, orig: Vec<(String, Node, Rect)>, kept: Vec<Node>, moved: bool, frame: Option<String>, turn: f64 },
    Endpoint { id: String, which: usize, orig: Node, moved: bool },
    /// `from`/`to`: the nodes a line's ends attach to, a connector's.
    Create { tool: Tool, parent: String, a: (f64, f64), b: (f64, f64), from: Option<String>, to: Option<String> },
    /// A pen or highlighter stroke: the tool it began with draws it, whatever is picked since (B17).
    Draw { parent: String, points: Vec<[f64; 2]>, tool: Tool },
    /// Dragging the centre of one node's radial or conic gradient (`field`: `gradient` or
    /// `stroke_gradient`) within the box it paints, `drawn` on the page and turned by `turn`:
    /// the glow moves, the layer stays in its frame.
    Centre { id: String, field: &'static str, drawn: Rect, turn: f64, orig: Node, moved: bool },
    /// Turning one node about its centre (page coordinates), from the pointer's first angle.
    Rotate { id: String, centre: (f64, f64), start: f64, orig: Node, moved: bool },
    /// Rubbing out strokes: each touched one with the opacity it had.
    Erase { hits: Vec<(String, Option<f64>)> },
}

/// The smallest a layer's box is on the screen, either way, for its gradient's centre to take a
/// handle: below it a press at its middle moves the layer.
const CENTRE_MIN_BOX: f64 = 48.0;

/// The inspector's width, beside the canvas's right edge.
const INSPECTOR_W: f64 = 256.0;

/// What the inspector key does now: hide it while it is on, show it while it is off.
fn inspector_tip(on: bool) -> &'static str {
    if on { "Hide the inspector (Ctrl+.)" } else { "Show the inspector (Ctrl+.)" }
}

pub struct Board {
    pub root: gtk::Box,
    pub(super) ui: Weak<Ui>,
    pub doc: Rc<Doc>,
    pub(super) canvas: Canvas,
    pub(super) overlay: gtk::Overlay,
    pub(super) painter: Rc<Painter>,
    pub(super) page: RefCell<String>,
    pub(super) placed: RefCell<Option<Rc<Placed>>>,
    /// How far each scrolling frame is scrolled on the canvas (Alt+wheel), by frame id: view
    /// state, never stored or exported, gone when the board closes (feature-layout §2.5).
    pub(super) scroll: RefCell<HashMap<String, (f64, f64)>>,
    pub(super) content: RefCell<Option<gsk::RenderNode>>,
    pub(super) cam: Cell<Camera>,
    pub(super) cams: RefCell<HashMap<String, Camera>>,
    pub(super) size: Cell<(f64, f64)>,
    fitted: Cell<bool>,
    /// A view was put back (a pop-out's): the one kept in the settings does not replace it.
    restored: Cell<bool>,
    pub(super) selection: RefCell<Vec<String>>,
    pub(super) hover: RefCell<Option<String>>,
    pub(super) tool: Cell<Tool>,
    pub(super) tool_lock: Cell<bool>,
    pub(super) gesture: RefCell<Gesture>,
    pub(super) guides: RefCell<Vec<Guide>>,
    /// The frame whose coordinates the guides are in, a turned one's; the page's when none.
    pub(super) guides_in: RefCell<Option<String>>,
    pub(super) space: Cell<bool>,
    /// Shift is held, as the keys and the pointer last told.
    pub(super) shift_held: Cell<bool>,
    pub(super) pointer: Cell<(f64, f64)>,
    /// The drag under way's last update (screen point, offset, modifiers), which auto-pan
    /// runs again as the camera moves under a still pointer.
    pub(super) drag_at: Cell<Option<DragAt>>,
    pub(super) autopan: Cell<bool>,
    /// The last primary press, for telling a double click: time in µs and where.
    pub(super) last_press: Cell<(i64, f64, f64)>,
    flash: RefCell<(HashSet<String>, i64)>,
    flash_tick: RefCell<Option<gtk::TickCallbackId>>,
    labels: RefCell<HashMap<String, pango::Layout>>,
    /// The colour picked for each drawing tool (pen, highlighter, vector pen); none is automatic.
    pub(super) pens: RefCell<HashMap<Tool, String>>,
    pub(super) clip: RefCell<Vec<Node>>,
    /// Where the context menu was opened, on the page: its Paste puts things there (E-board-15).
    pub(super) menu_at: Cell<Option<(f64, f64)>>,
    /// A copied style (Ctrl+Alt+C), to lay on other layers.
    pub(super) style_clip: RefCell<Option<serde_json::Map<String, Value>>>,
    /// The vector pen's path being placed: its parent and its points, in page coordinates.
    pub(super) vector: RefCell<Option<(String, Vec<[f64; 2]>)>>,
    pub(super) title: gtk::Label,
    pub(super) terminal: gtk::MenuButton,
    pub(super) terminal_label: gtk::Label,
    /// The terminal's state beside its name, as the gallery's chips show it.
    terminal_lamp: gtk::Box,
    lamp_state: RefCell<String>,
    pub(super) status: gtk::Label,
    /// Counts what the status said, so a timer ages only the word it was set for.
    status_serial: Cell<u64>,
    /// Undo the last change an agent made, shown once one arrives.
    pub(super) agent_undo: gtk::Button,
    /// Go to the page an agent changed while another was on show, and what it changed there.
    goto: gtk::Button,
    goto_page: RefCell<Option<(String, HashSet<String>)>>,
    /// Whose change that Undo reverts.
    agent_last: RefCell<Option<String>>,
    /// An Undo agent is on its way: another press is dropped (B80).
    agent_undoing: Cell<bool>,
    /// The audit row whose undo was refused as edited since, and when: a second press soon
    /// after undoes it anyway.
    agent_force: Cell<Option<(i64, i64)>>,
    pub(super) ask: gtk::Button,
    /// The design's terminal on the canvas, as a terminal or a conversation, while it is open,
    /// and the top bar's key that opens it.
    pub(super) agent_panel: RefCell<Option<Rc<super::agent_panel::AgentPanel>>>,
    agent_key: gtk::ToggleButton,
    /// What was typed to Ask and not sent, for the next time it opens.
    pub(super) ask_draft: RefCell<String>,
    /// The image key's stock photo picker: its last search, and the picker while it shows.
    pub(super) pictures: RefCell<super::pictures::State>,
    pub(super) zoom_label: gtk::Label,
    pub(super) tool_keys: RefCell<Vec<(Tool, gtk::ToggleButton)>>,
    pub(super) shape_key: gtk::ToggleButton,
    pub(super) pen_key: gtk::ToggleButton,
    pub(super) page_label: gtk::Label,
    /// The dock's page list, rebuilt while open when the pages change.
    pages_popover: RefCell<Option<gtk::Popover>>,
    pub(super) context: RefCell<Option<gtk::Widget>>,
    pub(super) context_at: Cell<(i32, i32)>,
    /// The toolbar's width with its words showing, measured before it was made compact.
    pub(super) context_full: Cell<Option<i32>>,
    /// Its words are hidden and its keys tighter: it did not fit beside the inspector.
    pub(super) context_compact: Cell<bool>,
    /// What the toolbar was built for (text, shape, alignment…), so a rebuild for the same kind
    /// starts compact rather than showing its words for a frame.
    pub(super) context_kind: Cell<&'static str>,
    context_refit: Cell<bool>,
    /// What brings the toolbar's values up to date in place, for the selection's nodes.
    #[allow(clippy::type_complexity)]
    pub(super) context_values: RefCell<Vec<Box<dyn Fn(&[Node])>>>,
    pub(super) inspector: gtk::Box,
    pub(super) inspector_scroll: gtk::ScrolledWindow,
    /// The tools along the top left, which the inspector keeps clear of.
    dock: gtk::Box,
    pub(super) inspector_key: gtk::ToggleButton,
    pub(super) layers: RefCell<Option<Rc<super::layers::Layers>>>,
    pub(super) layers_panel: gtk::Revealer,
    pub(super) edit: RefCell<Option<super::textedit::Edit>>,
    pub(super) toast: gtk::Revealer,
    pub(super) toast_label: gtk::Label,
    toast_serial: Cell<u64>,
    chrome_pending: Cell<bool>,
    pub(super) actions: gio::SimpleActionGroup,
    /// Back to the gallery, when the board sits in the Design view.
    pub(super) back: RefCell<Option<Rc<dyn Fn()>>>,
    pub(super) windowed: bool,
    /// Play, while the page's motion plays (or is paused): never written to the design.
    play: RefCell<Option<Play>>,
    /// The top bar's Play key, shown while something on the page moves.
    play_key: gtk::Button,
    timebar: TimeBar,
    me: Weak<Board>,
}

impl Board {
    pub fn new(ui: &Rc<Ui>, doc: Rc<Doc>, windowed: bool) -> Rc<Board> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("design-board");
        root.set_hexpand(true);
        root.set_vexpand(true);

        // Top bar: back to the gallery, the title and its menu, the terminal, saving, Ask.
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.add_css_class("design-topbar");
        let back_key = gtk::Button::new();
        back_key.add_css_class("quiet");
        back_key.add_css_class("design-crumb");
        let back_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        back_row.append(&crate::icons::image("chevron-left", 14));
        back_row.append(&label("Designs", ""));
        back_key.set_child(Some(&back_row));
        back_key.set_tooltip_text(Some("All designs of this project"));
        back_key.set_widget_name("design-back");
        top.append(&back_key);
        let slash = label("/", "faint");
        top.append(&slash);
        let title = label("", "design-title");
        title.set_ellipsize(pango::EllipsizeMode::End);
        title.set_max_width_chars(48);
        let title_key = gtk::MenuButton::new();
        title_key.add_css_class("quiet");
        title_key.add_css_class("design-title-key");
        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        title_row.append(&title);
        title_row.append(&crate::icons::image("chevron-down", 12));
        title_key.set_child(Some(&title_row));
        title_key.set_tooltip_text(Some("Rename, duplicate, export or delete this design"));
        top.append(&title_key);
        let terminal_label = label("", "");
        let terminal_lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        terminal_lamp.add_css_class("design-lamp");
        terminal_lamp.set_valign(gtk::Align::Center);
        terminal_lamp.set_visible(false);
        let terminal_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        terminal_row.append(&terminal_lamp);
        terminal_row.append(&terminal_label);
        let terminal = gtk::MenuButton::new();
        terminal.add_css_class("design-terminal");
        terminal.set_child(Some(&terminal_row));
        terminal.set_valign(gtk::Align::Center);
        top.append(&terminal);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        top.append(&spacer);
        let status = label("Saved", "design-status");
        status.set_width_chars(22);
        status.set_xalign(1.0);
        // Cut at its end, so "kevin edited it just now" keeps the name.
        status.set_ellipsize(pango::EllipsizeMode::End);
        top.append(&status);
        let agent_undo = gtk::Button::with_label("Undo");
        agent_undo.add_css_class("design-agent-undo");
        agent_undo.set_widget_name("design-agent-undo");
        agent_undo.set_visible(false);
        top.append(&agent_undo);
        let goto = gtk::Button::with_label("Show");
        goto.add_css_class("design-agent-undo");
        goto.set_widget_name("design-agent-goto");
        goto.set_visible(false);
        top.insert_child_after(&goto, Some(&status));
        let ask = gtk::Button::new();
        ask.add_css_class("design-ask");
        let ask_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        ask_row.append(&glyphs::image("spark", 14));
        ask_row.append(&label("Ask agent", ""));
        ask.set_child(Some(&ask_row));
        ask.set_tooltip_text(Some("Ask the terminal's agent to change the selection or this page (Ctrl+Enter)"));
        ask.set_widget_name("design-ask");
        let agent_key = gtk::ToggleButton::new();
        agent_key.add_css_class("quiet");
        agent_key.add_css_class("icon-key");
        agent_key.set_child(Some(&crate::icons::image("terminal", 16)));
        tip(&agent_key, "The design's terminal on the canvas, to move and resize: as its terminal or as a conversation");
        agent_key.set_widget_name("design-agent-key");
        let play_key = crate::app::icon_button("play", PLAY_TIP);
        play_key.set_widget_name("design-play");
        play_key.set_visible(false);
        top.append(&play_key);
        top.append(&agent_key);
        top.append(&ask);
        let pop_out = crate::app::icon_button("window", "Open in a new window");
        pop_out.set_child(Some(&glyphs::image("window", 16)));
        pop_out.set_visible(!windowed);
        top.append(&pop_out);
        let inspector_key = gtk::ToggleButton::new();
        inspector_key.add_css_class("quiet");
        inspector_key.add_css_class("icon-key");
        inspector_key.set_child(Some(&crate::icons::image("sidebar", 16)));
        // As it was left on the last design opened.
        let inspector_on = INSPECTOR_ON.with(Cell::get);
        tip(&inspector_key, inspector_tip(inspector_on));
        inspector_key.set_active(inspector_on);
        top.append(&inspector_key);
        let mut key = top.first_child();
        while let Some(widget) = key {
            key = widget.next_sibling();
            widget.set_valign(gtk::Align::Center);
        }
        // The inspector key, Undo and the rest leave the canvas its shortcuts.
        keep_canvas_focus(top.upcast_ref());
        root.append(&top);

        let canvas = Canvas::new();
        let overlay = gtk::Overlay::new();
        overlay.add_css_class("design-stage");
        overlay.set_child(Some(&canvas));
        overlay.set_vexpand(true);
        overlay.set_hexpand(true);
        root.append(&overlay);

        // The dock: layers, page, then the tools.
        let dock = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        dock.add_css_class("design-dock");
        dock.set_halign(gtk::Align::Start);
        dock.set_valign(gtk::Align::Start);
        dock.set_margin_start(12);
        dock.set_margin_top(12);
        overlay.add_overlay(&dock);

        let page_label = label("Page 1", "");
        page_label.set_max_width_chars(14);
        page_label.set_ellipsize(pango::EllipsizeMode::End);

        let inspector = gtk::Box::new(gtk::Orientation::Vertical, 0);
        inspector.add_css_class("design-inspector-body");
        let inspector_scroll = gtk::ScrolledWindow::new();
        inspector_scroll.set_child(Some(&inspector));
        inspector_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        inspector_scroll.set_propagate_natural_height(true);
        inspector_scroll.set_max_content_height(640);
        inspector_scroll.add_css_class("design-inspector");
        inspector_scroll.set_halign(gtk::Align::End);
        inspector_scroll.set_valign(gtk::Align::Start);
        inspector_scroll.set_margin_end(12);
        inspector_scroll.set_margin_top(12);
        inspector_scroll.set_size_request(INSPECTOR_W as i32, -1);
        inspector_scroll.set_visible(false);
        overlay.add_overlay(&inspector_scroll);

        let layers_panel = gtk::Revealer::new();
        layers_panel.set_transition_type(gtk::RevealerTransitionType::SlideRight);
        layers_panel.set_transition_duration(140);
        layers_panel.set_halign(gtk::Align::Start);
        layers_panel.set_valign(gtk::Align::Fill);
        layers_panel.set_margin_start(12);
        layers_panel.set_margin_top(62);
        layers_panel.set_margin_bottom(12);
        overlay.add_overlay(&layers_panel);

        // Zoom, bottom right.
        let zoom_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        zoom_bar.add_css_class("design-zoom");
        zoom_bar.set_halign(gtk::Align::End);
        zoom_bar.set_valign(gtk::Align::End);
        zoom_bar.set_margin_end(12);
        zoom_bar.set_margin_bottom(12);
        let zoom_out = crate::app::icon_button("minus", "Zoom out (Ctrl+-)");
        let zoom_label = label("100%", "design-zoom-label");
        zoom_label.set_width_chars(5);
        zoom_label.set_xalign(0.5);
        let zoom_menu = gtk::MenuButton::new();
        zoom_menu.add_css_class("quiet");
        zoom_menu.set_child(Some(&zoom_label));
        zoom_menu.set_tooltip_text(Some("Zoom"));
        let zoom_in = crate::app::icon_button("plus", "Zoom in (Ctrl+=)");
        let help = crate::app::icon_button("keyboard", "Keyboard shortcuts (?)");
        help.set_child(Some(&glyphs::image("keyboard", 16)));
        zoom_bar.append(&zoom_out);
        zoom_bar.append(&zoom_menu);
        zoom_bar.append(&zoom_in);
        zoom_bar.append(&help);
        keep_canvas_focus(zoom_bar.upcast_ref());
        overlay.add_overlay(&zoom_bar);

        let toast = gtk::Revealer::new();
        toast.set_transition_type(gtk::RevealerTransitionType::Crossfade);
        toast.set_halign(gtk::Align::Center);
        toast.set_valign(gtk::Align::End);
        toast.set_margin_bottom(16);
        toast.set_can_target(false);
        let toast_label = label("", "design-toast");
        toast.set_child(Some(&toast_label));
        overlay.add_overlay(&toast);
        let timebar = time_bar();
        overlay.add_overlay(&timebar.root);

        let shape_key = gtk::ToggleButton::new();
        let pen_key = gtk::ToggleButton::new();
        let painter = Painter::new(&canvas);
        *painter.assets_dir.borrow_mut() = doc.assets_dir.borrow().clone();
        let first_page = doc.scene.borrow().first_page().cloned().unwrap_or_default();
        let actions = gio::SimpleActionGroup::new();
        root.insert_action_group("design", Some(&actions));

        let board = Rc::new_cyclic(|me| Board {
            root,
            ui: Rc::downgrade(ui),
            doc,
            canvas,
            overlay,
            painter,
            page: RefCell::new(first_page),
            placed: RefCell::default(),
            scroll: RefCell::default(),
            content: RefCell::default(),
            cam: Cell::new(Camera::default()),
            cams: RefCell::default(),
            size: Cell::new((0.0, 0.0)),
            fitted: Cell::new(false),
            restored: Cell::new(false),
            selection: RefCell::default(),
            hover: RefCell::default(),
            tool: Cell::new(Tool::Select),
            tool_lock: Cell::new(false),
            gesture: RefCell::new(Gesture::Idle),
            guides: RefCell::default(),
            guides_in: RefCell::default(),
            space: Cell::new(false),
            shift_held: Cell::new(false),
            pointer: Cell::new((0.0, 0.0)),
            drag_at: Cell::new(None),
            autopan: Cell::new(false),
            last_press: Cell::new((0, 0.0, 0.0)),
            flash: RefCell::new((HashSet::new(), 0)),
            flash_tick: RefCell::default(),
            labels: RefCell::default(),
            pens: RefCell::default(),
            clip: RefCell::default(),
            style_clip: RefCell::default(),
            menu_at: Cell::new(None),
            vector: RefCell::default(),
            title,
            terminal,
            terminal_label,
            terminal_lamp,
            lamp_state: RefCell::default(),
            status,
            status_serial: Cell::new(0),
            agent_undo,
            goto,
            goto_page: RefCell::default(),
            agent_last: RefCell::default(),
            agent_undoing: Cell::new(false),
            agent_force: Cell::new(None),
            ask,
            agent_panel: RefCell::default(),
            agent_key,
            ask_draft: RefCell::default(),
            pictures: RefCell::default(),
            zoom_label,
            tool_keys: RefCell::default(),
            shape_key,
            pen_key,
            page_label,
            pages_popover: RefCell::default(),
            context: RefCell::default(),
            context_at: Cell::new((0, 0)),
            context_full: Cell::new(None),
            context_compact: Cell::new(false),
            context_kind: Cell::new(""),
            context_refit: Cell::new(false),
            context_values: RefCell::default(),
            inspector,
            inspector_scroll,
            dock: dock.clone(),
            inspector_key,
            layers: RefCell::default(),
            layers_panel,
            edit: RefCell::default(),
            toast,
            toast_label,
            toast_serial: Cell::new(0),
            chrome_pending: Cell::new(false),
            actions,
            back: RefCell::default(),
            windowed,
            play: RefCell::default(),
            play_key,
            timebar,
            me: me.clone(),
        });

        let weak = Rc::downgrade(&board);
        board.canvas.set_draw(move |snap, w, h| {
            if let Some(b) = weak.upgrade() {
                b.draw(snap, w, h);
            }
        });
        let weak = Rc::downgrade(&board);
        board.canvas.set_resized(move |w, h| {
            if let Some(b) = weak.upgrade() {
                b.resized(w as f64, h as f64);
            }
        });
        let weak = Rc::downgrade(&board);
        board.painter.on_loaded(move || {
            if let Some(b) = weak.upgrade() {
                b.content.borrow_mut().take();
                b.painter.clear_stills();
                // An SVG that animates itself may have just loaded: it plays, and Play is offered.
                b.refresh_play();
                b.refresh_play_key();
                b.canvas.queue_draw();
            }
        });
        let weak = Rc::downgrade(&board);
        board.doc.listen(move |change| {
            if let Some(b) = weak.upgrade() {
                b.changed(change);
            }
        });
        // Floating widgets that follow the selection are placed by the board.
        let weak = Rc::downgrade(&board);
        board.overlay.connect_get_child_position(move |_, widget| {
            let b = weak.upgrade()?;
            b.child_position(widget)
        });

        board.hand_back(&board.inspector_scroll);
        board.build_dock(&dock);
        board.build_zoom_menu(&zoom_menu);
        board.install_actions();
        board.wire_play();
        board.build_title_menu(&title_key);
        let layers = super::layers::Layers::new(&board);
        board.layers_panel.set_child(Some(&layers.root));
        *board.layers.borrow_mut() = Some(layers);
        super::tools::install(&board);

        let weak = Rc::downgrade(&board);
        back_key.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.stop_play();
                b.end_edit(true);
                if let Some(back) = b.back.borrow().clone() {
                    back();
                }
            }
        });
        back_key.set_visible(!windowed);
        slash.set_visible(!windowed);
        let weak = Rc::downgrade(&board);
        pop_out.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.pop_out();
            }
        });
        let weak = Rc::downgrade(&board);
        board.ask.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                b.ask_agent(key.upcast_ref());
            }
        });
        let weak = Rc::downgrade(&board);
        board.agent_key.connect_toggled(move |key| {
            if let Some(b) = weak.upgrade() {
                b.set_agent_panel(key.is_active());
            }
        });
        if AGENT_PANEL_ON.with(Cell::get) {
            board.agent_key.set_active(true);
        }
        let weak = Rc::downgrade(&board);
        board.agent_undo.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.undo_agent();
            }
        });
        let weak = Rc::downgrade(&board);
        board.goto.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.go_to_change();
            }
        });
        let weak = Rc::downgrade(&board);
        board.inspector_key.connect_toggled(move |key| {
            tip(key, inspector_tip(key.is_active()));
            INSPECTOR_ON.with(|on| on.set(key.is_active()));
            if let Some(b) = weak.upgrade() {
                b.refresh_chrome();
            }
        });
        let weak = Rc::downgrade(&board);
        zoom_out.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.step_zoom(-1);
            }
        });
        let weak = Rc::downgrade(&board);
        zoom_in.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.step_zoom(1);
            }
        });
        let weak = Rc::downgrade(&board);
        help.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                b.shortcuts(key.upcast_ref());
            }
        });
        board.design_changed();
        board.refresh_terminal_menu();
        board.restore_view();
        // Opened on a design an agent changed last: its Undo is still offered (E-ask-9).
        let by = board.doc.design.borrow().updated_by.clone();
        if !by.is_empty() && by != "user" {
            board.offer_agent_undo(by);
        }
        board
    }

    /// What the board shows, to show it again elsewhere (E-gallery-6): its page, the camera on
    /// each page, the selection and its panels.
    pub(super) fn state(&self) -> BoardState {
        let mut cams = self.cams.borrow().clone();
        cams.insert(self.page.borrow().clone(), self.cam.get());
        BoardState {
            page: self.page.borrow().clone(),
            cam: self.cam.get(),
            cams,
            selection: self.selection.borrow().clone(),
            layers_open: self.layers_panel.reveals_child(),
            inspector_open: self.inspector_key.is_active(),
        }
    }

    /// Show `state` (once laid out): its page where it still is, its camera rather than a fit,
    /// what of its selection is still there, and its panels.
    pub(super) fn restore(&self, state: BoardState) {
        let page_lives = self.doc.scene.borrow().node(&state.page).is_some_and(|n| n.kind == NodeKind::Page);
        if !page_lives {
            return;
        }
        self.fitted.set(true);
        self.restored.set(true);
        self.set_page(&state.page);
        self.cams.borrow_mut().extend(state.cams);
        self.set_cam(state.cam);
        let ids: Vec<String> = {
            let scene = self.doc.scene.borrow();
            state.selection.into_iter().filter(|id| scene.page_of(id) == Some(state.page.as_str())).collect()
        };
        self.select(ids);
        if let Some(key) = super::tools::find_named(self.overlay.upcast_ref(), "design-layers-key").and_downcast::<gtk::ToggleButton>() {
            key.set_active(state.layers_open);
        }
        self.inspector_key.set_active(state.inspector_open);
    }

    /// Open the design in a window of its own, showing what this board shows, and leave the
    /// Design view for the gallery.
    fn pop_out(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.stop_play();
        self.end_edit(true);
        let state = self.state();
        super::window::pop_out(&ui, self.doc.clone(), move |b| b.restore(state));
        if let Some(back) = self.back.borrow().clone() {
            back();
        }
    }

    /// Show the page and camera this design was last left at, read from the settings (I-15),
    /// unless the person has moved since it opened.
    fn restore_view(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let (weak, id) = (self.me.clone(), self.doc.id);
        glib::spawn_future_local(async move {
            let Ok(got) = ui.call("settings.get", json!({"path": format!("{VIEWS}.{id}")})).await else {
                return;
            };
            let Some(b) = weak.upgrade() else {
                return;
            };
            let Some((page, cam)) = read_view(&got["value"]) else {
                return;
            };
            let opened = *b.page.borrow() == b.doc.scene.borrow().first_page().cloned().unwrap_or_default();
            if opened && !b.restored.get() && b.selection.borrow().is_empty() && b.doc.scene.borrow().node(&page).is_some_and(|n| n.kind == NodeKind::Page) {
                b.fitted.set(true);
                b.set_page(&page);
                b.set_cam(cam);
            }
        });
    }

    pub(super) fn me(&self) -> Rc<Board> {
        self.me.upgrade().expect("board alive while called")
    }

    // ------------------------------------------------------------------ geometry

    pub(super) fn to_page(&self, sx: f64, sy: f64) -> (f64, f64) {
        self.cam.get().to_page(sx, sy)
    }

    /// The page point `id`'s children count their `x`/`y` from (a group's own, not its
    /// children's bounds), as a box of no size.
    pub(super) fn origin_of(&self, id: &str) -> Rect {
        let (x, y) = self.placed().origin(id);
        Rect::new(x, y, 0.0, 0.0)
    }

    pub(super) fn placed(&self) -> Rc<Placed> {
        if let Some(p) = self.placed.borrow().as_ref() {
            return p.clone();
        }
        let scene = self.doc.scene.borrow();
        let started = trace().then(std::time::Instant::now);
        let placed = Rc::new(layout::place_scrolled(&scene, &self.page.borrow(), &*self.painter, &self.scroll.borrow()));
        if let Some(t) = started {
            println!("DESIGN_TRACE layout {:.3} ms {} nodes", t.elapsed().as_secs_f64() * 1e3, placed.rects.len());
        }
        *self.placed.borrow_mut() = Some(placed.clone());
        placed
    }

    /// The page as a still picture shows it, every scrolling frame at its start: what export
    /// and `design.render` draw. The canvas's own layout when nothing is scrolled.
    pub(super) fn still_placed(&self) -> Rc<Placed> {
        if self.scroll.borrow().values().all(|&(x, y)| x == 0.0 && y == 0.0) {
            return self.placed();
        }
        Rc::new(layout::place(&self.doc.scene.borrow(), &self.page.borrow(), &*self.painter))
    }

    /// Scroll the scrolling frame `id` by `(dx, dy)` page px, held within what its content
    /// overflows (as last placed). Whether it moved.
    pub(super) fn scroll_by(&self, id: &str, dx: f64, dy: f64) -> bool {
        let placed = self.placed();
        let Some((r, s)) = placed.rect(id).zip(placed.scrolls.get(id).copied()) else {
            return false;
        };
        let Some(scroll) = self.doc.scene.borrow().node(id).and_then(|n| n.scroll) else {
            return false;
        };
        let to = scroll_to(&r, &s, scroll, (dx, dy));
        if to == s.offset {
            return false;
        }
        if to == (0.0, 0.0) {
            self.scroll.borrow_mut().remove(id);
        } else {
            self.scroll.borrow_mut().insert(id.to_string(), to);
        }
        self.invalidate();
        true
    }

    /// The scene or the layout changed: lay out again and redraw, and refresh the chrome
    /// once this turn of the main loop is done.
    pub(super) fn invalidate(&self) {
        self.placed.borrow_mut().take();
        self.content.borrow_mut().take();
        // While Play runs, an edit (the inspector's, an agent's) shows in the running motion.
        self.refresh_play();
        self.canvas.queue_draw();
        self.schedule_chrome();
    }

    pub(super) fn schedule_chrome(&self) {
        if self.chrome_pending.replace(true) {
            return;
        }
        let weak = self.me.clone();
        glib::idle_add_local_once(move || {
            if let Some(b) = weak.upgrade() {
                b.chrome_pending.set(false);
                b.refresh_chrome();
            }
        });
    }

    /// The page's content, cached as one render node until something changes.
    fn content_node(&self, scene: &Scene, placed: &Placed) -> Option<gsk::RenderNode> {
        if let Some(node) = self.content.borrow().as_ref() {
            return Some(node.clone());
        }
        let started = trace().then(std::time::Instant::now);
        // While Play runs: each mover's pose at the frame's time, in a map kept from frame to frame.
        let posing = self.play.borrow_mut().as_mut().map(|p| {
            let posing = Rc::make_mut(&mut p.posing);
            let clocks = posing.clocks.clone();
            clocks.poses(scene, p.t_ms, &mut posing.poses);
            p.posing.clone()
        });
        let movers = posing.as_ref().map(|p| p.poses.len());
        let snap = gtk::Snapshot::new();
        // On the page's fill, which `draw_inner` lays under the grid: a top-level blend mixes
        // with it, as it does in a frame (B30).
        let page = self.page.borrow();
        let ground = scene.node(&page).and_then(fill_of).unwrap_or_else(|| CANVAS.into());
        self.painter.with_posing(posing, || self.painter.paint_page_on(&snap, scene, placed, &page, None, Some(&ground)));
        let node = snap.to_node();
        if let Some(t) = started {
            let ms = t.elapsed().as_secs_f64() * 1e3;
            match movers {
                Some(n) => println!("DESIGN_TRACE play_frame {ms:.3} ms movers {n}"),
                None => println!("DESIGN_TRACE content {ms:.3} ms"),
            }
        }
        self.content.borrow_mut().clone_from(&node);
        node
    }

    /// What the page paints, its words, strokes and shadows included: what fit frames (B3: a
    /// line an agent wrote without a box no longer pulls it to the origin).
    pub(super) fn page_bounds(&self) -> Option<Rect> {
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let page = self.page.borrow();
        let shown = scene.children(&page).iter().filter(|id| scene.node(id).is_some_and(|n| !n.is_hidden()));
        let painted: Vec<Rect> = shown.filter_map(|id| bounds::painted_bounds(&scene, &placed, &*self.painter, id)).collect();
        Rect::bounds(painted.iter())
    }

    /// The box the selection is drawn in on the page: each layer's as it shows (a line's
    /// points wherever its box is, a turned node's turned box, a child of a turned frame where
    /// the frame's turn carries it), as a flip turns them over. Its outline, handles and
    /// toolbar sit on it.
    pub(super) fn selection_bounds(&self) -> Option<Rect> {
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let shown: Vec<Rect> = self.selection.borrow().iter().filter_map(|id| shown_box(&scene, &placed, id)).collect();
        Rect::bounds(shown.iter())
    }

    /// What the selection paints, for Zoom to selection (B3).
    pub(super) fn selection_painted(&self) -> Option<Rect> {
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let painted: Vec<Rect> =
            self.selection.borrow().iter().filter_map(|id| bounds::painted_bounds_in_page(&scene, &placed, &*self.painter, id)).collect();
        Rect::bounds(painted.iter())
    }

    // ------------------------------------------------------------------ camera

    pub(super) fn set_cam(&self, cam: Camera) {
        self.cam.set(cam);
        self.zoom_label.set_text(&format!("{}%", (cam.zoom * 100.0).round()));
        self.style_edit();
        self.canvas.queue_draw();
        self.overlay.queue_allocate();
    }

    pub(super) fn zoom_to(&self, zoom: f64) {
        let (w, h) = self.size.get();
        let mut cam = self.cam.get();
        cam.zoom_at(w / 2.0, h / 2.0, zoom);
        self.set_cam(cam);
    }

    pub(super) fn step_zoom(&self, dir: i32) {
        self.zoom_to(step_zoom(self.cam.get().zoom, dir));
    }

    /// Fit `r` in the canvas, never past 100% unless `closer`.
    pub(super) fn fit(&self, r: Option<Rect>, closer: bool) {
        if self.size.get().0 <= 0.0 {
            self.fitted.set(false);
            return;
        }
        self.set_cam(self.fit_cam(r, closer));
    }

    fn fit_cam(&self, r: Option<Rect>, closer: bool) -> Camera {
        let (w, h) = self.size.get();
        // Clear of the layers panel on the left and the inspector on the right where they show
        // (E-board-21), 8 px from each.
        let layers = self.layers_panel.reveals_child().then(|| f64::from(self.layers_panel.width()) + 12.0 + 8.0);
        let inspector = self.inspector_scroll.is_visible().then(|| w - f64::from(self.inspector_scroll.width().max(INSPECTOR_W as i32)) - 12.0 - 8.0);
        let area = fit_area(w, h, layers, inspector);
        match r {
            Some(r) => {
                let mut cam = Camera::fit(&r, area.w, area.h, 56.0, if closer { 4.0 } else { 1.0 });
                cam.x -= area.x / cam.zoom;
                cam.y -= area.y / cam.zoom;
                cam
            }
            None => Camera { x: -w / 2.0, y: -h / 2.0, zoom: 1.0 },
        }
    }

    // ------------------------------------------------------------------ pages

    pub(super) fn set_page(&self, page: &str) {
        if *self.page.borrow() == page {
            return;
        }
        // A page deleted meanwhile (its row still in an open list) is not shown (TM-69).
        if !self.doc.scene.borrow().node(page).is_some_and(|n| n.kind == NodeKind::Page) {
            return;
        }
        // Play is of the page it was started on.
        self.stop_play();
        // A vector path being placed is finished on the page it was drawn on (TM-68).
        super::tools::finish_vector(self, false);
        self.end_edit(true);
        self.release_fields();
        let old = self.page.replace(page.to_string());
        self.cams.borrow_mut().insert(old, self.cam.get());
        self.selection.borrow_mut().clear();
        self.invalidate();
        match self.cams.borrow().get(page).copied() {
            Some(cam) => self.set_cam(cam),
            None => {
                let bounds = self.page_bounds();
                self.fit(bounds, false);
            }
        }
        self.selection_changed();
        // The pages list lights the page on show, and opens its Hidden fold while that is hidden.
        if let Some(l) = self.layers.borrow().as_ref() {
            l.pages_changed();
        }
    }

    pub(super) fn page_name(&self, id: &str) -> String {
        let scene = self.doc.scene.borrow();
        let index = scene.pages().iter().position(|p| p == id).unwrap_or(0) + 1;
        scene.node(id).and_then(|n| n.name.clone()).unwrap_or_else(|| format!("Page {index}"))
    }

    // ------------------------------------------------------------------ selection

    pub(super) fn select(&self, ids: Vec<String>) {
        if *self.selection.borrow() == ids {
            return;
        }
        // Another layer picked while one is typed into: those words are done (E-text-5).
        let editing = self.edit.borrow().as_ref().map(|e| e.id.clone());
        if editing.is_some_and(|e| !ids.contains(&e)) {
            self.end_edit(true);
        }
        self.release_fields();
        *self.selection.borrow_mut() = ids;
        self.selection_changed();
    }

    pub(super) fn selection_changed(&self) {
        self.canvas.queue_draw();
        self.rebuild_context();
        self.rebuild_inspector();
        if let Some(layers) = self.layers.borrow().as_ref() {
            layers.selection_changed();
        }
        self.overlay.queue_allocate();
    }

    /// The selection's ids that still exist, in paint order.
    pub(super) fn selected(&self) -> Vec<String> {
        let scene = self.doc.scene.borrow();
        edit::outermost(&scene, &self.selection.borrow())
    }

    // ------------------------------------------------------------------ edits

    /// Send `ops` as one undoable step; undoing it selects again what is selected now.
    pub(super) fn commit(&self, ops: Vec<NodeOp>) -> bool {
        let selection = self.selection.borrow().clone();
        self.doc.commit(ops, selection)
    }

    /// Set the same fields on every selected node.
    pub(super) fn set_selected(&self, set: Map<String, Value>) {
        let mut ids = self.selected();
        // A text being typed for the first time is not on the engine yet: style it in place,
        // and the style goes out with it.
        let fresh = self.edit.borrow().as_ref().filter(|e| e.created).map(|e| e.id.clone());
        if let Some(fresh) = fresh.filter(|f| ids.contains(f)) {
            let patched = self.doc.scene.borrow().node(&fresh).and_then(|n| n.patched(&set).ok());
            if let (Some(p), Some(n)) = (patched, self.doc.scene.borrow_mut().node_mut(&fresh)) {
                *n = p;
            }
            ids.retain(|id| *id != fresh);
            self.invalidate();
        }
        let ops: Vec<NodeOp> = ids.into_iter().map(|id| NodeOp::Update { id, set: set.clone() }).collect();
        self.commit(ops);
        if let Some(e) = self.edit.borrow().as_ref() {
            e.zoom.set(0.0);
        }
        self.style_edit();
    }

    fn changed(&self, change: &Change) {
        match change {
            Change::Scene => {
                self.keep_typed();
                // A picture deleted or replaced lets go of its decoded copy (B33).
                self.painter.retain(&self.doc.scene.borrow());
                // An undo can take away the page on show (one just added, or duplicated).
                self.keep_page();
                self.prune_selection();
                self.invalidate();
            }
            Change::Reloaded { changed, by } => {
                self.keep_typed();
                *self.painter.assets_dir.borrow_mut() = self.doc.assets_dir.borrow().clone();
                self.painter.retain(&self.doc.scene.borrow());
                self.prune_selection();
                self.invalidate();
                let removed = self.doc.take_removed();
                let actors = self.doc.take_actors();
                if by != "user" && (!changed.is_empty() || !removed.is_empty()) {
                    self.agent_changed(by, changed, &actors, removed.len());
                }
                self.keep_page();
            }
            Change::Design => self.design_changed(),
            // What an undo or redo touched is selected again, where it is still on this page
            // (and not under a drag or typing begun since).
            Change::Selected(ids) => {
                if !matches!(*self.gesture.borrow(), Gesture::Idle) || self.edit.borrow().is_some() {
                    return;
                }
                let ids: Vec<String> = {
                    let scene = self.doc.scene.borrow();
                    let page = self.page.borrow();
                    ids.iter().filter(|id| scene.page_of(id) == Some(page.as_str())).cloned().collect()
                };
                self.select(ids);
            }
            Change::Saving(on) => {
                // A refused edit is not saved until the next one is.
                let unsaved = !*on && self.doc.unsaved();
                let text = if *on { "Saving…" } else if unsaved { "Not saved" } else { "Saved" };
                self.set_status(text, unsaved.then_some("design-status-unsaved"));
                self.refresh_actions();
            }
            Change::Error(e) => self.toast(e),
        }
    }

    /// The words being typed stay on the canvas when the scene is put back to what the engine
    /// holds (a confirm, a reread), which has the words as they were before typing (TM-46).
    /// The edit still sends them when it ends.
    fn keep_typed(&self) {
        let typed = self.edit.borrow().as_ref().filter(|e| !e.created).map(|e| {
            let buffer = e.view.buffer();
            (e.id.clone(), buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string())
        });
        let Some((id, words)) = typed else {
            return;
        };
        if let Some(n) = self.doc.scene.borrow_mut().node_mut(&id) {
            if n.text.as_deref() != Some(words.as_str()) {
                let runs = relay_client::design::text::remap_runs(n.text.as_deref().unwrap_or(""), &words, n.runs.as_deref().unwrap_or_default());
                n.runs = (!runs.is_empty()).then_some(runs);
                n.text = Some(words);
            }
        }
    }

    /// An agent changed the design (the read after it names `changed` and `removed` layers):
    /// say so, glow what each agent changed, and offer to undo the newest's change (TM-12,
    /// TM-13). What it changed on another page is named with a key that goes there (E-ask-10).
    fn agent_changed(&self, by: &str, changed: &HashSet<String>, actors: &[(String, HashSet<String>)], removed: usize) {
        let glow: HashSet<String> = if actors.is_empty() { changed.clone() } else { actors.iter().flat_map(|(_, ids)| ids.iter().cloned()).collect() };
        let page = self.page.borrow().clone();
        let elsewhere = {
            let scene = self.doc.scene.borrow();
            let mut pages: Vec<(String, usize)> = Vec::new();
            for id in &glow {
                match scene.page_of(id) {
                    Some(p) if p != page => match pages.iter_mut().find(|(q, _)| q == p) {
                        Some((_, n)) => *n += 1,
                        None => pages.push((p.to_string(), 1)),
                    },
                    _ => {}
                }
            }
            pages.into_iter().max_by_key(|(_, n)| *n).map(|(p, _)| p)
        };
        let here = glow.iter().any(|id| self.doc.scene.borrow().page_of(id) == Some(page.as_str()));
        // "Just now" for a minute; then who it was.
        let words = match &elsewhere {
            Some(p) if !here => format!("{by} edited {}", self.page_name(p)),
            _ => format!("{by} edited it just now"),
        };
        let serial = self.set_status(&words, Some("design-status-agent"));
        let (weak, who) = (self.me.clone(), by.to_string());
        glib::timeout_add_local_once(std::time::Duration::from_secs(60), move || {
            if let Some(b) = weak.upgrade().filter(|b| b.status_serial.get() == serial) {
                b.set_status(&format!("Last edited by {who}"), None);
            }
        });
        match elsewhere {
            Some(p) => {
                self.goto.set_tooltip_text(Some(&format!("Show {} and what {by} changed there", self.page_name(&p))));
                *self.goto_page.borrow_mut() = Some((p, glow.clone()));
                self.goto.set_visible(true);
            }
            None => {
                self.goto_page.borrow_mut().take();
                self.goto.set_visible(false);
            }
        }
        self.start_flash(glow);
        if removed > 0 {
            self.toast(&format!("{by} removed {removed} {}", if removed == 1 { "layer" } else { "layers" }));
        }
        *self.agent_last.borrow_mut() = Some(by.to_string());
        self.agent_force.set(None);
        self.agent_undo.set_tooltip_text(Some(&format!("Undo {by}'s last change to this design")));
        self.agent_undo.set_visible(true);
    }

    /// Show the page an agent's change was on, and glow what it changed there.
    fn go_to_change(&self) {
        let Some((page, ids)) = self.goto_page.borrow_mut().take() else {
            return;
        };
        self.goto.set_visible(false);
        if self.doc.scene.borrow().node(&page).is_some() {
            self.set_page(&page);
            self.start_flash(ids);
        }
    }

    /// Show the first page when the one on show is gone, and bring the page list up to date.
    fn keep_page(&self) {
        let gone = self.doc.scene.borrow().node(&self.page.borrow()).is_none();
        if gone {
            let first = self.doc.scene.borrow().first_page().cloned().unwrap_or_default();
            self.set_page(&first);
        }
        if let Some(layers) = self.layers.borrow().as_ref() {
            layers.pages_changed();
        }
        // The dock's open page list lists the pages there are now (TM-69).
        if let Some(pop) = self.pages_popover.borrow().as_ref().filter(|p| p.is_visible()) {
            pop.set_child(Some(&super::layers::pages_menu(&self.me(), pop)));
        }
    }

    fn prune_selection(&self) {
        let scene = self.doc.scene.borrow();
        let page = self.page.borrow().clone();
        let keep: Vec<String> = self
            .selection
            .borrow()
            .iter()
            .filter(|id| scene.node(id).is_some() && scene.page_of(id) == Some(page.as_str()))
            .cloned()
            .collect();
        drop(scene);
        if keep != *self.selection.borrow() {
            *self.selection.borrow_mut() = keep;
            self.selection_changed();
        }
    }

    fn design_changed(&self) {
        let mut design = self.doc.design.borrow().clone();
        // A title stored before control characters were refused would stop GTK at a NUL (B43).
        design.title = relay_bus::design::strip_controls(&design.title, false).into_owned();
        self.title.set_text(&design.title);
        // The whole title, which the key cuts short past 48 characters.
        if let Some(key) = self.title.ancestor(gtk::MenuButton::static_type()) {
            key.set_tooltip_text(Some(&format!("{} — rename, duplicate, export or delete it", design.title)));
        }
        if let Some(window) = self.root.root().and_downcast::<gtk::Window>().filter(|_| self.windowed) {
            window.set_title(Some(&format!("{} — Relay Design", design.title)));
        }
        match &design.session {
            Some(s) => {
                self.terminal_label.set_text(s);
                self.terminal.remove_css_class("unlinked");
                self.terminal.set_tooltip_text(Some(&format!("This design belongs to {s}'s terminal")));
                self.refresh_lamp();
            }
            None => {
                self.terminal_lamp.set_visible(false);
                self.terminal_label.set_text("Project only");
                self.terminal.add_css_class("unlinked");
                self.terminal.set_tooltip_text(Some("Tie this design to a terminal so its agent can be asked about it"));
            }
        }
        let asking = match &design.session {
            Some(s) => format!("Ask {s}"),
            None => "Ask agent".into(),
        };
        if let Some(caption) = self.ask.child().and_then(|c| c.last_child()).and_downcast::<gtk::Label>() {
            caption.set_text(&asking);
        }
        if let Some(layers) = self.layers.borrow().as_ref() {
            layers.pages_changed();
        }
    }

    /// What the top bar says of saving and of who changed the design last; `class` marks an
    /// agent's change or a refused edit. Returns the word's serial, for a timer that ages it.
    fn set_status(&self, text: &str, class: Option<&str>) -> u64 {
        for c in ["design-status-agent", "design-status-unsaved"] {
            self.status.remove_css_class(c);
        }
        if let Some(c) = class {
            self.status.add_css_class(c);
        }
        self.status.set_text(text);
        let serial = self.status_serial.get() + 1;
        self.status_serial.set(serial);
        serial
    }

    /// Light the terminal key's lamp for `state` (running, idle, closed…).
    pub(super) fn set_lamp(&self, state: &str) {
        let old = self.lamp_state.replace(state.to_string());
        if !old.is_empty() {
            self.terminal_lamp.remove_css_class(&old);
        }
        self.terminal_lamp.add_css_class(state);
        self.terminal_lamp.set_visible(self.doc.session().is_some());
    }

    /// The design's terminal's state as its lamp last showed it (running, idle, closed…).
    pub(super) fn lamp_state(&self) -> String {
        self.lamp_state.borrow().clone()
    }

    /// Open or close the agent panel: the design's terminal on the canvas. Its key in the top
    /// bar follows, and the next design opened shows it as this one left it.
    pub(super) fn set_agent_panel(self: &Rc<Self>, on: bool) {
        AGENT_PANEL_ON.with(|c| c.set(on));
        if self.agent_key.is_active() != on {
            // Toggling comes back here.
            return self.agent_key.set_active(on);
        }
        let open = self.agent_panel.borrow().clone();
        match (on, open) {
            (true, None) => {
                let panel = super::agent_panel::AgentPanel::new(self);
                self.overlay.add_overlay(&panel.root);
                *self.agent_panel.borrow_mut() = Some(panel.clone());
                panel.open();
            }
            (true, Some(panel)) => panel.open(),
            (false, Some(panel)) => {
                panel.close();
                self.overlay.remove_overlay(&panel.root);
                self.agent_panel.borrow_mut().take();
                self.canvas.grab_focus();
            }
            (false, None) => {}
        }
    }

    /// Put the agent panel away as the board closes: its terminal back on the wall.
    pub(super) fn close_agent_panel(&self) {
        if let Some(panel) = self.agent_panel.borrow_mut().take() {
            panel.close();
            self.overlay.remove_overlay(&panel.root);
        }
    }

    /// Read the design's terminal's state for its lamp.
    fn refresh_lamp(&self) {
        let (Some(ui), Some(session)) = (self.ui.upgrade(), self.doc.session()) else {
            return;
        };
        let project = self.doc.design.borrow().project_id;
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let Ok(listed) = ui.call("session.list", json!({"project_id": project})).await else {
                return;
            };
            let Some(b) = weak.upgrade().filter(|b| b.doc.session().as_deref() == Some(session.as_str())) else {
                return;
            };
            let rows = crate::app::rows(&listed, "sessions");
            b.set_lamp(rows.iter().find(|s| crate::app::text(s, "name") == session).map_or("closed", |s| crate::app::text(s, "state")));
        });
    }

    pub fn toast(&self, text: &str) {
        self.toast_label.set_text(text);
        self.toast.set_reveal_child(true);
        let serial = self.toast_serial.get() + 1;
        self.toast_serial.set(serial);
        let weak = self.me.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(toast_ms(text)), move || {
            if let Some(b) = weak.upgrade().filter(|b| b.toast_serial.get() == serial) {
                b.toast.set_reveal_child(false);
            }
        });
    }

    fn start_flash(&self, ids: HashSet<String>) {
        let now = self.canvas.frame_clock().map(|c| c.frame_time()).unwrap_or_else(glib::monotonic_time);
        *self.flash.borrow_mut() = (ids, now);
        if self.flash_tick.borrow().is_some() {
            return;
        }
        let weak = self.me.clone();
        let id = self.canvas.add_tick_callback(move |_, clock| {
            let Some(b) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            b.canvas.queue_draw();
            if clock.frame_time() - b.flash.borrow().1 > FLASH_US {
                b.flash.borrow_mut().0.clear();
                b.flash_tick.borrow_mut().take();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        *self.flash_tick.borrow_mut() = Some(id);
    }

    // ------------------------------------------------------------------ drawing

    /// The canvas was allocated: the first time, fit the page in it; widgets that follow its
    /// size change after this layout pass, not during it.
    fn resized(&self, w: f64, h: f64) {
        self.size.set((w, h));
        if !self.fitted.get() && w > 0.0 {
            self.fitted.set(true);
            let bounds = self.page_bounds();
            self.cam.set(self.fit_cam(bounds, false));
        }
        let weak = self.me.clone();
        glib::idle_add_local_once(move || {
            if let Some(b) = weak.upgrade() {
                let cam = b.cam.get();
                b.zoom_label.set_text(&format!("{}%", (cam.zoom * 100.0).round()));
                // Too narrow for the tools and the inspector side by side, the inspector sits
                // under the tools instead of over the last of them (a 1024px window hid the pen's
                // options, icons and pictures).
                let (w, h) = b.size.get();
                let (_, dock) = b.dock.preferred_size();
                let beside = 12.0 + f64::from(dock.width()) + 12.0 + INSPECTOR_W + 12.0 <= w;
                let top = if beside { 12 } else { 12 + dock.height() + 10 };
                if b.inspector_scroll.margin_top() != top {
                    b.inspector_scroll.set_margin_top(top);
                }
                let max_inspector = (h - 64.0 - f64::from(top)).max(120.0) as i32;
                if b.inspector_scroll.max_content_height() != max_inspector {
                    b.inspector_scroll.set_max_content_height(max_inspector);
                }
            }
        });
    }

    fn draw(&self, snap: &gtk::Snapshot, w: f64, h: f64) {
        let started = trace().then(std::time::Instant::now);
        self.draw_inner(snap, w, h);
        if let Some(t) = started {
            println!("DESIGN_TRACE draw {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
        }
    }

    fn draw_inner(&self, snap: &gtk::Snapshot, w: f64, h: f64) {
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let page = self.page.borrow().clone();
        let cam = self.cam.get();
        let canvas_fill = scene.node(&page).and_then(fill_of).unwrap_or_else(|| CANVAS.into());
        snap.append_color(&rgba(&canvas_fill), &graphene::Rect::new(0.0, 0.0, w as f32, h as f32));
        self.grid(snap, w, h, &canvas_fill);
        if let Some(node) = self.content_node(&scene, &placed) {
            snap.save();
            snap.scale(cam.zoom as f32, cam.zoom as f32);
            snap.translate(&graphene::Point::new(-cam.x as f32, -cam.y as f32));
            snap.append_node(&node);
            snap.restore();
        }
        self.frame_labels(snap, &scene, &placed, &page);
        if scene.children(&page).is_empty() && matches!(*self.gesture.borrow(), Gesture::Idle) && self.vector.borrow().is_none() {
            self.empty_hint(snap, w, h, &canvas_fill);
        }
        self.overlays(snap, &scene, &placed);
    }

    /// What an empty page says, in its middle (I-16).
    fn empty_hint(&self, snap: &gtk::Snapshot, w: f64, h: f64, ground: &str) {
        let layout = self.label_layout("empty", "Press F, R or T to start, or Ask an agent", 14.0, pango::Weight::Normal, false);
        let (lw, lh) = layout.pixel_size();
        let mut ink = label_ink(paint::luminance(ground));
        ink.set_alpha(0.8);
        snap.save();
        snap.translate(&graphene::Point::new(((w - lw as f64) / 2.0) as f32, ((h - lh as f64) / 2.0) as f32));
        snap.append_layout(&layout, &ink);
        snap.restore();
    }

    /// A dot every 8, 16, 32… page pixels, whichever falls 16 to 32 screen pixels apart.
    fn grid(&self, snap: &gtk::Snapshot, w: f64, h: f64, ground: &str) {
        let cam = self.cam.get();
        let mut step = 8.0;
        while step * cam.zoom < 16.0 {
            step *= 2.0;
        }
        let s = step * cam.zoom;
        if s > 400.0 {
            return;
        }
        let (ox, oy) = cam.to_screen(0.0, 0.0);
        let (ox, oy) = (ox.rem_euclid(s), oy.rem_euclid(s));
        let dot = if paint::luminance(ground) > 0.35 { gdk::RGBA::new(0.0, 0.0, 0.0, 0.14) } else { gdk::RGBA::new(1.0, 1.0, 1.0, 0.11) };
        let bounds = graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
        // The tile starts half a dot before a grid point, so its dot is whole inside it.
        let tile = graphene::Rect::new((ox - s - 0.75) as f32, (oy - s - 0.75) as f32, s as f32, s as f32);
        snap.push_repeat(&bounds, Some(&tile));
        snap.append_color(&dot, &graphene::Rect::new(tile.x(), tile.y(), 1.5, 1.5));
        snap.pop();
    }

    fn screen(&self, r: &Rect) -> Rect {
        self.cam.get().rect_to_screen(r)
    }

    fn outline(&self, snap: &gtk::Snapshot, r: &Rect, color: &str, width: f32) {
        let c = rgba(color);
        let rr = gsk::RoundedRect::from_rect(grect(r), 0.0);
        snap.append_border(&rr, &[width; 4], &[c, c, c, c]);
    }

    /// One layer's outline where it is drawn: its turned box when it or a frame it is in is
    /// turned, else the box it shows (a line's points, wherever its box is).
    fn outline_node(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, id: &str, width: f32) {
        let turned = scene.node(id).filter(|n| n.kind != NodeKind::Line).and_then(|_| shapes::visual(scene, placed, id)).filter(|(_, t)| *t != 0.0);
        match turned {
            Some((r, turn)) => self.quad(snap, &self.screen(&r), turn, width),
            None => {
                if let Some(r) = shown_box(scene, placed, id) {
                    self.outline(snap, &self.screen(&r), SELECT, width);
                }
            }
        }
    }

    /// Screen box `s` turned `turn` degrees about its centre, outlined.
    fn quad(&self, snap: &gtk::Snapshot, s: &Rect, turn: f64, width: f32) {
        let at = |x: f64, y: f64| shapes::rotate(x, y, s.cx(), s.cy(), turn);
        let corners = [at(s.x, s.y), at(s.right(), s.y), at(s.right(), s.bottom()), at(s.x, s.bottom())];
        let pb = gsk::PathBuilder::new();
        pb.move_to(corners[0].0 as f32, corners[0].1 as f32);
        for c in &corners[1..] {
            pb.line_to(c.0 as f32, c.1 as f32);
        }
        pb.close();
        snap.append_stroke(&pb.to_path(), &gsk::Stroke::new(width), &rgba(SELECT));
    }

    fn label_layout(&self, key: &str, text: &str, size: f64, weight: pango::Weight, mono: bool) -> pango::Layout {
        let mut labels = self.labels.borrow_mut();
        if let Some(l) = labels.get(key) {
            if l.text() == text {
                return l.clone();
            }
        }
        let l = self.canvas.create_pango_layout(Some(text));
        let mut d = pango::FontDescription::new();
        d.set_family(if mono { "Geist Mono" } else { "Geist" });
        d.set_absolute_size(size * pango::SCALE as f64);
        d.set_weight(weight);
        l.set_font_description(Some(&d));
        if labels.len() > 400 {
            labels.clear();
        }
        labels.insert(key.to_string(), l.clone());
        l
    }

    /// The names above the page's frames, as artboards carry them.
    fn frame_labels(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, page: &str) {
        let selection = self.selection.borrow();
        let hover = self.hover.borrow();
        // A grey that reads on the page's own fill, 4.5:1 or better on either (E-board-1).
        let ground = scene.node(page).and_then(fill_of).unwrap_or_else(|| CANVAS.into());
        let ink = label_ink(paint::luminance(&ground));
        for id in scene.children(page) {
            let Some(n) = scene.node(id).filter(|n| n.kind == NodeKind::Frame && !n.is_hidden()) else {
                continue;
            };
            let Some(r) = placed.rect(id) else {
                continue;
            };
            let s = self.screen(&r);
            if s.w < 28.0 || s.bottom() < 0.0 || s.y > self.size.get().1 + 20.0 {
                continue;
            }
            let name = frame_label(n);
            let layout = self.label_layout(&format!("frame:{id}"), &name, 12.0, pango::Weight::Medium, false);
            layout.set_width((s.w * pango::SCALE as f64) as i32);
            layout.set_ellipsize(pango::EllipsizeMode::End);
            let (_, lh) = layout.pixel_size();
            let lit = selection.contains(id) || hover.as_deref() == Some(id.as_str());
            let color = if lit { rgba(SELECT) } else { ink };
            snap.save();
            snap.translate(&graphene::Point::new(s.x as f32, (s.y - lh as f64 - 6.0) as f32));
            snap.append_layout(&layout, &color);
            snap.restore();
        }
    }

    /// The label above a top-level frame, as a hit box in screen pixels.
    pub(super) fn frame_label_at(&self, sx: f64, sy: f64) -> Option<String> {
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let page = self.page.borrow();
        for id in scene.children(&page).iter().rev() {
            let Some(n) = scene.node(id).filter(|n| n.kind == NodeKind::Frame && !n.is_hidden()) else {
                continue;
            };
            let Some(r) = placed.rect(id) else {
                continue;
            };
            let s = self.screen(&r);
            if s.w < 28.0 {
                continue;
            }
            let name = frame_label(n);
            let layout = self.label_layout(&format!("frame:{id}"), &name, 12.0, pango::Weight::Medium, false);
            let (lw, lh) = layout.pixel_size();
            let hit = Rect::new(s.x, s.y - lh as f64 - 8.0, (lw as f64).min(s.w) + 4.0, lh as f64 + 6.0);
            if hit.contains(sx, sy) {
                return Some(id.clone());
            }
        }
        None
    }

    fn overlays(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed) {
        let editing = self.edit.borrow().as_ref().map(|e| e.id.clone());
        // What an agent just changed.
        let (flash, since) = self.flash.borrow().clone();
        if !flash.is_empty() {
            let now = self.canvas.frame_clock().map(|c| c.frame_time()).unwrap_or_else(glib::monotonic_time);
            let t = ((now - since) as f64 / FLASH_US as f64).clamp(0.0, 1.0);
            let alpha = (1.0 - t).powf(1.6);
            let mut c = rgba(AGENT);
            c.set_alpha(alpha as f32);
            for id in &flash {
                if let Some(r) = shown_box(scene, placed, id) {
                    let s = self.screen(&r).inflate(2.0);
                    let rr = gsk::RoundedRect::from_rect(grect(&s), 3.0);
                    snap.append_border(&rr, &[2.0; 4], &[c, c, c, c]);
                }
            }
        }
        // While Play runs the page shows as it plays: no selection, hover, guides or handles.
        if self.playing() {
            return;
        }
        // Hover.
        if let Some(id) = self.hover.borrow().as_ref() {
            if !self.selection.borrow().contains(id) && matches!(*self.gesture.borrow(), Gesture::Idle) {
                self.outline_node(snap, scene, placed, id, 1.5);
            }
        }
        self.scrollbars(snap, scene, placed);
        let gesture = self.gesture.borrow();
        // Where a dragged node would land, and its place there in a layout.
        if let Gesture::Move { target: Some(t), into, moved: true, ids, .. } = &*gesture {
            self.outline_node(snap, scene, placed, t, 2.0);
            if let Some(index) = into {
                self.insertion(snap, scene, placed, t, *index as usize, ids);
            }
        }
        // Laid-out layers dragged: what they paint, under the pointer (E-board-10).
        if let Gesture::Move { ghost: Some(ghost), moved: true, start, at, .. } = &*gesture {
            let cam = self.cam.get();
            snap.save();
            snap.scale(cam.zoom as f32, cam.zoom as f32);
            snap.translate(&graphene::Point::new((at.0 - start.0 - cam.x) as f32, (at.1 - start.1 - cam.y) as f32));
            snap.push_opacity(0.6);
            snap.append_node(ghost);
            snap.pop();
            snap.restore();
        }
        if let Gesture::Move { reorder: Some((parent, index)), moved: true, ids, .. } = &*gesture {
            self.insertion(snap, scene, placed, parent, *index as usize, ids);
        }
        // The selection: each node, then the box with its handles and size.
        let selection = self.selection.borrow();
        let resizing = matches!(*gesture, Gesture::Resize { .. } | Gesture::Endpoint { .. });
        if !selection.is_empty() && editing.is_none() {
            for id in selection.iter() {
                self.outline_node(snap, scene, placed, id, 1.0);
            }
            // A locked layer shows it is selected, but has no handles to drag (B16), nor does a
            // group, whose box is its children's (B90).
            let locked = !handled(scene, &selection);
            if let Some((_, r, turn)) = self.single_turn() {
                // One turned node: its outline (drawn above) and handles turn with it.
                let s = self.screen(&r);
                let at = |x: f64, y: f64| shapes::rotate(x, y, s.cx(), s.cy(), turn);
                if !locked && !matches!(*gesture, Gesture::Move { moved: true, .. }) {
                    for h in HANDLES {
                        let (x, y) = handle_point(&s, h);
                        let (x, y) = at(x, y);
                        self.handle(snap, x, y, false);
                    }
                }
                let shown = shapes::rotated_bounds(&s, turn);
                self.size_pill(snap, &shown, &r);
            } else if let Some(b) = Rect::bounds(selection.iter().filter_map(|id| shown_box(scene, placed, id)).collect::<Vec<_>>().iter()) {
                let s = self.screen(&b);
                if selection.len() > 1 {
                    self.outline(snap, &s, SELECT, 1.0);
                }
                let line = selection.len() == 1 && scene.node(&selection[0]).is_some_and(ends_dragged);
                if line && !locked {
                    for p in self.endpoints(&selection[0]) {
                        self.handle(snap, p.0, p.1, true);
                    }
                } else if !line && !locked && !matches!(*gesture, Gesture::Move { moved: true, .. }) && s.w > 2.0 && s.h > 2.0 {
                    for h in HANDLES {
                        let (x, y) = handle_point(&s, h);
                        if (h.left || h.right) && (h.top || h.bottom) || s.w.min(s.h) > 24.0 {
                            self.handle(snap, x, y, false);
                        }
                    }
                }
                if resizing || matches!(*gesture, Gesture::Idle) || matches!(*gesture, Gesture::Move { .. }) {
                    self.size_pill(snap, &s, &b);
                }
            }
            if !matches!(*gesture, Gesture::Move { moved: true, .. }) {
                if let Some((_, _, (x, y))) = self.gradient_centre() {
                    self.centre_handle(snap, x, y);
                }
            }
        }
        if editing.is_none() && matches!(*gesture, Gesture::Idle | Gesture::Resize { .. }) {
            self.pin_marks(snap, scene, placed);
        }
        // Guides; inside a turned frame, turned with it.
        let guides_in = self.guides_in.borrow().clone();
        for g in self.guides.borrow().iter() {
            let c = rgba(GUIDE);
            let cam = self.cam.get();
            if let Some(f) = &guides_in {
                let ends = if g.vertical { [(g.at, g.from), (g.at, g.to)] } else { [(g.from, g.at), (g.to, g.at)] };
                let [a, z] = ends.map(|(x, y)| {
                    let (x, y) = shapes::to_page_pt(scene, placed, f, x, y);
                    cam.to_screen(x, y)
                });
                let pb = gsk::PathBuilder::new();
                pb.move_to(a.0 as f32, a.1 as f32);
                pb.line_to(z.0 as f32, z.1 as f32);
                snap.append_stroke(&pb.to_path(), &gsk::Stroke::new(1.0), &c);
            } else if g.vertical {
                let (x, y0) = cam.to_screen(g.at, g.from);
                let (_, y1) = cam.to_screen(g.at, g.to);
                snap.append_color(&c, &graphene::Rect::new(x.round() as f32 - 0.5, y0 as f32, 1.0, (y1 - y0) as f32));
            } else {
                let (x0, y) = cam.to_screen(g.from, g.at);
                let (x1, _) = cam.to_screen(g.to, g.at);
                snap.append_color(&c, &graphene::Rect::new(x0 as f32, y.round() as f32 - 0.5, (x1 - x0) as f32, 1.0));
            }
        }
        match &*gesture {
            Gesture::Marquee { a, b, moved: true, .. } => {
                let r = self.screen(&Rect::spanning(*a, *b));
                let mut fill = rgba(SELECT);
                fill.set_alpha(0.08);
                snap.append_color(&fill, &grect(&r));
                self.outline(snap, &r, SELECT, 1.0);
            }
            Gesture::Create { tool, a, b, from, to, .. } => {
                let cam = self.cam.get();
                // The shapes a connector attaches to.
                for end in [from, to].into_iter().flatten() {
                    if let Some(r) = placed.rect(end) {
                        self.outline(snap, &self.screen(&r).inflate(3.0), SELECT, 2.0);
                    }
                }
                match tool {
                    Tool::Line | Tool::Arrow => {
                        let (x0, y0) = cam.to_screen(a.0, a.1);
                        let (x1, y1) = cam.to_screen(b.0, b.1);
                        let pb = gsk::PathBuilder::new();
                        pb.move_to(x0 as f32, y0 as f32);
                        pb.line_to(x1 as f32, y1 as f32);
                        snap.append_stroke(&pb.to_path(), &gsk::Stroke::new(1.5), &rgba(SELECT));
                    }
                    _ => {
                        let r = self.screen(&Rect::spanning(*a, *b));
                        if let Some((sides, star)) = polygon_of(*tool) {
                            let pts = shapes::polygon(&r, sides, star);
                            let pb = gsk::PathBuilder::new();
                            pb.move_to(pts[0].0 as f32, pts[0].1 as f32);
                            for q in &pts[1..] {
                                pb.line_to(q.0 as f32, q.1 as f32);
                            }
                            pb.close();
                            snap.append_stroke(&pb.to_path(), &gsk::Stroke::new(1.0), &rgba(SELECT));
                        } else if *tool == Tool::Ellipse {
                            let half = graphene::Size::new(r.w as f32 / 2.0, r.h as f32 / 2.0);
                            let rr = gsk::RoundedRect::new(grect(&r), half, half, half, half);
                            let c = rgba(SELECT);
                            snap.append_border(&rr, &[1.0; 4], &[c, c, c, c]);
                        } else {
                            self.outline(snap, &r, SELECT, 1.0);
                        }
                        self.size_pill(snap, &r, &Rect::spanning(*a, *b));
                    }
                }
            }
            _ if self.vector.borrow().is_some() => self.draw_vector(snap),
            Gesture::Draw { points, tool, parent } if points.len() > 1 => {
                let cam = self.cam.get();
                let pb = gsk::PathBuilder::new();
                let (x0, y0) = cam.to_screen(points[0][0], points[0][1]);
                pb.move_to(x0 as f32, y0 as f32);
                for p in &points[1..] {
                    let (x, y) = cam.to_screen(p[0], p[1]);
                    pb.line_to(x as f32, y as f32);
                }
                let highlighter = *tool == Tool::Highlighter;
                let width = if highlighter { 16.0 } else { 3.0 } * cam.zoom;
                let stroke = gsk::Stroke::new(width as f32);
                stroke.set_line_cap(gsk::LineCap::Round);
                stroke.set_line_join(gsk::LineJoin::Round);
                snap.append_stroke(&pb.to_path(), &stroke, &rgba(&self.pen_color(*tool, parent)));
            }
            _ => {}
        }
    }

    /// How the one selected layer is pinned in its frame: a dashed line from each pinned edge to
    /// the frame's, and a tick on the frame's middle for a centred axis (feature-layout §3.5).
    fn pin_marks(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed) {
        let selection = self.selection.borrow();
        let [id] = selection.as_slice() else {
            return;
        };
        let Some(n) = scene.node(id).filter(|n| n.is_pinned() && !layout::is_laid(scene, id)) else {
            return;
        };
        let Some(parent) = scene.parent(id).filter(|p| scene.node(p).is_some_and(|p| p.kind == NodeKind::Frame)) else {
            return;
        };
        let (Some(r), Some(pr)) = (placed.rect(id), placed.rect(parent)) else {
            return;
        };
        let cam = self.cam.get();
        let pb = gsk::PathBuilder::new();
        for (a, z) in pin_lines(n, &r, &pr, 1.0 / cam.zoom) {
            let [a, z] = [a, z].map(|(x, y)| {
                let (x, y) = shapes::to_page_pt(scene, placed, parent, x, y);
                cam.to_screen(x, y)
            });
            pb.move_to(a.0 as f32, a.1 as f32);
            pb.line_to(z.0 as f32, z.1 as f32);
        }
        let stroke = gsk::Stroke::new(1.0);
        stroke.set_dash(&[3.0, 2.0]);
        snap.append_stroke(&pb.to_path(), &stroke, &rgba(SELECT));
    }

    /// The overlay scrollbars of the scrolling frames under the pointer, selected or holding the
    /// selection (feature-layout §2.5): drawn over the canvas, never part of the content, so no
    /// export carries them. A turned frame's turn with it.
    fn scrollbars(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed) {
        if placed.scrolls.is_empty() {
            return;
        }
        let hover = self.hover.borrow().clone();
        let selection = self.selection.borrow();
        let px = 1.0 / self.cam.get().zoom;
        // A mid grey reads on light and dark content alike.
        let mut c = rgba("#7A7A7A");
        c.set_alpha(0.7);
        for (id, s) in &placed.scrolls {
            let Some((n, r)) = scene.node(id).filter(|n| !n.is_hidden()).zip(placed.rect(id)) else {
                continue;
            };
            let near = |other: &str| other == id || scene.is_inside(other, id);
            if !hover.as_deref().is_some_and(near) && !selection.iter().any(|sel| near(sel)) {
                continue;
            }
            let turned = !shapes::turned_ancestors(scene, placed, id).is_empty() || shapes::rotation(n) != 0.0;
            for bar in scroll_bars(&r, s, n.scroll.unwrap_or(Scroll::Both), px) {
                if turned {
                    let pb = gsk::PathBuilder::new();
                    for (i, (x, y)) in bar.corners().into_iter().enumerate() {
                        let (x, y) = shapes::to_page_pt(scene, placed, id, x, y);
                        let (x, y) = self.cam.get().to_screen(x, y);
                        if i == 0 { pb.move_to(x as f32, y as f32) } else { pb.line_to(x as f32, y as f32) }
                    }
                    pb.close();
                    snap.append_fill(&pb.to_path(), gsk::FillRule::Winding, &c);
                } else {
                    let rr = gsk::RoundedRect::from_rect(grect(&self.screen(&bar)), 2.0);
                    snap.push_rounded_clip(&rr);
                    snap.append_color(&c, &grect(&self.screen(&bar)));
                    snap.pop();
                }
            }
        }
    }

    /// The vector pen's path so far, the segment to the pointer, and its points; the first
    /// grows under the pointer when a click there would close the path.
    fn draw_vector(&self, snap: &gtk::Snapshot) {
        let vector = self.vector.borrow();
        let Some((_, points)) = vector.as_ref() else {
            return;
        };
        let cam = self.cam.get();
        let screen: Vec<(f64, f64)> = points.iter().map(|p| cam.to_screen(p[0], p[1])).collect();
        let Some(&first) = screen.first() else {
            return;
        };
        let pointer = self.pointer.get();
        let closing = screen.len() > 2 && (pointer.0 - first.0).abs() <= 8.0 && (pointer.1 - first.1).abs() <= 8.0;
        let pb = gsk::PathBuilder::new();
        pb.move_to(first.0 as f32, first.1 as f32);
        for q in &screen[1..] {
            pb.line_to(q.0 as f32, q.1 as f32);
        }
        let stroke = gsk::Stroke::new(1.5);
        snap.append_stroke(&pb.to_path(), &stroke, &rgba(SELECT));
        let rubber = gsk::PathBuilder::new();
        let last = screen[screen.len() - 1];
        rubber.move_to(last.0 as f32, last.1 as f32);
        // With Shift, at 45° steps from the last point, where the click would put it (E-board-2).
        let to = if closing {
            first
        } else if self.shift_held.get() {
            super::tools::constrain45(last, pointer)
        } else {
            pointer
        };
        rubber.line_to(to.0 as f32, to.1 as f32);
        let dashed = gsk::Stroke::new(1.0);
        dashed.set_dash(&[4.0, 3.0]);
        snap.append_stroke(&rubber.to_path(), &dashed, &rgba(SELECT));
        for (i, q) in screen.iter().enumerate() {
            let size = if i == 0 && closing { 10.0 } else { 7.0 };
            let r = Rect::new(q.0 - size / 2.0, q.1 - size / 2.0, size, size);
            snap.append_color(&gdk::RGBA::WHITE, &grect(&r));
            self.outline(snap, &r, SELECT, 1.5);
        }
    }

    /// Where a reorder or a drop would put the dragged children: a bar before the child at
    /// `index` among the others (hidden ones counted, as the engine counts them: B20), or after
    /// the last shown; in a wrap, beside that child at its height.
    fn insertion(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, parent: &str, index: usize, moving: &[String]) {
        let Some(p) = scene.node(parent) else {
            return;
        };
        let Some(pr) = placed.rect(parent) else {
            return;
        };
        let rest: Vec<&String> = scene.children(parent).iter().filter(|c| !moving.contains(c)).collect();
        // What the layout places: neither hidden (no box) nor absolute.
        let laid = |c: &&String| scene.node(c).is_some_and(|n| !n.is_absolute()).then(|| placed.rect(c)).flatten();
        let next = rest.iter().skip(index).find_map(laid);
        let last = rest.iter().rev().find_map(laid);
        let bar = insertion_bar(p.layout_mode(), &pr, next, last, 1.0 / self.cam.get().zoom);
        let s = self.screen(&bar);
        snap.append_color(&rgba(SELECT), &grect(&Rect::new(s.x, s.y, s.w.max(2.0), s.h.max(2.0))));
    }

    fn handle(&self, snap: &gtk::Snapshot, x: f64, y: f64, round: bool) {
        let r = Rect::new(x - 4.0, y - 4.0, 8.0, 8.0);
        let rr = gsk::RoundedRect::from_rect(grect(&r), if round { 4.0 } else { 1.5 });
        snap.push_rounded_clip(&rr);
        snap.append_color(&gdk::RGBA::WHITE, &grect(&r));
        snap.pop();
        let c = rgba(SELECT);
        snap.append_border(&rr, &[1.2; 4], &[c, c, c, c]);
    }

    /// A gradient's centre: a ring round a dot, apart from the square resize handles.
    fn centre_handle(&self, snap: &gtk::Snapshot, x: f64, y: f64) {
        let c = rgba(SELECT);
        for (radius, colour, width) in [(7.0, gdk::RGBA::WHITE, 3.0), (7.0, c, 1.5)] {
            let r = Rect::new(x - radius, y - radius, radius * 2.0, radius * 2.0);
            let rr = gsk::RoundedRect::from_rect(grect(&r), radius as f32);
            snap.append_border(&rr, &[width; 4], &[colour; 4]);
        }
        let dot = Rect::new(x - 2.0, y - 2.0, 4.0, 4.0);
        snap.push_rounded_clip(&gsk::RoundedRect::from_rect(grect(&dot), 2.0));
        snap.append_color(&c, &grect(&dot));
        snap.pop();
    }

    /// "W × H" under the selection.
    fn size_pill(&self, snap: &gtk::Snapshot, s: &Rect, page: &Rect) {
        let text = format!("{} × {}", fmt(page.w), fmt(page.h));
        let layout = self.label_layout("size", &text, 11.0, pango::Weight::Medium, true);
        let (lw, lh) = layout.pixel_size();
        let pill = Rect::new(s.cx() - lw as f64 / 2.0 - 6.0, s.bottom() + 8.0, lw as f64 + 12.0, lh as f64 + 4.0);
        let rr = gsk::RoundedRect::from_rect(grect(&pill), 4.0);
        snap.push_rounded_clip(&rr);
        snap.append_color(&rgba(SELECT), &grect(&pill));
        snap.pop();
        snap.save();
        snap.translate(&graphene::Point::new((pill.x + 6.0) as f32, (pill.y + 2.0) as f32));
        snap.append_layout(&layout, &gdk::RGBA::WHITE);
        snap.restore();
    }

    /// Whether the selection has resize handles: something in it is neither locked (B16) nor a
    /// group (B90).
    pub(super) fn has_handles(&self) -> bool {
        handled(&self.doc.scene.borrow(), &self.selection.borrow())
    }

    /// The one selected node when it is drawn turned, by its own turn or a frame's it is in:
    /// its id, its unturned box moved to where it is drawn on the page, and its whole turn (B12).
    pub(super) fn single_turn(&self) -> Option<(String, Rect, f64)> {
        let sel = self.selection.borrow();
        let [id] = sel.as_slice() else {
            return None;
        };
        let scene = self.doc.scene.borrow();
        scene.node(id).filter(|n| n.kind != NodeKind::Line)?;
        let (r, turn) = shapes::visual(&scene, &self.placed(), id)?;
        (turn != 0.0).then(|| (id.clone(), r, turn))
    }

    /// The one selected layer's radial or conic gradient (its fill's, else its stroke's), for
    /// the handle that drags its centre: its id, the field, and the centre on the screen. None
    /// for a locked layer, a linear gradient, or a box too small on the screen to grab apart
    /// from moving the layer.
    pub(super) fn gradient_centre(&self) -> Option<(String, &'static str, (f64, f64))> {
        let sel = self.selection.borrow();
        let [id] = sel.as_slice() else {
            return None;
        };
        let scene = self.doc.scene.borrow();
        if !handled(&scene, &sel) {
            return None;
        }
        let n = scene.node(id)?;
        let (field, g) = [("gradient", &n.gradient), ("stroke_gradient", &n.stroke_gradient)]
            .into_iter()
            .find_map(|(f, g)| g.as_ref().filter(|g| g.kind != relay_bus::design::GradientKind::Linear).map(|g| (f, g)))?;
        let (r, turn) = shapes::visual(&scene, &self.placed(), id)?;
        let zoom = self.cam.get().zoom;
        if r.w * zoom < CENTRE_MIN_BOX || r.h * zoom < CENTRE_MIN_BOX {
            return None;
        }
        let (fx, fy) = g.center_or_middle();
        let (x, y) = shapes::rotate(r.x + r.w * fx, r.y + r.h * fy, r.cx(), r.cy(), turn);
        Some((id.clone(), field, self.cam.get().to_screen(x, y)))
    }

    /// A line's ends, on screen.
    pub(super) fn endpoints(&self, id: &str) -> Vec<(f64, f64)> {
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let (Some(n), Some(r)) = (scene.node(id), placed.rect(id)) else {
            return Vec::new();
        };
        let cam = self.cam.get();
        // A connector's ends are where it is drawn: on the shapes it joins.
        if n.from.is_some() || n.to.is_some() {
            let (pts, _) = shapes::line_points(&scene, &placed, n);
            return [pts.first(), pts.last()].into_iter().flatten().map(|p| cam.to_screen(p.0, p.1)).collect();
        }
        n.points
            .as_deref()
            .map(|pts| {
                [pts.first(), pts.last()]
                    .into_iter()
                    .flatten()
                    .map(|p| cam.to_screen(r.x + p[0], r.y + p[1]))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The colour `tool` (the pen, the highlighter or the vector pen) draws with inside
    /// `container`: the one picked for that tool, else the highlighter's yellow, else the ink
    /// that reads on the ground it draws on (TM-42, E-board-18).
    pub(super) fn pen_color(&self, tool: Tool, container: &str) -> String {
        if let Some(chosen) = self.pens.borrow().get(&tool).filter(|c| !c.is_empty()) {
            return chosen.clone();
        }
        if tool == Tool::Highlighter {
            return "#FFD60A66".into();
        }
        self.ink_for(container)
    }

    /// The colour a mark reads well on inside `container`: dark on a light ground, light on a
    /// dark one, the ground being the container's fill, or its nearest ancestor's, or the canvas.
    pub(super) fn ink_for(&self, container: &str) -> String {
        let scene = self.doc.scene.borrow();
        let ground = std::iter::once(container)
            .chain(scene.ancestors(container))
            .filter_map(|id| scene.node(id))
            .find_map(fill_of)
            .unwrap_or_else(|| CANVAS.into());
        if paint::luminance(&ground) > 0.35 { "#1A1A1A".into() } else { "#EDE9E2".into() }
    }

    // ------------------------------------------------------------------ play

    /// Whether the page's motion is playing or paused in Play (feature-motion §3).
    pub(super) fn playing(&self) -> bool {
        self.play.borrow().is_some()
    }

    /// The page's clocks for Play, with the shown pictures that animate themselves touched (they
    /// repaint every frame), and whether there are any.
    fn play_clocks(&self) -> (Clocks, bool) {
        let scene = self.doc.scene.borrow();
        let page = self.page.borrow().clone();
        let mut clocks = Clocks::new(&scene, &page);
        let moving = self.painter.moving_srcs();
        let mut pictures = false;
        if !moving.is_empty() {
            let mut stack = vec![page];
            while let Some(id) = stack.pop() {
                let Some(n) = scene.node(&id).filter(|n| !n.is_hidden()) else {
                    continue;
                };
                if n.kind == NodeKind::Image && n.src.as_deref().is_some_and(|s| moving.contains(s)) {
                    clocks.touch(&scene, &id);
                    pictures = true;
                }
                stack.extend(scene.children(&id).iter().cloned());
            }
        }
        (clocks, pictures)
    }

    /// The Play key shows while something on the page moves, or Play runs.
    fn refresh_play_key(&self) {
        let shows = self.playing() || {
            let (clocks, pictures) = self.play_clocks();
            !clocks.is_empty() || pictures
        };
        if self.play_key.is_visible() != shows {
            self.play_key.set_visible(shows);
        }
    }

    /// Ctrl+P and the Play key: start Play, or end it.
    pub(super) fn toggle_play(&self) {
        if self.playing() {
            self.stop_play();
        } else {
            self.start_play();
        }
    }

    /// Play the page's motion from its start: every clock at 0 together, on the canvas's frame
    /// clock. Typing and any drag end first; Play edits nothing.
    pub(super) fn start_play(&self) {
        if self.playing() {
            return;
        }
        super::tools::finish_vector(self, false);
        self.end_edit(true);
        self.cancel_gesture();
        let (clocks, pictures) = self.play_clocks();
        if clocks.is_empty() && !pictures {
            self.toast("Nothing on this page moves yet");
            return;
        }
        let posing = Rc::new(Posing { poses: HashMap::new(), clocks: Rc::new(clocks) });
        *self.play.borrow_mut() = Some(Play { tick: None, timing: Timing::default(), t_ms: 0.0, posing, pictures, ended: false, labelled: f64::NEG_INFINITY });
        self.hover.borrow_mut().take();
        self.run_play();
        self.show_play(true);
        self.content.borrow_mut().take();
        self.canvas.queue_draw();
    }

    /// Run the clock from where it is shown: a tick callback each frame (the frame clock, never
    /// a timer), and the pictures that animate themselves.
    fn run_play(&self) {
        let weak = self.me.clone();
        let tick = self.canvas.add_tick_callback(move |_, clock| {
            let Some(b) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            b.play_tick(clock.frame_time());
            glib::ControlFlow::Continue
        });
        match self.play.borrow_mut().as_mut() {
            Some(p) => {
                p.timing.resume();
                p.tick = Some(tick);
            }
            None => return tick.remove(),
        }
        if let Some(clock) = self.canvas.frame_clock() {
            self.painter.play_pictures(&clock);
        }
        self.show_time(true);
    }

    fn play_tick(&self, frame_us: i64) {
        let t = match self.play.borrow_mut().as_mut() {
            Some(p) => p.timing.at(frame_us),
            None => return,
        };
        self.play_frame(t);
    }

    /// Show the frame at `t_ms` (the poses are taken as it draws). Past the end of a motion that
    /// does not loop, with no picture animating itself, Play holds the end and pauses there, so a
    /// finished animation asks for no more frames.
    fn play_frame(&self, t_ms: f64) {
        let end = {
            let mut play = self.play.borrow_mut();
            let Some(p) = play.as_mut() else {
                return;
            };
            let (span, looped) = play_span(&p.posing.clocks);
            let end = ends(t_ms, span, looped, p.pictures);
            p.t_ms = if end { span } else { t_ms.max(0.0) };
            p.ended = end;
            end
        };
        if end {
            self.pause_play();
        }
        // Not `invalidate`: layout stands, and the chrome has nothing new to show.
        self.content.borrow_mut().take();
        self.canvas.queue_draw();
        self.show_time(false);
    }

    /// Hold Play where it is: the clock and the animated pictures stop until it resumes.
    pub(super) fn pause_play(&self) {
        let tick = {
            let mut play = self.play.borrow_mut();
            let Some(p) = play.as_mut() else {
                return;
            };
            let at = p.t_ms;
            p.timing.pause(at);
            p.tick.take()
        };
        let Some(tick) = tick else {
            return;
        };
        tick.remove();
        self.painter.pause_pictures();
        self.show_time(true);
    }

    /// Go on from where Play was paused; from the start once it has ended.
    pub(super) fn resume_play(&self) {
        {
            let mut play = self.play.borrow_mut();
            let Some(p) = play.as_mut().filter(|p| p.tick.is_none()) else {
                return;
            };
            if p.ended {
                p.ended = false;
                p.t_ms = 0.0;
                p.timing.seek(0.0);
            }
        }
        self.run_play();
        self.content.borrow_mut().take();
        self.canvas.queue_draw();
    }

    /// A paused frame at `ms` into the page's motion (the scrubber). Pictures that animate
    /// themselves cannot be sought, so they stay where they were.
    pub(super) fn seek(&self, ms: f64) {
        self.pause_play();
        {
            let mut play = self.play.borrow_mut();
            let Some(p) = play.as_mut() else {
                return;
            };
            let ms = if ms.is_finite() { ms.max(0.0) } else { 0.0 };
            p.t_ms = ms;
            p.timing.seek(ms);
            p.ended = false;
        }
        self.content.borrow_mut().take();
        self.canvas.queue_draw();
        self.show_time(true);
    }

    /// End Play: the page shows at rest again, its pictures at their first frame.
    pub(super) fn stop_play(&self) {
        let Some(play) = self.play.borrow_mut().take() else {
            return;
        };
        if let Some(tick) = play.tick {
            tick.remove();
        }
        self.painter.stop_pictures();
        self.painter.clear_stills();
        self.show_play(false);
        self.content.borrow_mut().take();
        self.canvas.queue_draw();
        self.overlay.queue_allocate();
        self.schedule_chrome();
    }

    /// The scene, the layout or the pictures changed while Play runs: its clocks again from the
    /// page as it is (the pictures kept of what rests go with them), at the same time. Nothing
    /// left to play ends it.
    fn refresh_play(&self) {
        if !self.playing() {
            return;
        }
        let (clocks, pictures) = self.play_clocks();
        if clocks.is_empty() && !pictures {
            self.stop_play();
            return;
        }
        let running = {
            let mut play = self.play.borrow_mut();
            let Some(p) = play.as_mut() else {
                return;
            };
            p.posing = Rc::new(Posing { poses: HashMap::new(), clocks: Rc::new(clocks) });
            p.pictures = pictures;
            p.tick.is_some()
        };
        // A picture that animates itself may have loaded since.
        if let Some(clock) = self.canvas.frame_clock().filter(|_| running) {
            self.painter.play_pictures(&clock);
        }
        self.show_time(true);
    }

    /// The time bar and the Play key as Play starts (`on`) or ends.
    fn show_play(&self, on: bool) {
        self.timebar.root.set_visible(on);
        // The toast rises above the bar while it shows.
        self.toast.set_margin_bottom(if on { 68 } else { 16 });
        let (icon, tip) = if on { ("stop", "Stop playing (Esc)") } else { ("play", PLAY_TIP) };
        self.play_key.set_child(Some(&crate::icons::image(icon, 16)));
        self.play_key.set_tooltip_text(Some(tip));
        self.play_key.update_property(&[gtk::accessible::Property::Label(tip)]);
        self.refresh_play_key();
        self.show_time(true);
    }

    /// The time bar: the time (at most every 100 ms unless `now`), the scrubber every frame, and
    /// the pause key as paused, playing or ended.
    fn show_time(&self, now: bool) {
        let mut play = self.play.borrow_mut();
        let Some(p) = play.as_mut() else {
            return;
        };
        let (span, looped) = play_span(&p.posing.clocks);
        let shown = if span > 0.0 { motion::local_time(p.t_ms, span, looped) } else { p.t_ms };
        let bar = &self.timebar;
        if span > 0.0 {
            let adjust = bar.scrub.adjustment();
            if adjust.upper() != span {
                adjust.set_upper(span);
            }
            bar.scrub.set_value(shown);
        }
        bar.scrub.set_visible(span > 0.0);
        if now || (p.t_ms - p.labelled).abs() >= 100.0 {
            p.labelled = p.t_ms;
            bar.time.set_text(&time_text(shown, span));
        }
        if now {
            let (icon, tip) = match (p.tick.is_some(), p.ended) {
                (true, _) => ("pause", "Pause"),
                (false, true) => ("resume", "Play again"),
                (false, false) => ("play", "Play"),
            };
            bar.pause.set_child(Some(&crate::icons::image(icon, 16)));
            bar.pause.set_tooltip_text(Some(tip));
        }
    }

    /// The Play key, the time bar's keys and scrubber, and a pause when the canvas is hidden.
    fn wire_play(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.play_key.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.toggle_play();
            }
        });
        let weak = Rc::downgrade(self);
        self.timebar.pause.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                let running = b.play.borrow().as_ref().is_some_and(|p| p.tick.is_some());
                if running {
                    b.pause_play();
                } else {
                    b.resume_play();
                }
            }
        });
        let weak = Rc::downgrade(self);
        self.timebar.restart.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.seek(0.0);
                b.resume_play();
            }
        });
        let weak = Rc::downgrade(self);
        self.timebar.stop.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.stop_play();
            }
        });
        // Only the person's drags and keys: the bar sets the value every frame itself.
        let weak = Rc::downgrade(self);
        self.timebar.scrub.connect_change_value(move |_, _, ms| {
            if let Some(b) = weak.upgrade() {
                b.seek(ms);
            }
            glib::Propagation::Proceed
        });
        // Hidden (another view, a minimised window), Play holds: nothing ticks behind it.
        let weak = Rc::downgrade(self);
        self.canvas.connect_unmap(move |_| {
            if let Some(b) = weak.upgrade() {
                b.pause_play();
            }
        });
        keep_canvas_focus(self.timebar.root.upcast_ref());
        self.timebar.scrub.set_focus_on_click(false);
    }

    // ------------------------------------------------------------------ chrome

    pub(super) fn refresh_chrome(&self) {
        self.refresh_actions();
        // The toolbar shows the selection's values: rebuild it, unless one of its popovers is
        // open (a colour being picked) or the text is being typed.
        let picking = self.context.borrow().as_ref().is_some_and(|bar| {
            let mut child = bar.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                if c.downcast_ref::<gtk::MenuButton>().and_then(|m| m.popover()).is_some_and(|p| p.is_visible()) {
                    return true;
                }
            }
            false
        });
        // Nor while one of its fields has the keyboard (a size being typed or stepped).
        let typing = self.context.borrow().as_ref().is_some_and(|bar| self.busy_in(bar));
        if !picking && !typing && self.edit.borrow().is_none() && matches!(*self.gesture.borrow(), Gesture::Idle) {
            self.rebuild_context();
        } else if self.edit.borrow().is_some() {
            self.refresh_context_values();
        }
        self.refresh_inspector_values();
        if let Some(layers) = self.layers.borrow().as_ref() {
            layers.refresh();
        }
        self.refresh_play_key();
        let page = self.page_name(&self.page.borrow());
        self.page_label.set_text(&page);
        // The whole name, which the key cuts short.
        if let Some(key) = self.page_label.ancestor(gtk::MenuButton::static_type()) {
            key.set_tooltip_text(Some(&format!("Pages: {page}")));
        }
        self.overlay.queue_allocate();
    }

    /// The widget with the keyboard in the board's window.
    pub(super) fn focus_widget(&self) -> Option<gtk::Widget> {
        self.root.root().and_then(|r| gtk::prelude::GtkWindowExt::focus(r.downcast_ref::<gtk::Window>()?))
    }

    /// Something in `panel` is being typed or picked: a field on screen has the keyboard, or
    /// one of the panel's popovers is open. Rebuilding the panel then would take it away.
    pub(super) fn busy_in(&self, panel: &impl IsA<gtk::Widget>) -> bool {
        self.focus_widget().is_some_and(|f| {
            f.is_ancestor(panel) && f.is_mapped() && (f.is::<gtk::Text>() || f.ancestor(gtk::Popover::static_type()).is_some())
        })
    }

    /// The keyboard in `panel` goes back to the canvas: what was typed in a field there is
    /// sent (to the layers it was typed for, before the selection changes), and the panel
    /// can be rebuilt without taking the focused widget with it, which would leave GTK to
    /// hand the keyboard to whatever comes next in the window.
    pub(super) fn release(&self, panel: &impl IsA<gtk::Widget>) {
        if self.focus_widget().is_some_and(|f| f.is_ancestor(panel)) {
            self.canvas.grab_focus();
        }
    }

    /// `release` the inspector and the toolbar.
    pub(super) fn release_fields(&self) {
        self.release(&self.inspector_scroll);
        let bar = self.context.borrow().clone();
        if let Some(bar) = bar {
            self.release(&bar);
        }
    }

    /// The keyboard goes back from `panel` to the canvas on Esc (a field has put back what it
    /// showed by then), and on a press that is not in one of its fields, so the panel shows
    /// what the key pressed does (it is not rebuilt while a field has the keyboard, and its
    /// keys take none).
    pub(super) fn hand_back(self: &Rc<Self>, panel: &impl IsA<gtk::Widget>) {
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(b) = weak.upgrade().filter(|_| key == gdk::Key::Escape) else {
                return glib::Propagation::Proceed;
            };
            b.canvas.grab_focus();
            b.schedule_chrome();
            glib::Propagation::Stop
        });
        panel.as_ref().add_controller(keys);
        let press = gtk::GestureClick::new();
        press.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        press.connect_pressed(move |g, _, x, y| {
            let (Some(b), Some(panel)) = (weak.upgrade(), g.widget()) else {
                return;
            };
            let in_field = panel
                .pick(x, y, gtk::PickFlags::DEFAULT)
                .is_some_and(|w| w.is::<gtk::Text>() || w.ancestor(gtk::Entry::static_type()).is_some());
            if !in_field {
                b.release(&panel);
            }
        });
        panel.as_ref().add_controller(press);
    }

    pub(super) fn set_tool(&self, tool: Tool) {
        if tool != Tool::Vector {
            super::tools::finish_vector(self, false);
        }
        // Another tool lets go of the lock on the last one.
        if self.tool.replace(tool) != tool {
            self.tool_lock.set(false);
        }
        for (t, key) in self.tool_keys.borrow().iter() {
            key.set_active(*t == tool);
        }
        let shapes = tool.is_shape();
        self.shape_key.set_active(shapes);
        let pens = matches!(tool, Tool::Pen | Tool::Vector | Tool::Highlighter | Tool::Eraser);
        self.pen_key.set_active(pens);
        if shapes {
            tip(&self.shape_key, &tool.tip());
        }
        if pens {
            tip(&self.pen_key, &tool.tip());
        }
        self.show_tool_lock();
        if tool != Tool::Select {
            self.end_edit(true);
        }
        super::tools::update_cursor(self, None);
    }

    /// Keep the tool on hand after it makes something, or let it go back to Move after each
    /// (a double click on its key; I-16).
    pub(super) fn toggle_tool_lock(&self) {
        let locked = !self.tool_lock.get() && self.tool.get() != Tool::Select && self.tool.get() != Tool::Hand;
        self.tool_lock.set(locked);
        self.show_tool_lock();
        if locked {
            self.toast(&format!("{} stays on until you pick another tool", self.tool.get().caption()));
        }
    }

    /// Each tool key's glyph, the tool on hand's with a lock while it is kept.
    fn show_tool_lock(&self) {
        let (tool, locked) = (self.tool.get(), self.tool_lock.get());
        for (t, key) in self.tool_keys.borrow().iter() {
            key.set_child(Some(&tool_glyph(*t, locked && *t == tool)));
        }
        if tool.is_shape() {
            self.shape_key.set_child(Some(&tool_glyph(tool, locked)));
        }
        if matches!(tool, Tool::Pen | Tool::Vector | Tool::Highlighter | Tool::Eraser) {
            self.pen_key.set_child(Some(&tool_glyph(tool, locked)));
        }
    }

    fn build_dock(self: &Rc<Self>, dock: &gtk::Box) {
        let layers_key = gtk::ToggleButton::new();
        layers_key.add_css_class("design-tool");
        layers_key.set_child(Some(&glyphs::image("layers", 16)));
        tip(&layers_key, "Pages and layers (Alt+1)");
        layers_key.set_widget_name("design-layers-key");
        dock.append(&layers_key);
        let weak = Rc::downgrade(self);
        layers_key.connect_toggled(move |key| {
            LAYERS_ON.with(|on| on.set(key.is_active()));
            if let Some(b) = weak.upgrade() {
                b.layers_panel.set_reveal_child(key.is_active());
                if let Some(l) = b.layers.borrow().as_ref() {
                    l.refresh();
                }
            }
        });
        let weak = Rc::downgrade(self);
        self.layers_panel.connect_reveal_child_notify(move |r| {
            if let Some(b) = weak.upgrade() {
                b.layers_panel.set_can_target(r.reveals_child());
            }
        });
        self.layers_panel.set_can_target(false);
        // Open again if it was open on the last design.
        if LAYERS_ON.with(Cell::get) {
            layers_key.set_active(true);
        }
        let page_key = gtk::MenuButton::new();
        page_key.add_css_class("design-page-key");
        let page_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        page_row.append(&self.page_label);
        page_row.append(&crate::icons::image("chevron-down", 12));
        page_key.set_child(Some(&page_row));
        page_key.set_tooltip_text(Some("Pages"));
        let pages_popover = gtk::Popover::new();
        pages_popover.add_css_class("design-menu");
        page_key.set_popover(Some(&pages_popover));
        *self.pages_popover.borrow_mut() = Some(pages_popover.clone());
        let weak = Rc::downgrade(self);
        pages_popover.connect_show(move |pop| {
            if let Some(b) = weak.upgrade() {
                pop.set_child(Some(&super::layers::pages_menu(&b, pop)));
            }
        });
        dock.append(&page_key);
        let sep = gtk::Separator::new(gtk::Orientation::Vertical);
        sep.add_css_class("design-dock-sep");
        dock.append(&sep);
        let mut keys = Vec::new();
        for tool in [Tool::Select, Tool::Hand, Tool::Frame] {
            let key = self.tool_key(tool);
            dock.append(&key);
            keys.push((tool, key));
        }
        dock.append(&self.frame_presets());
        // Shapes share a key; its menu picks which.
        self.shape_key.add_css_class("design-tool");
        self.shape_key.set_child(Some(&glyphs::image("rect", 16)));
        tip(&self.shape_key, &Tool::Rect.tip());
        let weak = Rc::downgrade(self);
        self.shape_key.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                let current = b.tool.get();
                let tool = if current.is_shape() {
                    current
                } else {
                    b.last_shape()
                };
                b.set_tool(tool);
                key.set_active(true);
            }
        });
        self.lock_on_double_click(&self.shape_key);
        dock.append(&self.shape_key);
        dock.append(&self.tool_menu(&Tool::SHAPES));
        for tool in [Tool::Text, Tool::Sticky] {
            let key = self.tool_key(tool);
            dock.append(&key);
            keys.push((tool, key));
        }
        self.pen_key.add_css_class("design-tool");
        self.pen_key.set_child(Some(&glyphs::image("pen", 16)));
        tip(&self.pen_key, &Tool::Pen.tip());
        let weak = Rc::downgrade(self);
        self.pen_key.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                let tool = match b.tool.get() {
                    t @ (Tool::Vector | Tool::Highlighter | Tool::Eraser) => t,
                    _ => Tool::Pen,
                };
                b.set_tool(tool);
                key.set_active(true);
            }
        });
        self.lock_on_double_click(&self.pen_key);
        dock.append(&self.pen_key);
        dock.append(&self.tool_menu(&[Tool::Pen, Tool::Vector, Tool::Highlighter, Tool::Eraser]));
        let icon_key = gtk::MenuButton::new();
        icon_key.add_css_class("design-tool");
        icon_key.set_child(Some(&glyphs::image("icon", 16)));
        tip(&icon_key, "Place an icon (K)");
        icon_key.set_widget_name("design-icon-key");
        let picker = gtk::Popover::new();
        picker.add_css_class("design-menu");
        icon_key.set_popover(Some(&picker));
        let weak = Rc::downgrade(self);
        picker.connect_show(move |pop| {
            if let Some(b) = weak.upgrade() {
                // The dock key and K place a new icon; replacing one is the toolbar's (TM-73).
                pop.set_child(Some(&super::icons::picker(&b, pop, None)));
            }
        });
        dock.append(&icon_key);
        // A file from this computer is one click into its popover, and still Ctrl+Shift+K; the
        // rest of it is stock photos (pictures-SPEC-v2 §13.2).
        let image_key = gtk::MenuButton::new();
        image_key.add_css_class("design-tool");
        image_key.set_child(Some(&glyphs::image("image", 16)));
        tip(&image_key, "Place an image or a stock photo");
        image_key.set_widget_name("design-image-key");
        let picker = gtk::Popover::new();
        picker.add_css_class("design-menu");
        image_key.set_popover(Some(&picker));
        let weak = Rc::downgrade(self);
        picker.connect_show(move |pop| {
            if let Some(b) = weak.upgrade() {
                pop.set_child(Some(&super::pictures::picker(&b, pop)));
            }
        });
        let weak = Rc::downgrade(self);
        picker.connect_closed(move |pop| {
            if let Some(b) = weak.upgrade() {
                super::pictures::closed(&b, pop);
            }
        });
        dock.append(&image_key);
        *self.tool_keys.borrow_mut() = keys;
        self.set_tool(Tool::Select);
        keep_canvas_focus(dock.upcast_ref());
    }

    /// The frame key's menu: the screens people design for, placed beside what is on the page.
    fn frame_presets(self: &Rc<Self>) -> gtk::MenuButton {
        let menu = gtk::MenuButton::new();
        menu.add_css_class("design-tool-more");
        menu.set_child(Some(&crate::icons::image("chevron-down", 10)));
        tip(&menu, "Frame sizes");
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.append(&label("Frame sizes", "design-menu-heading"));
        for (name, w, h) in FRAME_PRESETS {
            let row = gtk::Button::new();
            row.add_css_class("design-menu-row");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let caption = label(name, "");
            caption.set_hexpand(true);
            content.append(&caption);
            content.append(&label(&format!("{w}×{h}"), "design-keycap"));
            row.set_child(Some(&content));
            let weak = Rc::downgrade(self);
            let p = pop.downgrade();
            row.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if let Some(b) = weak.upgrade() {
                    b.place_frame(name, w as f64, h as f64);
                }
            });
            list.append(&row);
        }
        pop.set_child(Some(&list));
        menu.set_popover(Some(&pop));
        menu
    }

    /// A new artboard `w` by `h` to the right of the page's content, selected and in view.
    pub(super) fn place_frame(&self, name: &str, w: f64, h: f64) {
        let page = self.page.borrow().clone();
        let (x, y) = self.page_bounds().map_or((0.0, 0.0), |b| ((b.right() + 80.0).round(), b.y.round()));
        let mut n = Node::new(NodeKind::Frame);
        n.id = self.doc.scene.borrow_mut().fresh_id(NodeKind::Frame);
        n.parent = Some(page);
        n.name = Some(name.to_string());
        (n.x, n.y, n.w, n.h) = (x, y, w, h);
        let id = n.id.clone();
        if self.commit(vec![NodeOp::Add { node: n, index: None }]) {
            self.select(vec![id]);
            let r = self.selection_bounds();
            self.fit(r, false);
            self.set_tool(Tool::Select);
        }
    }

    fn last_shape(&self) -> Tool {
        self.shape_key
            .tooltip_text()
            .map(|t| t.to_string())
            .and_then(|t| Tool::SHAPES.into_iter().find(|tool| t.starts_with(tool.caption())))
            .unwrap_or(Tool::Rect)
    }

    fn tool_key(self: &Rc<Self>, tool: Tool) -> gtk::ToggleButton {
        let key = gtk::ToggleButton::new();
        key.add_css_class("design-tool");
        key.set_child(Some(&glyphs::image(tool.glyph(), 16)));
        key.set_tooltip_text(Some(&tool.tip()));
        key.set_widget_name(&format!("design-tool-{}", tool.glyph()));
        key.update_property(&[gtk::accessible::Property::Label(tool.caption())]);
        self.lock_on_double_click(&key);
        let weak = Rc::downgrade(self);
        key.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                b.set_tool(tool);
                key.set_active(true);
            }
        });
        key
    }

    /// A double click on a tool key locks its tool on.
    fn lock_on_double_click(self: &Rc<Self>, key: &gtk::ToggleButton) {
        let clicks = gtk::GestureClick::new();
        clicks.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        clicks.connect_pressed(move |_, n, _, _| {
            if let Some(b) = weak.upgrade().filter(|_| n == 2) {
                b.toggle_tool_lock();
            }
        });
        key.add_controller(clicks);
    }

    /// A narrow chevron key whose menu lists `tools`.
    fn tool_menu(self: &Rc<Self>, tools: &[Tool]) -> gtk::MenuButton {
        let menu = gtk::MenuButton::new();
        menu.add_css_class("design-tool-more");
        menu.set_child(Some(&crate::icons::image("chevron-down", 10)));
        tip(&menu, "More tools");
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        for tool in tools {
            let row = gtk::Button::new();
            row.add_css_class("design-menu-row");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            content.append(&glyphs::image(tool.glyph(), 16));
            let name = label(tool.caption(), "");
            name.set_hexpand(true);
            content.append(&name);
            if !tool.key().is_empty() {
                content.append(&label(tool.key(), "design-keycap"));
            }
            row.set_child(Some(&content));
            let weak = Rc::downgrade(self);
            let tool = *tool;
            let p = pop.downgrade();
            row.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if let Some(b) = weak.upgrade() {
                    b.set_tool(tool);
                }
            });
            list.append(&row);
        }
        if tools.contains(&Tool::Pen) {
            list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            let swatches = gtk::FlowBox::new();
            swatches.set_max_children_per_line(6);
            swatches.set_selection_mode(gtk::SelectionMode::None);
            for color in ["", "#1A1A1A", "#EDE9E2", "#FF5C5C", "#FFB020", "#2EC469", "#4C9DFF", "#A178FF", "#FF6FB5"] {
                let swatch = swatch_button(if color.is_empty() { None } else { Some(color) }, if color.is_empty() { "Automatic" } else { color });
                let weak = Rc::downgrade(self);
                let p = pop.downgrade();
                swatch.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        // The drawing tool on hand takes it; any other picks the pen.
                        let tool = match b.tool.get() {
                            t @ (Tool::Pen | Tool::Highlighter | Tool::Vector) => t,
                            _ => Tool::Pen,
                        };
                        b.pens.borrow_mut().insert(tool, color.to_string());
                        b.set_tool(tool);
                    }
                    if let Some(p) = p.upgrade() {
                        p.popdown();
                    }
                });
                swatches.insert(&swatch, -1);
            }
            list.append(&swatches);
        }
        pop.set_child(Some(&list));
        menu.set_popover(Some(&pop));
        menu
    }

    fn build_zoom_menu(self: &Rc<Self>, key: &gtk::MenuButton) {
        let menu = gio::Menu::new();
        let section = gio::Menu::new();
        section.append_item(&menu_item("Zoom in", "zoom-in", Some("<Control>equal")));
        section.append_item(&menu_item("Zoom out", "zoom-out", Some("<Control>minus")));
        menu.append_section(None, &section);
        let section = gio::Menu::new();
        section.append_item(&menu_item("Zoom to fit", "zoom-fit", Some("<Shift>1")));
        section.append_item(&menu_item("Zoom to selection", "zoom-selection", Some("<Shift>2")));
        section.append_item(&menu_item("Zoom to 50%", "zoom-50", None));
        section.append_item(&menu_item("Zoom to 100%", "zoom-100", Some("<Shift>0")));
        section.append_item(&menu_item("Zoom to 200%", "zoom-200", None));
        menu.append_section(None, &section);
        key.set_menu_model(Some(&menu));
    }

    fn build_title_menu(self: &Rc<Self>, key: &gtk::MenuButton) {
        let menu = gio::Menu::new();
        let section = gio::Menu::new();
        section.append(Some("Rename…"), Some("design.rename"));
        section.append(Some("Duplicate design"), Some("design.duplicate-design"));
        if !self.windowed {
            section.append(Some("Open in a new window"), Some("design.pop-out"));
        }
        menu.append_section(None, &section);
        let section = gio::Menu::new();
        section.append(Some("Export page as PNG (2×)…"), Some("design.export-page"));
        section.append(Some("Export page as SVG…"), Some("design.export-page-svg"));
        section.append(Some("Export page as PDF…"), Some("design.export-page-pdf"));
        section.append(Some("Export page as animation (APNG)…"), Some("design.export-page-apng"));
        section.append(Some("Export page as PNG frames…"), Some("design.export-page-frames"));
        menu.append_section(None, &section);
        menu.append_section(None, &export_menu());
        let section = gio::Menu::new();
        section.append(Some("Delete design"), Some("design.delete-design"));
        menu.append_section(None, &section);
        key.set_menu_model(Some(&menu));
    }

    /// The terminal key's menu: the project's sessions to tie the design to, or none.
    pub(super) fn refresh_terminal_menu(self: &Rc<Self>) {
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        self.terminal.set_popover(Some(&pop));
        let weak = Rc::downgrade(self);
        pop.connect_show(move |pop| {
            let Some(b) = weak.upgrade() else {
                return;
            };
            let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
            list.append(&label("Belongs to", "design-menu-heading"));
            pop.set_child(Some(&list));
            let Some(ui) = b.ui.upgrade() else {
                return;
            };
            let project = b.doc.design.borrow().project_id;
            let p = pop.downgrade();
            let weak = Rc::downgrade(&b);
            glib::spawn_future_local(async move {
                let sessions = ui.call("session.list", json!({"project_id": project})).await;
                let (Some(b), Some(pop)) = (weak.upgrade(), p.upgrade()) else {
                    return;
                };
                let current = b.doc.session();
                let rows = sessions.map(|v| crate::app::rows(&v, "sessions")).unwrap_or_default();
                if let Some(c) = &current {
                    b.set_lamp(rows.iter().find(|s| crate::app::text(s, "name") == c).map_or("closed", |s| crate::app::text(s, "state")));
                }
                let mut names: Vec<(String, String)> = rows
                    .iter()
                    .map(|s| (crate::app::text(s, "name").to_string(), crate::app::text(s, "state").to_string()))
                    .collect();
                if let Some(c) = &current {
                    if !names.iter().any(|(n, _)| n == c) {
                        names.insert(0, (c.clone(), "closed".into()));
                    }
                }
                for (name, state) in names {
                    let row = gtk::Button::new();
                    row.add_css_class("design-menu-row");
                    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    lamp.add_css_class("design-lamp");
                    lamp.add_css_class(&state);
                    lamp.set_valign(gtk::Align::Center);
                    content.append(&lamp);
                    let caption = label(&name, "");
                    caption.set_hexpand(true);
                    content.append(&caption);
                    if current.as_deref() == Some(name.as_str()) {
                        content.append(&crate::icons::image("check", 14));
                    }
                    row.set_child(Some(&content));
                    let weak = Rc::downgrade(&b);
                    let pp = pop.downgrade();
                    row.connect_clicked(move |_| {
                        if let Some(p) = pp.upgrade() {
                            p.popdown();
                        }
                        if let Some(b) = weak.upgrade() {
                            b.link_session(Some(name.clone()));
                        }
                    });
                    list.append(&row);
                }
                let none = gtk::Button::new();
                none.add_css_class("design-menu-row");
                let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                let caption = label("Project only", "");
                caption.set_hexpand(true);
                content.append(&caption);
                if current.is_none() {
                    content.append(&crate::icons::image("check", 14));
                }
                none.set_child(Some(&content));
                let weak = Rc::downgrade(&b);
                let pp = pop.downgrade();
                none.connect_clicked(move |_| {
                    if let Some(p) = pp.upgrade() {
                        p.popdown();
                    }
                    if let Some(b) = weak.upgrade() {
                        b.link_session(None);
                    }
                });
                list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
                list.append(&none);
            });
        });
    }

    pub(super) fn link_session(&self, session: Option<String>) {
        // The terminal it already belongs to: nothing to write (TM-58).
        if session == self.doc.session() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let id = self.doc.id;
        let doc = self.doc.clone();
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            match ui.call("design.update", json!({"design_id": id, "session": session})).await {
                Ok(v) => {
                    if let Ok(row) = serde_json::from_value(v) {
                        doc.adopt(row);
                    }
                    if let Some(b) = weak.upgrade() {
                        b.design_changed();
                    }
                    super::gallery_changed(&ui);
                }
                Err(e) => {
                    if let Some(b) = weak.upgrade() {
                        b.toast(&e.to_string());
                    }
                }
            }
        });
    }

    /// The keyboard reference, grouped as people look for it.
    fn shortcuts(&self, anchor: &gtk::Widget) {
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        pop.set_parent(anchor);
        let grid = gtk::Grid::new();
        grid.set_column_spacing(24);
        grid.set_row_spacing(4);
        grid.add_css_class("design-shortcuts");
        type Group<'a> = (&'a str, &'a [(&'a str, &'a str)]);
        let tools: Group = ("Tools", &[("V", "Move"), ("H or Space", "Hand"), ("F", "Frame"), ("R / O", "Rectangle / ellipse"), ("Shift R / Shift S", "Triangle / star"), ("K", "Icon"), ("E", "Eraser"), ("L / Shift L", "Line / arrow"), ("T", "Text"), ("S", "Sticky note"), ("P / Shift P", "Pen / highlighter"), ("B", "Vector pen: click points, close on the first")]);
        let text: Group = ("Text", &[("Esc or Ctrl Enter", "Finish typing"), ("Ctrl B / I / U", "Bold / italic / underline")]);
        let view: Group = ("View", &[("Ctrl scroll / pinch", "Zoom at the pointer"), ("Alt scroll", "Scroll a scrolling frame"), ("Ctrl − / Ctrl =", "Zoom out / in"), ("Shift 1", "Zoom to fit"), ("Shift 2", "Zoom to selection"), ("Shift 0 or Ctrl 0", "Zoom to 100%"), ("Alt 1", "Pages and layers"), ("Ctrl .", "Inspector"), ("Ctrl P", "Play the page's motion; Esc stops")]);
        let selection: Group = ("Selection", &[("Click", "Select what is under the pointer"), ("Esc", "Select the parent"), ("Enter", "Edit text, or select inside"), ("Tab / Shift Tab", "Next / previous layer"), ("Shift click", "Add or remove"), ("Ctrl A", "Select all"), ("Alt drag", "Duplicate while moving"), ("Alt while dragging", "No snapping"), ("Alt drag a handle", "Resize from the centre"), ("Shift drag a handle", "Keep proportions")]);
        let edit: Group = ("Edit", &[("Ctrl Z / Ctrl Shift Z", "Undo / redo"), ("Ctrl C / X / V", "Copy / cut / paste"), ("Ctrl D", "Duplicate"), ("Delete / Backspace", "Delete"), ("Ctrl G / Ctrl Shift G", "Group / ungroup"), ("Ctrl Alt G", "Frame selection"), ("Ctrl ] / Ctrl [", "Forward / backward"), ("Ctrl Shift ] / [", "Bring to front / send to back"), ("Shift H / Shift V", "Flip horizontally / vertically"), ("Ctrl Alt C / V", "Copy / paste style"), ("Alt A / D / W / S", "Align left / right / top / bottom"), ("Alt H / Alt V", "Align centres / middles"), ("Ctrl Shift L / H", "Lock / hide"), ("Ctrl Shift K", "Place an image"), ("Ctrl Enter", "Ask the agent"), ("Arrows", "Nudge 1 px (Shift: 10)")]);
        // Two columns, each group under the last, so a longer group never runs into the next.
        for (col, groups) in [[tools, text, view].as_slice(), &[selection, edit]].iter().enumerate() {
            let x = col as i32 * 2;
            let mut y = 0;
            for (heading, rows) in groups.iter() {
                grid.attach(&label(heading, "design-menu-heading"), x, y, 2, 1);
                for (i, (keys, what)) in rows.iter().enumerate() {
                    grid.attach(&label(keys, "design-keycap"), x, y + 1 + i as i32, 1, 1);
                    grid.attach(&label(what, "dim"), x + 1, y + 1 + i as i32, 1, 1);
                }
                y += 1 + rows.len() as i32;
            }
        }
        pop.set_child(Some(&grid));
        pop.connect_closed(|p| p.unparent());
        pop.popup();
    }

    // ------------------------------------------------------------------ floating widgets

    fn child_position(&self, widget: &gtk::Widget) -> Option<gdk::Rectangle> {
        if let Some(panel) = self.agent_panel.borrow().as_ref().filter(|p| p.root.upcast_ref::<gtk::Widget>() == widget) {
            let (w, h) = self.size.get();
            return Some(panel.placed(w, h));
        }
        if let Some(edit) = self.edit.borrow().as_ref() {
            if edit.view.upcast_ref::<gtk::Widget>() == widget {
                let Some(r) = self.edit_rect() else {
                    return Some(gdk::Rectangle::new(-10_000, -10_000, 10, 10));
                };
                let wraps = edit.view.wrap_mode() != gtk::WrapMode::None;
                let (x, y, w, h, left, top) = editor_box(&self.screen(&r), wraps);
                // The words start where the canvas draws them, not at the whole pixel before,
                // and wrap at the canvas's width (TM-32, E-text-14).
                // Set after this layout pass, not during it.
                if edit.view.left_margin() != left || edit.view.top_margin() != top {
                    let view = edit.view.downgrade();
                    glib::idle_add_local_once(move || {
                        if let Some(view) = view.upgrade() {
                            view.set_left_margin(left);
                            view.set_top_margin(top);
                        }
                    });
                }
                return Some(gdk::Rectangle::new(x, y, w, h));
            }
        }
        let context = self.context.borrow().clone()?;
        if &context != widget {
            return None;
        }
        let (_, nat) = context.preferred_size();
        let (w, h) = (nat.width(), nat.height());
        // Out of sight while a gesture runs or nothing is selected; an overlay child given no
        // place would cover the whole canvas.
        let away = gdk::Rectangle::new(-10_000, -10_000, w, h);
        if self.selection.borrow().is_empty() || !matches!(*self.gesture.borrow(), Gesture::Idle) || self.playing() {
            return Some(away);
        }
        let Some(b) = self.selection_bounds() else {
            return Some(away);
        };
        let s = self.screen(&b);
        let (cw, ch) = self.size.get();
        // Scrolled out of sight, the selection takes its toolbar with it rather than leave it
        // over what is on show (E-board-4); it comes back with the selection.
        if !s.intersects(&Rect::new(0.0, 0.0, cw, ch)) {
            return Some(away);
        }
        // Between the layers panel and the inspector when they show and the bar fits there; it
        // sat over their own controls otherwise. Too narrow for both, it keeps clear of what it can.
        let inspector = self.inspector_scroll.is_visible().then(|| f64::from(self.inspector_scroll.width().max(INSPECTOR_W as i32)) + 12.0);
        let layers = self.layers_panel.reveals_child().then(|| f64::from(self.layers_panel.width()) + 12.0);
        // Wider than the room beside the inspector, the bar drops its words and tightens its keys
        // (a text's ran 640px, over the inspector in windows under ~1150px); wide enough again,
        // they come back. The switch waits for this layout pass to end, out of sight meanwhile.
        if !self.context_compact.get() {
            self.context_full.set(Some(w));
        }
        let full = f64::from(self.context_full.get().unwrap_or(w));
        let compact = full > cw - inspector.unwrap_or(0.0) - 16.0;
        if compact != self.context_compact.get() {
            if !self.context_refit.replace(true) {
                let weak = self.me.clone();
                glib::idle_add_local_once(move || {
                    if let Some(b) = weak.upgrade() {
                        b.context_refit.set(false);
                        b.set_context_compact(compact);
                    }
                });
            }
            return Some(away);
        }
        let mut right = cw - 8.0;
        if let Some(iw) = inspector.filter(|iw| cw - iw - 8.0 - w as f64 >= 8.0) {
            right = cw - iw - 8.0;
        }
        let mut left = 8.0;
        if let Some(lw) = layers.filter(|lw| right - w as f64 >= lw + 8.0) {
            left = lw + 8.0;
        }
        let mut x = (s.cx() - w as f64 / 2.0).round();
        x = x.clamp(left, (right - w as f64).max(left));
        let above = s.y - h as f64 - 12.0;
        let mut y = if above > 56.0 { above } else { s.bottom() + 36.0 };
        y = y.clamp(56.0, (ch - h as f64 - 56.0).max(56.0));
        self.context_at.set((x as i32, y as i32));
        Some(gdk::Rectangle::new(x as i32, y as i32, w, h))
    }

    // ------------------------------------------------------------------ actions

    fn install_actions(self: &Rc<Self>) {
        let add = |name: &str, f: fn(&Board)| {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(b) = weak.upgrade() {
                    // A menu or action in the middle of a drag puts the drag back first, so the
                    // release does not overwrite what it did (TM-43).
                    b.cancel_gesture();
                    f(&b);
                }
            });
            self.actions.add_action(&action);
        };
        add("undo", |b| b.undo());
        add("redo", |b| b.redo());
        add("cut", |b| b.cut());
        add("copy", |b| b.copy());
        add("paste", |b| b.paste());
        add("paste-here", |b| b.paste_to(b.menu_at.take()));
        add("duplicate", |b| b.duplicate());
        add("delete", |b| b.delete_selection());
        add("group", |b| b.group(NodeKind::Group));
        add("frame", |b| b.group(NodeKind::Frame));
        add("ungroup", |b| b.ungroup());
        add("forward", |b| b.restack(true, false));
        add("front", |b| b.restack(true, true));
        add("backward", |b| b.restack(false, false));
        add("back", |b| b.restack(false, true));
        add("lock", |b| b.toggle_flag("locked"));
        add("hide", |b| b.toggle_flag("hidden"));
        add("select-all", |b| b.select_all());
        add("zoom-in", |b| b.step_zoom(1));
        add("zoom-out", |b| b.step_zoom(-1));
        add("zoom-fit", |b| {
            let r = b.page_bounds();
            b.fit(r, false);
        });
        add("zoom-selection", |b| {
            let r = b.selection_painted().or_else(|| b.page_bounds());
            b.fit(r, true);
        });
        add("zoom-50", |b| b.zoom_to(0.5));
        add("zoom-100", |b| b.zoom_to(1.0));
        add("zoom-200", |b| b.zoom_to(2.0));
        use super::export::{export_animation, export_frames, export_selection, Format};
        add("export", |b| export_selection(b, false, Format::Png(2.0)));
        add("export-svg", |b| export_selection(b, false, Format::Svg));
        add("export-pdf", |b| export_selection(b, false, Format::Pdf));
        add("export-page", |b| export_selection(b, true, Format::Png(2.0)));
        add("export-page-svg", |b| export_selection(b, true, Format::Svg));
        add("export-page-pdf", |b| export_selection(b, true, Format::Pdf));
        add("export-apng", |b| export_animation(b, false));
        add("export-frames", |b| export_frames(b, false));
        add("export-page-apng", |b| export_animation(b, true));
        add("export-page-frames", |b| export_frames(b, true));
        add("flip-h", |b| b.flip(true));
        add("flip-v", |b| b.flip(false));
        add("copy-style", |b| b.copy_style());
        add("paste-style", |b| b.paste_style());
        add("ask", |b| b.ask_agent(b.ask.upcast_ref()));
        add("rename", |b| b.rename_design());
        add("duplicate-design", super::duplicate_design);
        add("delete-design", super::delete_design);
        add("pop-out", |b| b.pop_out());
        add("add-page", |b| b.add_page());
        add("play", |b| b.toggle_play());
    }

    fn refresh_actions(&self) {
        let has = !self.selection.borrow().is_empty();
        // The animation entries wait for something that moves (feature-motion §3).
        let (moves, page_moves) = {
            let (scene, page) = (self.doc.scene.borrow(), self.page.borrow());
            (has && super::export::moves(&scene, &page, &self.selection.borrow()), super::export::moves(&scene, &page, &[]))
        };
        for (name, on) in [
            ("undo", self.doc.can_undo()),
            ("redo", self.doc.can_redo()),
            ("cut", has),
            ("copy", has),
            ("duplicate", has),
            ("delete", has),
            ("group", has),
            ("frame", has),
            ("forward", has),
            ("front", has),
            ("backward", has),
            ("back", has),
            ("lock", has),
            ("hide", has),
            ("export", has),
            ("export-svg", has),
            ("export-pdf", has),
            ("export-apng", moves),
            ("export-frames", moves),
            ("export-page-apng", page_moves),
            ("export-page-frames", page_moves),
            ("flip-h", has),
            ("flip-v", has),
            ("copy-style", has),
            ("paste-style", has && self.style_clip.borrow().is_some()),
        ] {
            if let Some(a) = self.actions.lookup_action(name).and_downcast::<gio::SimpleAction>() {
                a.set_enabled(on);
            }
        }
    }

    /// The newest change `agent` made to this design (TM-11): audited `design.apply` rows of
    /// the project, read back 500 at a time until one of this design turns up, at most five
    /// times (E-ask-8: an agent busy elsewhere pushes it past one page).
    async fn agent_change(ui: &Rc<Ui>, design: i64, project: i64, agent: &str) -> AgentChange {
        let mut until: Option<(Value, i64)> = None;
        for _ in 0..AGENT_PAGES {
            let mut ask = json!({"project_id": project, "actor": format!("agent:{agent}"), "op_prefix": "design.apply", "limit": AGENT_PAGE});
            if let Some((ts, _)) = &until {
                ask["until"] = ts.clone();
            }
            let Ok(listed) = ui.call("audit.list", ask).await else {
                return AgentChange::None;
            };
            let mut rows = crate::app::rows(&listed, "rows");
            if let Some((_, below)) = &until {
                rows.retain(|r| r["id"].as_i64().is_some_and(|id| id < *below));
            }
            if let Some(found) = pick_agent_change(&rows, design) {
                return found;
            }
            match rows.iter().filter_map(|r| Some((r["ts"].clone(), r["id"].as_i64()?))).min_by_key(|(_, id)| *id) {
                Some(oldest) if crate::app::rows(&listed, "rows").len() >= AGENT_PAGE as usize => until = Some(oldest),
                _ => return AgentChange::None,
            }
        }
        AgentChange::None
    }

    /// Show Undo agent for `agent`'s newest change still standing, as found: the key when it
    /// can be undone, the reason when it is too large to (I-7). For a board opening on a design
    /// an agent edited last (E-ask-9).
    fn offer_agent_undo(&self, agent: String) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let design = self.doc.id;
        let project = self.doc.design.borrow().project_id;
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let found = Self::agent_change(&ui, design, project, &agent).await;
            let Some(b) = weak.upgrade() else {
                return;
            };
            // An agent's change heard meanwhile has set the key already.
            if b.agent_last.borrow().as_ref().is_some_and(|a| *a != agent) {
                return;
            }
            match found {
                AgentChange::Undoable(_) => {
                    b.set_status(&format!("Last edited by {agent}"), None);
                    *b.agent_last.borrow_mut() = Some(agent.clone());
                    b.agent_undo.set_tooltip_text(Some(&format!("Undo {agent}'s last change to this design")));
                    b.agent_undo.set_visible(true);
                }
                AgentChange::TooLarge => {
                    b.set_status(&format!("Last edited by {agent}"), None);
                    b.agent_undo.set_tooltip_text(Some(&too_large(&agent)));
                }
                AgentChange::None => {}
            }
        });
    }

    /// Revert the last change the agent named by the status made, through `audit.undo`: the
    /// person's veto over what an agent drew. Each press undoes one more of its changes, and a
    /// press while one is on its way is dropped (B80). A change the person edited since is
    /// kept, with what was edited named; a second press on it within ten seconds undoes it
    /// anyway. A change too large to have kept its undo is said to be, and nothing older is
    /// undone in its place (TM-11).
    pub(super) fn undo_agent(&self) {
        let (Some(ui), Some(agent)) = (self.ui.upgrade(), self.agent_last.borrow().clone()) else {
            return;
        };
        if self.agent_undoing.replace(true) {
            return;
        }
        self.agent_undo.set_sensitive(false);
        let design = self.doc.id;
        let project = self.doc.design.borrow().project_id;
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let found = Self::agent_change(&ui, design, project, &agent).await;
            let audit = match found {
                AgentChange::Undoable(audit) => audit,
                other => {
                    if let Some(b) = weak.upgrade() {
                        b.agent_undoing.set(false);
                        b.agent_undo.set_sensitive(true);
                        b.agent_undo.set_visible(false);
                        b.toast(&match other {
                            AgentChange::TooLarge => too_large(&agent),
                            _ => format!("Nothing of {agent}'s left to undo here"),
                        });
                    }
                    return;
                }
            };
            let now = glib::monotonic_time();
            let force = weak.upgrade().is_some_and(|b| b.agent_force.take().is_some_and(|(id, at)| id == audit && now - at < FORCE_US));
            let mut ask = json!({"audit_id": audit});
            if force {
                ask["force"] = json!(true);
            }
            let undone = ui.call("audit.undo", ask).await;
            let more = Self::agent_change(&ui, design, project, &agent).await;
            let Some(b) = weak.upgrade() else {
                return;
            };
            b.agent_undoing.set(false);
            b.agent_undo.set_sensitive(true);
            match undone {
                Ok(_) => {
                    b.set_status(&format!("Undid {agent}'s change"), None);
                    b.agent_undo.set_visible(more != AgentChange::None);
                }
                Err(crate::client::Error::Bus(e)) if e.code == "audit.stale" => {
                    let what = e.details.as_ref().map(|d| b.conflict_names(d)).unwrap_or_default();
                    let what = if what.is_empty() { String::new() } else { format!(": {what}") };
                    b.toast(&format!("{agent}'s change was edited since{what}. Press Undo again to undo it anyway."));
                    b.agent_force.set(Some((audit, glib::monotonic_time())));
                }
                // Undone already (from another window, or by the agent): nothing to say.
                Err(crate::client::Error::Bus(e)) if e.code == "audit.not_undoable" => {
                    b.agent_undo.set_visible(more != AgentChange::None);
                }
                Err(e) => b.toast(&format!("Could not undo {agent}'s change: {e}")),
            }
        });
    }

    /// The layers an `audit.stale` names (`details.conflicts`), as the canvas calls them, with
    /// the fields edited since: "Button (fill), Title and 2 more".
    fn conflict_names(&self, details: &Value) -> String {
        let scene = self.doc.scene.borrow();
        let named: Vec<String> = details["conflicts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| {
                let id = c["id"].as_str()?;
                let name = scene.node(id).map_or_else(|| id.to_string(), display_name);
                let fields: Vec<&str> = c["fields"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                Some(if fields.is_empty() { name } else { format!("{name} ({})", fields.join(", ")) })
            })
            .collect();
        match named.len() {
            0..=3 => named.join(", "),
            n => format!("{} and {} more", named[..3].join(", "), n - 3),
        }
    }

    /// Undo the last step. A drag under way is put back first, as Esc does, so the step's
    /// inverse lands on what the engine holds and the release has nothing left to send (B11).
    pub(super) fn undo(&self) {
        self.cancel_gesture();
        self.end_edit(true);
        self.doc.undo(self.selection.borrow().clone());
    }

    pub(super) fn redo(&self) {
        self.cancel_gesture();
        self.end_edit(true);
        self.doc.redo(self.selection.borrow().clone());
    }

    /// Put back what a gesture under way has previewed and end it.
    pub(super) fn cancel_gesture(&self) {
        if !matches!(*self.gesture.borrow(), Gesture::Idle | Gesture::Pan { .. }) {
            super::tools::cancel(self);
        }
    }

    pub(super) fn delete_selection(&self) {
        let ids = self.selected();
        // A text being typed for the first time is not on the engine yet: its edit drops it.
        // One typed into that was there is sent first, so undo brings back its words (TM-45).
        let editing = self.edit.borrow().as_ref().map(|e| (e.id.clone(), e.created));
        match editing {
            Some((id, true)) if ids.contains(&id) => self.end_edit(false),
            Some((id, false)) if ids.contains(&id) => self.end_edit(true),
            _ => {}
        }
        let ids = self.selected();
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let ids: Vec<String> = ids.into_iter().filter(|id| scene.node(id).is_some_and(|n| n.kind != NodeKind::Page)).collect();
        // A locked layer stays (B16): said so, and the selection kept, when nothing else goes.
        if !ids.is_empty() && edit::unlocked(&scene, ids.clone()).is_empty() {
            drop(scene);
            self.toast("Unlock the layer to delete it");
            return;
        }
        // Connectors attached to what goes let go where they are drawn, rather than dangle.
        let ops = edit::delete(&scene, &placed, &ids);
        drop(scene);
        if self.commit(ops) {
            self.select(Vec::new());
        }
    }

    pub(super) fn duplicate(&self) {
        let ids = self.selected();
        let placed = self.placed();
        let (ops, roots) = edit::duplicate(&mut self.doc.scene.borrow_mut(), &placed, &ids, 16.0);
        if self.commit(ops) {
            self.select(roots);
        }
    }

    /// Mirror the selection left to right (`horizontal`) or top to bottom.
    pub(super) fn flip(&self, horizontal: bool) {
        let placed = self.placed();
        let ops = relay_client::design::flip::flip(&self.doc.scene.borrow(), &placed, &*self.painter, &self.selection.borrow(), horizontal);
        self.commit(ops);
    }

    /// Keep a selected layer's style, to paste on others: the one clicked last, else the
    /// topmost (E-board-14). With it, the fill and corner radius it shows, for a paste onto a
    /// kind whose own default differs (TM-48).
    pub(super) fn copy_style(&self) {
        let last = self.selection.borrow().last().cloned().filter(|id| self.doc.scene.borrow().node(id).is_some());
        let Some(id) = last.or_else(|| self.selected().pop()) else {
            return;
        };
        let scene = self.doc.scene.borrow();
        let style = relay_client::design::flip::style_of(&scene, &id).map(|mut style| {
            if let Some(n) = scene.node(&id) {
                style.insert(SHOWN_FILL.into(), fill_of(n).map_or(Value::Null, Value::from));
                style.insert(SHOWN_RADIUS.into(), json!(shapes::radius(n)));
            }
            style
        });
        drop(scene);
        if style.is_some() {
            *self.style_clip.borrow_mut() = style;
            self.refresh_actions();
            self.toast("Copied the style");
        }
    }

    /// Lay the copied style on the selection.
    pub(super) fn paste_style(&self) {
        let Some(style) = self.style_clip.borrow().clone() else {
            self.toast("Copy a style first (Ctrl+Alt+C)");
            return;
        };
        let scene = self.doc.scene.borrow();
        let mut ops = relay_client::design::flip::paste_style(&scene, &self.selected(), &style);
        keep_shown_defaults(&mut ops, &style, |id| scene.node(id).map(|n| n.kind));
        drop(scene);
        self.commit(ops);
    }

    pub(super) fn copy(&self) {
        if let Some(count) = self.copy_quietly() {
            self.toast(&format!("Copied {count} {}", if count == 1 { "layer" } else { "layers" }));
        }
    }

    /// Copy the selection and take it away.
    pub(super) fn cut(&self) {
        // The words typed so far are what is cut (TM-45).
        self.end_edit(true);
        if let Some(count) = self.copy_quietly() {
            self.delete_selection();
            self.toast(&format!("Cut {count} {}", if count == 1 { "layer" } else { "layers" }));
        }
    }

    /// Put the selection on the clipboard; how many layers that was. It goes twice: as it
    /// draws (`baked`: its style laid under it and its colour names as colours), for another
    /// design, and as stored (`linked`), naming this design's colours and styles, for a paste
    /// back into it.
    fn copy_quietly(&self) -> Option<usize> {
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let linked = edit::gather(&scene, &placed, &self.selection.borrow());
        if linked.is_empty() {
            return None;
        }
        let nodes = edit::baked(&scene, &linked);
        drop(scene);
        let texts: Vec<String> = nodes.iter().filter_map(|n| n.text.clone()).collect();
        let clip = json!({
            "relay_design": 1,
            "design": self.doc.id,
            "nodes": written(&nodes),
            "linked": written(&linked),
        });
        let bytes = serde_json::to_vec(&clip).unwrap_or_default();
        *self.clip.borrow_mut() = nodes;
        CLIP.with(|c| *c.borrow_mut() = clip);
        let providers = [
            gdk::ContentProvider::for_bytes(MIME, &glib::Bytes::from_owned(bytes)),
            gdk::ContentProvider::for_value(&texts.join("\n").to_value()),
        ];
        let union = gdk::ContentProvider::new_union(&providers);
        if let Err(e) = self.canvas.clipboard().set_content(Some(&union)) {
            self.toast(&e.to_string());
            return None;
        }
        Some(self.selected().len())
    }

    pub(super) fn paste(&self) {
        self.paste_to(None);
    }

    /// Paste the clipboard: layers copied on a canvas, a picture, files or words; at page point
    /// `at` (the context menu's), inside the frame under it, or where pastes go.
    pub(super) fn paste_to(&self, at: Option<(f64, f64)>) {
        let clipboard = self.canvas.clipboard();
        let formats = clipboard.formats();
        let weak = self.me.clone();
        if formats.contain_mime_type(MIME) && clipboard.is_local() {
            let clip = CLIP.with(|c| c.borrow().clone());
            self.paste_nodes(self.clip_nodes(&clip), at);
            return;
        }
        if formats.contain_mime_type(MIME) {
            glib::spawn_future_local(async move {
                let Ok((stream, _)) = clipboard.read_future(&[MIME], glib::Priority::DEFAULT).await else {
                    return;
                };
                let mut bytes = Vec::new();
                loop {
                    match stream.read_bytes_future(65_536, glib::Priority::DEFAULT).await {
                        Ok(chunk) if !chunk.is_empty() => bytes.extend_from_slice(&chunk),
                        _ => break,
                    }
                }
                let clip = serde_json::from_slice::<Value>(&bytes).unwrap_or_default();
                if let Some(b) = weak.upgrade() {
                    b.paste_nodes(b.clip_nodes(&clip), at);
                }
            });
            return;
        }
        // Files copied in the file manager: placed as a drop places them (I-16).
        if formats.contains_type(gdk::FileList::static_type()) {
            glib::spawn_future_local(async move {
                let Ok(value) = clipboard.read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT).await else {
                    return;
                };
                let paths: Vec<std::path::PathBuf> =
                    value.get::<gdk::FileList>().map(|f| f.files().iter().filter_map(gio::prelude::FileExt::path).collect()).unwrap_or_default();
                if let Some(b) = weak.upgrade().filter(|_| !paths.is_empty()) {
                    super::assets::place_files(&b, paths, at);
                }
            });
            return;
        }
        if formats.contains_type(gdk::Texture::static_type()) {
            glib::spawn_future_local(async move {
                if let Ok(Some(texture)) = clipboard.read_texture_future().await {
                    if let Some(b) = weak.upgrade() {
                        super::assets::place_texture(&b, texture, at);
                    }
                }
            });
            return;
        }
        glib::spawn_future_local(async move {
            if let Ok(Some(text)) = clipboard.read_text_future().await {
                if let Some(b) = weak.upgrade() {
                    b.paste_text(text.trim_end(), at);
                }
            }
        });
    }

    /// Where pasted or dropped things go: into the one selected frame or group, beside the
    /// selection, or onto the page.
    pub(super) fn paste_target(&self) -> String {
        let scene = self.doc.scene.borrow();
        let selection = self.selection.borrow();
        if let [one] = selection.as_slice() {
            if let Some(n) = scene.node(one) {
                if matches!(n.kind, NodeKind::Frame | NodeKind::Group) {
                    return one.clone();
                }
                if let Some(p) = n.parent.clone() {
                    return p;
                }
            }
        }
        self.page.borrow().clone()
    }

    /// The layers a clipboard holds for this design: as stored, naming its colours and styles,
    /// when they were copied from it and it still holds every name they use; else as they drew.
    fn clip_nodes(&self, clip: &Value) -> Vec<Node> {
        clip_nodes(clip, self.doc.id, self.doc.scene.borrow().styles())
    }

    fn paste_nodes(&self, nodes: Vec<Node>, at: Option<(f64, f64)>) {
        if nodes.is_empty() {
            return;
        }
        let roots: Vec<&Node> = nodes.iter().filter(|n| n.parent.as_deref().is_none_or(|p| !nodes.iter().any(|o| o.id == p))).collect();
        let mut target = match at {
            Some(p) => super::tools::container_at(self, p),
            None => self.paste_target(),
        };
        // Pasting a copied frame with it still selected puts the copy beside it, not inside
        // it (TM-41).
        if let Some(root) = roots.iter().find(|n| n.id == target) {
            target = root.parent.clone().filter(|p| self.doc.scene.borrow().node(p).is_some()).unwrap_or_else(|| self.page.borrow().clone());
        }
        let placed = self.placed();
        let Some(min_x) = roots.iter().map(|n| n.x).reduce(f64::min) else {
            return;
        };
        let min_y = roots.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
        let same = roots.iter().all(|n| n.parent.as_deref() == Some(target.as_str()))
            && roots.iter().all(|n| self.doc.scene.borrow().node(&n.id).is_some());
        let (dx, dy) = if let Some(p) = at {
            // Their top left at the point (E-board-15), in the container's coordinates.
            let origin = self.origin_of(&target);
            let (lx, ly) = shapes::to_local(&self.doc.scene.borrow(), &placed, &target, p.0, p.1);
            ((lx - origin.x - min_x).round(), (ly - origin.y - min_y).round())
        } else if same {
            (16.0, 16.0)
        } else if target == *self.page.borrow() {
            // Centre of the view.
            let (w, h) = self.size.get();
            let max_x = roots.iter().map(|n| n.x + n.w).fold(f64::NEG_INFINITY, f64::max);
            let max_y = roots.iter().map(|n| n.y + n.h).fold(f64::NEG_INFINITY, f64::max);
            let (cx, cy) = self.to_page(w / 2.0, h / 2.0);
            (cx - (min_x + max_x) / 2.0, cy - (min_y + max_y) / 2.0)
        } else {
            let fits = placed.rect(&target).is_some_and(|r| min_x >= 0.0 && min_y >= 0.0 && min_x < r.w && min_y < r.h);
            if fits { (0.0, 0.0) } else { (16.0 - min_x, 16.0 - min_y) }
        };
        let (ops, new_roots) = edit::clone_into(&mut self.doc.scene.borrow_mut(), &nodes, &target, None, dx, dy);
        if self.commit(ops) {
            self.select(new_roots);
        }
    }

    fn paste_text(&self, text: &str, at: Option<(f64, f64)>) {
        // Only spaces or zero-width characters: a text that shows nothing (B42).
        if !relay_client::design::names::visible(text) {
            return;
        }
        // Cut at a text's most, without splitting what is drawn as one character, and said so
        // rather than losing the rest without a word (B102).
        let max = relay_bus::design::MAX_TEXT;
        let (text, cut) = relay_client::design::names::cut_text(text, max);
        if cut {
            self.toast(&format!("Pasted the first {} characters: a text holds at most {max}", text.chars().count()));
        }
        let target = match at {
            Some(p) => super::tools::container_at(self, p),
            None => self.paste_target(),
        };
        let (w, h) = self.size.get();
        let (cx, cy) = at.unwrap_or_else(|| self.to_page(w / 2.0, h / 2.0));
        let (cx, cy) = shapes::to_local(&self.doc.scene.borrow(), &self.placed(), &target, cx, cy);
        let origin = self.origin_of(&target);
        let mut n = Node::new(NodeKind::Text);
        n.id = self.doc.scene.borrow_mut().fresh_id(NodeKind::Text);
        n.parent = Some(target);
        n.text = Some(text.clone());
        n.x = (cx - origin.x).round();
        n.y = (cy - origin.y).round();
        if text.chars().count() > 60 {
            n.w = 360.0;
            n.w_sizing = Some(relay_bus::design::Sizing::Fixed);
        }
        let id = n.id.clone();
        if self.commit(vec![NodeOp::Add { node: n, index: None }]) {
            self.select(vec![id]);
        }
    }

    pub(super) fn group(&self, kind: NodeKind) {
        let ids = self.selected();
        if ids.is_empty() {
            return;
        }
        let placed = self.placed();
        let made = edit::group(&mut self.doc.scene.borrow_mut(), &placed, &ids, kind);
        match made {
            Some((ops, id)) => {
                if self.commit(ops) {
                    self.select(vec![id]);
                }
            }
            None if kind == NodeKind::Group && ids.iter().any(|id| self.doc.scene.borrow().parent(id).is_some_and(|p| edit::lays_out(&self.doc.scene.borrow(), p))) => {
                self.toast("Auto layout places its layers: frame them instead (Ctrl+Alt+G)")
            }
            None => self.toast("Only layers in the same parent can be grouped"),
        }
    }

    pub(super) fn ungroup(&self) {
        let ids = self.selected();
        let placed = self.placed();
        let (ops, kids) = edit::ungroup_all(&self.doc.scene.borrow(), &placed, &ids);
        if !ops.is_empty() && self.commit(ops) {
            self.select(kids);
        }
    }

    pub(super) fn restack(&self, up: bool, all: bool) {
        let ops = edit::restack(&self.doc.scene.borrow(), &self.selection.borrow(), up, all);
        self.commit(ops);
    }

    pub(super) fn toggle_flag(&self, flag: &str) {
        // Hidden or locked, a layer is typed into no more (TM-45).
        self.end_edit(true);
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let on = ids.iter().any(|id| {
            scene.node(id).is_some_and(|n| if flag == "locked" { !n.is_locked() } else { !n.is_hidden() })
        });
        drop(scene);
        let mut set = Map::new();
        set.insert(flag.into(), if on { json!(true) } else { Value::Null });
        self.set_selected(set);
        if on && flag == "hidden" {
            self.select(Vec::new());
        }
    }

    /// What Lock and Hide do to the selection now, as `toggle_flag` turns them: Unlock when
    /// all of it is locked, Show when all of it is hidden.
    pub(super) fn flag_captions(&self) -> (&'static str, &'static str) {
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let nodes: Vec<&Node> = ids.iter().filter_map(|id| scene.node(id)).collect();
        let all = |f: fn(&Node) -> bool| !nodes.is_empty() && nodes.iter().all(|n| f(n));
        (if all(Node::is_locked) { "Unlock" } else { "Lock" }, if all(Node::is_hidden) { "Show" } else { "Hide" })
    }

    pub(super) fn select_all(&self) {
        let scene = self.doc.scene.borrow();
        let scope = self
            .selection
            .borrow()
            .first()
            .and_then(|id| scene.parent(id))
            .map(str::to_string)
            .unwrap_or_else(|| self.page.borrow().clone());
        let ids: Vec<String> = scene
            .children(&scope)
            .iter()
            .filter(|id| scene.node(id).is_some_and(|n| !n.is_hidden() && !n.is_locked()))
            .cloned()
            .collect();
        drop(scene);
        self.select(ids);
    }

    pub(super) fn add_page(&self) {
        let pages = self.doc.scene.borrow().pages().to_vec();
        let taken: Vec<String> = pages.iter().map(|p| self.page_name(p)).collect();
        let id = self.doc.scene.borrow_mut().fresh_id(NodeKind::Page);
        let mut page = Node::new(NodeKind::Page);
        page.id = id.clone();
        page.name = Some(free_name("Page", &taken));
        if self.commit(vec![NodeOp::Add { node: page, index: None }]) {
            self.set_page(&id);
        }
    }

    fn rename_design(&self) {
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        pop.set_parent(&self.title);
        let entry = gtk::Entry::new();
        let shown = relay_bus::design::strip_controls(&self.doc.title(), false).into_owned();
        entry.set_text(&shown);
        entry.set_width_chars(32);
        pop.set_child(Some(&entry));
        // Enter, or a click elsewhere that closes it, renames the design, as the inspector's
        // name field does on leaving it (B88); Esc closes it with the title as it was.
        let cancel = Rc::new(Cell::new(false));
        let keys = gtk::EventControllerKey::new();
        let c = cancel.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                c.set(true);
            }
            glib::Propagation::Proceed
        });
        entry.add_controller(keys);
        let p = pop.downgrade();
        entry.connect_activate(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
        });
        let weak = self.me.clone();
        let e = entry.downgrade();
        pop.connect_closed(move |p| {
            let title = e.upgrade().filter(|_| !cancel.replace(true)).and_then(|e| title_set(&e.text(), &shown));
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
            let (Some(b), Some(title)) = (weak.upgrade(), title) else {
                return;
            };
            let Some(ui) = b.ui.upgrade() else {
                return;
            };
            let id = b.doc.id;
            glib::spawn_future_local(async move {
                match ui.call("design.update", json!({"design_id": id, "title": title})).await {
                    Ok(v) => {
                        // Our own write: the doc keeps its rev when it is the one write since,
                        // rather than read the whole design again (B83).
                        if let Ok(row) = serde_json::from_value(v) {
                            b.doc.adopt(row);
                        }
                        b.design_changed();
                        super::gallery_changed(&ui);
                    }
                    Err(e) => b.toast(&e.to_string()),
                }
            });
        });
        pop.popup();
        entry.grab_focus();
    }
}

/// The title a rename sends for what was typed, against the title shown: trimmed, without
/// control characters (a pasted line break or NUL, which the engine refuses, B43); `None` when
/// it is empty or unchanged.
fn title_set(typed: &str, shown: &str) -> Option<String> {
    let plain = relay_bus::design::strip_controls(typed, false);
    let title = plain.trim();
    (!title.is_empty() && title != shown.trim()).then(|| title.to_string())
}

thread_local! {
    /// The nodes last copied in this process, for a paste whose clipboard is our own.
    static CLIP: RefCell<Value> = const { RefCell::new(Value::Null) };
    /// Whether the inspector and the pages and layers panel show, kept for the session from
    /// one design (and window) to the next.
    static INSPECTOR_ON: Cell<bool> = const { Cell::new(true) };
    static LAYERS_ON: Cell<bool> = const { Cell::new(false) };
    /// Whether the agent panel shows, the same way.
    static AGENT_PANEL_ON: Cell<bool> = const { Cell::new(false) };
}

/// `RELAY_DESIGN_TRACE` set: print how long layout, content rebuilds and draws take.
fn trace() -> bool {
    thread_local! {
        static ON: bool = std::env::var_os("RELAY_DESIGN_TRACE").is_some();
    }
    ON.with(|on| *on)
}

/// The frame sizes the frame key offers: name, width, height.
pub const FRAME_PRESETS: [(&str, u32, u32); 8] = [
    ("Phone", 390, 844),
    ("Phone large", 430, 932),
    ("Tablet", 834, 1194),
    ("Laptop", 1280, 832),
    ("Desktop", 1440, 1024),
    ("Slide 16:9", 1920, 1080),
    ("Social post", 1080, 1080),
    ("Watch", 198, 242),
];

/// How long an agent's change glows, in microseconds.
const FLASH_US: i64 = 1_600_000;

/// Keys over the canvas take no keyboard focus when clicked, so the canvas keeps its shortcuts
/// (V, Delete, Ctrl+Z…) right after one is used, as in other design tools. A check button is
/// no button in GTK 4, so it is named too.
pub fn keep_canvas_focus(root: &gtk::Widget) {
    let mut stack = vec![root.clone()];
    while let Some(w) = stack.pop() {
        if w.is::<gtk::Button>() || w.is::<gtk::MenuButton>() || w.is::<gtk::CheckButton>() {
            w.set_focus_on_click(false);
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            stack.push(c);
        }
    }
}

/// The corners and star of a polygon tool.
pub fn polygon_of(tool: Tool) -> Option<(u8, Option<f64>)> {
    match tool {
        Tool::Triangle => Some((3, None)),
        Tool::Diamond => Some((4, None)),
        Tool::Hexagon => Some((6, None)),
        Tool::Star => Some((5, Some(0.4))),
        _ => None,
    }
}

pub fn handle_point(s: &Rect, h: Handle) -> (f64, f64) {
    let x = if h.left { s.x } else if h.right { s.right() } else { s.cx() };
    let y = if h.top { s.y } else if h.bottom { s.bottom() } else { s.cy() };
    (x, y)
}

/// A size as the pill shows it: up to two decimals, as the number fields show it (B103).
pub fn fmt(v: f64) -> String {
    relay_client::design::number::fmt(v)
}

/// A round colour key; `None` shows the automatic (checkered) swatch.
pub fn swatch_button(color: Option<&str>, tooltip: &str) -> gtk::Button {
    let swatch = gtk::Button::new();
    swatch.add_css_class("design-swatch");
    swatch.set_tooltip_text(Some(tooltip));
    let dot = gtk::DrawingArea::new();
    dot.set_content_width(16);
    dot.set_content_height(16);
    // A transparent colour shows as none: a slashed, empty swatch.
    let color = color.filter(|c| relay_bus::design::parse_color(c).is_some_and(|c| c[3] > 0)).map(str::to_string);
    dot.set_draw_func(move |_, cr, w, h| {
        // A round swatch in the middle of whatever room the key is given.
        let r = (w.min(h) as f64) / 2.0;
        let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
        cr.arc(cx, cy, r - 0.5, 0.0, std::f64::consts::TAU);
        match &color {
            Some(c) => {
                let c = rgba(c);
                cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64);
                let _ = cr.fill_preserve();
            }
            None => {
                cr.set_source_rgba(0.5, 0.5, 0.5, 0.35);
                let _ = cr.fill_preserve();
            }
        }
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.25);
        cr.set_line_width(1.0);
        let _ = cr.stroke();
        if color.is_none() {
            let k = (r - 2.0) * std::f64::consts::FRAC_1_SQRT_2;
            cr.move_to(cx - k, cy + k);
            cr.line_to(cx + k, cy - k);
            cr.set_source_rgba(0.9, 0.3, 0.3, 0.9);
            cr.set_line_width(1.5);
            let _ = cr.stroke();
        }
    });
    swatch.set_child(Some(&dot));
    swatch
}

/// What a node of `kind` is called on the canvas (`names::kind_caption`).
pub fn kind_caption(kind: NodeKind) -> &'static str {
    relay_client::design::names::kind_caption(kind)
}

pub fn kind_glyph(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Page => "page",
        NodeKind::Frame => "frame",
        NodeKind::Group => "group",
        NodeKind::Rect => "rect",
        NodeKind::Ellipse => "ellipse",
        NodeKind::Line => "line",
        NodeKind::Text => "text",
        NodeKind::Sticky => "sticky",
        NodeKind::Path => "pen",
        NodeKind::Image => "image",
        NodeKind::Polygon => "triangle",
        NodeKind::Icon => "icon",
    }
}

/// `base` with the first number no name in `taken` has: "Page 1" again once Page 1 is gone,
/// not a second "Page 2".
pub fn free_name(base: &str, taken: &[String]) -> String {
    (1..).map(|n| format!("{base} {n}")).find(|name| !taken.contains(name)).unwrap_or_default()
}

/// A node's name as the breadcrumb, Ask and export show it, never blank: its name when it shows
/// anything, else its text's first line that does, else what it draws (B42, E-board-24).
pub fn display_name(n: &Node) -> String {
    relay_client::design::names::display_name(n)
}

/// Whether a line is reshaped by dragging its ends: a connector, or a straight open line. A
/// vector path (more than two points) or a closed shape gets the box's handles instead, which
/// scale its points (I-16).
pub fn ends_dragged(n: &Node) -> bool {
    n.kind == NodeKind::Line
        && (n.from.is_some() || n.to.is_some() || (n.closed != Some(true) && n.points.as_ref().is_none_or(|p| p.len() <= 2)))
}

/// A tool's glyph for its key; locked on, with a small lock in its corner.
fn tool_glyph(tool: Tool, locked: bool) -> gtk::Widget {
    let glyph = glyphs::image(tool.glyph(), 16);
    if !locked {
        return glyph.upcast();
    }
    let stack = gtk::Overlay::new();
    stack.set_child(Some(&glyph));
    let lock = glyphs::image("lock", 8);
    lock.set_halign(gtk::Align::End);
    lock.set_valign(gtk::Align::End);
    stack.add_overlay(&lock);
    stack.upcast()
}

/// A drag's last update: its screen point, its offset from the press, and the modifiers held.
pub type DragAt = (f64, f64, f64, f64, gdk::ModifierType);

/// What a board shows: what `Board::state` takes and `Board::restore` puts back.
#[derive(Debug, Clone)]
pub struct BoardState {
    pub page: String,
    pub cam: Camera,
    pub cams: HashMap<String, Camera>,
    pub selection: Vec<String>,
    pub layers_open: bool,
    pub inspector_open: bool,
}

/// Where each design's page and camera are kept when its board closes, by design id.
const VIEWS: &str = "native.design.views";

/// The setting a board's page and camera are kept as.
fn view_value(page: &str, cam: Camera) -> Value {
    json!({"page": page, "x": cam.x, "y": cam.y, "zoom": cam.zoom})
}

/// A kept page and camera, when it reads.
fn read_view(v: &Value) -> Option<(String, Camera)> {
    let page = v["page"].as_str()?.to_string();
    let (x, y, zoom) = (v["x"].as_f64()?, v["y"].as_f64()?, v["zoom"].as_f64()?);
    (x.is_finite() && y.is_finite() && zoom.is_finite() && zoom > 0.0).then_some((page, Camera { x, y, zoom }))
}

impl Drop for Board {
    /// Keep the page and camera it was left at, for the next time the design opens (I-15).
    fn drop(&mut self) {
        // The agent panel's terminal went back to the wall as the board was closed
        // (`close_agent_panel`); handing it back here would lay out a wall the app may be
        // tearing down as it quits.
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if self.size.get().0 <= 0.0 {
            return;
        }
        let value = view_value(&self.page.borrow(), self.cam.get());
        let path = format!("{VIEWS}.{}", self.doc.id);
        glib::spawn_future_local(async move {
            let _ = ui.call("settings.set", json!({"path": path, "value": value})).await;
        });
    }
}

/// The text editor's place over screen box `s` (the words' box at the zoom): whole pixels from
/// the one at or before its corner, with the fraction before the words as margins so they start
/// where the canvas draws them; a wrapping text exactly as wide as the canvas wraps it, a
/// hugging one with room for the caret. `(x, y, w, h, left, top)`.
fn editor_box(s: &Rect, wraps: bool) -> (i32, i32, i32, i32, i32, i32) {
    let (x0, y0) = (s.x.floor(), s.y.floor());
    let (left, top) = ((s.x - x0).round() as i32, (s.y - y0).round() as i32);
    let w = if wraps { left + s.w.round().max(4.0) as i32 } else { ((s.right()).ceil() - x0).max(4.0) as i32 + 2 };
    let h = ((s.bottom()).ceil() - y0).max(4.0) as i32 + top;
    (x0 as i32, y0 as i32, w, h, left, top)
}

/// The keys a copied style carries besides its fields, which `flip::paste_style` passes over:
/// the fill and corner radius its layer showed.
const SHOWN_FILL: &str = "_fill";
const SHOWN_RADIUS: &str = "_radius";

/// A pasted style's `fill: null` or `radius: null` means "its kind's default", which on another
/// kind is another look (a white frame's style turning a rect grey, a rect's square corners
/// rounding on a sticky): there it writes what the copied layer showed (TM-48). A style with
/// a named style of its own keeps the nulls, which let that style show.
fn keep_shown_defaults(ops: &mut [NodeOp], style: &Map<String, Value>, kind_of: impl Fn(&str) -> Option<NodeKind>) {
    if !style.get("style").unwrap_or(&Value::Null).is_null() {
        return;
    }
    let from: Option<NodeKind> = style.get("kind").cloned().and_then(|k| serde_json::from_value(k).ok());
    let Some(from) = from else {
        return;
    };
    for op in ops.iter_mut() {
        let NodeOp::Update { id, set } = op else {
            continue;
        };
        let Some(kind) = kind_of(id).filter(|k| *k != from) else {
            continue;
        };
        let bare = |k: NodeKind| fill_of(&Node::new(k));
        if set.get("fill").is_some_and(Value::is_null) && bare(kind) != bare(from) {
            if let Some(shown) = style.get(SHOWN_FILL).filter(|v| v.is_string()) {
                set.insert("fill".into(), shown.clone());
            }
        }
        if set.get("radius").is_some_and(Value::is_null) && shapes::default_radius(kind) != shapes::default_radius(from) {
            if let Some(shown) = style.get(SHOWN_RADIUS).and_then(Value::as_f64) {
                set.insert("radius".into(), json!(shown));
            }
        }
    }
}

/// Nodes as JSON, as the engine writes them (`Node::to_map`: what this build cannot read kept).
fn written(nodes: &[Node]) -> Value {
    Value::Array(nodes.iter().map(|n| Value::Object(n.to_map())).collect())
}

/// The layers clipboard JSON `clip` holds for design `design`: its `linked` copy, as stored,
/// when it was copied from that design and every colour and style its layers name is there
/// still; else its `nodes`, as they drew. Each read as the engine reads a node.
fn clip_nodes(clip: &Value, design: i64, styles: &relay_bus::design::Styles) -> Vec<Node> {
    let read = |v: &Value| -> Option<Vec<Node>> { v.as_array()?.iter().map(|n| Node::read(n.clone()).ok()).collect() };
    let linked = (clip["design"].as_i64() == Some(design)).then(|| read(&clip["linked"])).flatten();
    match linked.filter(|nodes| nodes.iter().all(|n| styles.check(n).is_ok())) {
        Some(nodes) => nodes,
        None => read(&clip["nodes"]).unwrap_or_default(),
    }
}

/// Whether layers `ids` show resize handles: one of them is neither locked (B16) nor a group,
/// whose box is its children's and whose own size nothing reads (B90).
pub(super) fn handled(scene: &Scene, ids: &[String]) -> bool {
    ids.iter().any(|id| scene.node(id).is_some_and(|n| !n.is_locked() && n.kind != NodeKind::Group))
}

/// The marks of `n`'s pins, placed at `r` in a frame placed at `pr` (`px` page units to a screen
/// pixel), as segments: from each edge it is pinned by to the frame's along its middle (both
/// for left and right or scale), and an 8 px tick across the frame's middle line for center.
fn pin_lines(n: &Node, r: &Rect, pr: &Rect, px: f64) -> Vec<((f64, f64), (f64, f64))> {
    use relay_bus::design::Pin;
    let (pin_x, pin_y) = n.pins();
    let tick = 4.0 * px;
    let mut lines = Vec::new();
    match pin_x {
        Pin::Center => lines.push(((pr.cx(), r.cy() - tick), (pr.cx(), r.cy() + tick))),
        pin => {
            if pin != Pin::End {
                lines.push(((pr.x, r.cy()), (r.x, r.cy())));
            }
            if pin != Pin::Start {
                lines.push(((r.right(), r.cy()), (pr.right(), r.cy())));
            }
        }
    }
    match pin_y {
        Pin::Center => lines.push(((r.cx() - tick, pr.cy()), (r.cx() + tick, pr.cy()))),
        pin => {
            if pin != Pin::End {
                lines.push(((r.cx(), pr.y), (r.cx(), r.y)));
            }
            if pin != Pin::Start {
                lines.push(((r.cx(), r.bottom()), (r.cx(), pr.bottom())));
            }
        }
    }
    lines
}

/// Where a fit frames what it shows on a `w` by `h` canvas: below the dock along the top, which
/// keeps the frame names in view, and between the layers panel's right edge `left` and the
/// inspector's left edge `right` where they show; the whole width when that leaves too little.
fn fit_area(w: f64, h: f64, left: Option<f64>, right: Option<f64>) -> Rect {
    const TOP: f64 = 76.0;
    let x0 = left.unwrap_or(0.0).clamp(0.0, w);
    let x1 = right.unwrap_or(w).clamp(x0, w);
    let (x0, x1) = if x1 - x0 < 240.0 { (0.0, w) } else { (x0, x1) };
    Rect::new(x0, TOP, (x1 - x0).max(1.0), (h - TOP).max(1.0))
}

/// The insertion bar, in page coordinates (`px` page units to a screen pixel), for a place in
/// frame `pr` laid out `mode`: 2 screen px centred just before child `next` at that place, or
/// just after the `last` laid out when there is none, or inside the empty frame's start. A row's
/// and a wrap's bar stand upright, a wrap's at its child's height; a column's lies across.
fn insertion_bar(mode: Layout, pr: &Rect, next: Option<Rect>, last: Option<Rect>, px: f64) -> Rect {
    let across = matches!(mode, Layout::Row | Layout::Wrap);
    let at = match (next, last) {
        (Some(k), _) => if across { k.x - 2.0 } else { k.y - 2.0 },
        (None, Some(l)) => if across { l.right() + 2.0 } else { l.bottom() + 2.0 },
        (None, None) => if across { pr.x + 8.0 } else { pr.y + 8.0 },
    };
    let beside = next.or(last).filter(|_| mode == Layout::Wrap);
    match (across, beside) {
        (true, Some(k)) => Rect::new(at - px, k.y, 2.0 * px, k.h),
        (true, None) => Rect::new(at - px, pr.y + 4.0, 2.0 * px, pr.h - 8.0),
        _ => Rect::new(pr.x + 4.0, at - px, pr.w - 8.0, 2.0 * px),
    }
}

/// The name above a top-level frame, with a mark for the way it scrolls: ⇆ across, ⇅ down.
fn frame_label(n: &Node) -> String {
    let mark = match n.scroll {
        Some(Scroll::X) => " ⇆",
        Some(Scroll::Y) => " ⇅",
        Some(Scroll::Both) => " ⇆⇅",
        None => "",
    };
    format!("{}{mark}", display_name(n))
}

/// The wheel's `(dx, dy)` as a frame that scrolls `scroll` takes it: one that scrolls only
/// across takes the wheel's turn (down) across, one that scrolls only down takes a sideways
/// swipe down.
pub(super) fn scroll_delta(scroll: Scroll, dx: f64, dy: f64) -> (f64, f64) {
    let main = if dx.abs() > dy.abs() { dx } else { dy };
    match scroll {
        Scroll::X => (main, 0.0),
        Scroll::Y => (0.0, main),
        Scroll::Both => (dx, dy),
    }
}

/// Where a frame placed at `r` and scrolled as `s` is scrolled to by `d` more: along the axes it
/// scrolls, from its start to as far as its content passes its box.
fn scroll_to(r: &Rect, s: &Scrolled, scroll: Scroll, d: (f64, f64)) -> (f64, f64) {
    let along = |on: bool, at: f64, by: f64, room: f64| if on { (at + by).clamp(0.0, room.max(0.0)) } else { 0.0 };
    (along(scroll.x(), s.offset.0, d.0, s.content.w - r.w), along(scroll.y(), s.offset.1, d.1, s.content.h - r.h))
}

/// The scrollbar thumbs of a frame placed at `r` and scrolled as `s`, in page coordinates
/// (`px` page units to a screen pixel): 4 px bars 2 px inside the edges it scrolls past, only
/// where its content overflows; each as long against the edge as the box is against the content,
/// and as far along as it is scrolled.
fn scroll_bars(r: &Rect, s: &Scrolled, scroll: Scroll, px: f64) -> Vec<Rect> {
    let (thick, inset) = (4.0 * px, 2.0 * px);
    let mut bars = Vec::new();
    if scroll.x() && s.content.w > r.w + 0.5 {
        let len = r.w * r.w / s.content.w;
        bars.push(Rect::new(r.x + s.offset.0 / s.content.w * r.w, r.bottom() - inset - thick, len, thick));
    }
    if scroll.y() && s.content.h > r.h + 0.5 {
        let len = r.h * r.h / s.content.h;
        bars.push(Rect::new(r.right() - inset - thick, r.y + s.offset.1 / s.content.h * r.h, thick, len));
    }
    bars
}

/// Where layer `id` shows on the page, as a box: `shapes::shown_rect` (its drawing, turned by its
/// own turn) carried out through each turned frame it is in.
pub(super) fn shown_box(scene: &Scene, placed: &Placed, id: &str) -> Option<Rect> {
    let r = shapes::shown_rect(scene, placed, id)?;
    Some(shapes::turned_ancestors(scene, placed, id).iter().rev().fold(r, |b, &(cx, cy, d)| shapes::rotated_bounds_about(&b, d, cx, cy)))
}

/// Undo agent's audit pages: rows a page, and how many pages it reads back.
const AGENT_PAGE: u32 = 500;
const AGENT_PAGES: usize = 5;
/// How long a second press undoes an agent's change edited since anyway, in microseconds.
const FORCE_US: i64 = 10_000_000;

/// What Undo agent finds of an agent's changes to a design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentChange {
    /// The newest one standing, by audit row: it can be undone.
    Undoable(i64),
    /// The newest one standing kept no undo: it was too large to (64 KiB).
    TooLarge,
    None,
}

/// The design an audited `design.apply` row wrote: its payload's, else its undo's, else its
/// answer's (a payload too large to keep is stored as none).
fn row_design(r: &Value) -> Option<i64> {
    r["payload"]["design_id"]
        .as_i64()
        .or_else(|| r["undo_op"]["payload"]["design_id"].as_i64())
        .or_else(|| r["result_summary"]["design_id"].as_i64())
        .or_else(|| r["result_summary"]["design"]["id"].as_i64())
}

/// Among audit `rows`, the newest change of `design` still standing (applied, not undone), if
/// any is there: undoable when it kept its undo, too large otherwise. Older changes are never
/// offered in place of a newer one that cannot be undone (TM-11).
pub(super) fn pick_agent_change(rows: &[Value], design: i64) -> Option<AgentChange> {
    let newest = rows
        .iter()
        .filter(|r| r["kind"] == "ok" && r["undone_by"].is_null() && row_design(r) == Some(design) && changed_something(r))
        .max_by_key(|r| r["id"].as_i64().unwrap_or(0))?;
    Some(match (newest["undo_op"].is_object(), newest["id"].as_i64()) {
        (true, Some(id)) => AgentChange::Undoable(id),
        _ => AgentChange::TooLarge,
    })
}

/// Whether an audited apply changed the design: a list that changed nothing is audited too,
/// with no undo and its answer's `rev` its `prev_rev`, and is no change to offer.
fn changed_something(r: &Value) -> bool {
    let answer = &r["result_summary"];
    r["undo_op"].is_object() || answer["rev"].is_null() || answer["rev"] != answer["prev_rev"]
}

/// What Undo agent says of a change it cannot undo for its size.
fn too_large(agent: &str) -> String {
    format!("{agent}'s last change is too large to undo here")
}

/// The frame names' grey on a ground of `luminance`: a dark one on a light page, the light one
/// on a dark page, switching where the two read alike (E-board-1); 4.5:1 or better on white
/// and on the canvas's own dark.
fn label_ink(luminance: f64) -> gdk::RGBA {
    if luminance > 0.19 { gdk::RGBA::new(0.33, 0.32, 0.3, 1.0) } else { gdk::RGBA::new(0.62, 0.6, 0.57, 1.0) }
}

/// How long a toast stays up for `text`: longer for more words, by characters rather than bytes
/// (a CJK message is not three times as long), and never past about six seconds (B75).
pub fn toast_ms(text: &str) -> u64 {
    2600 + text.chars().count().min(120) as u64 * 30
}

/// A menu entry for `design.{action}` that shows its shortcut (`<Control>c`), as the notes'
/// menus do. Only shown: the board's own keys run it.
pub(super) fn menu_item(caption: &str, action: &str, accel: Option<&str>) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(caption), Some(&format!("design.{action}")));
    if let Some(accel) = accel {
        item.set_attribute_value("accel", Some(&accel.to_variant()));
    }
    item
}

/// The Play key's tooltip while Play is off.
const PLAY_TIP: &str = "Play the motion on this page (Ctrl+P)";

/// Play under way: the clock, the frame shown and its poses. Never written to the design.
struct Play {
    /// The frame clock's callback while it runs; `None` while paused.
    tick: Option<gtk::TickCallbackId>,
    timing: Timing,
    /// The time shown, in ms since every clock started.
    t_ms: f64,
    /// The poses of the frame shown (filled as it draws) and the page's clocks.
    posing: Rc<Posing>,
    /// A shown picture animates itself: Play then runs until it is stopped.
    pictures: bool,
    /// Paused at the end of a motion that does not loop.
    ended: bool,
    /// The time the time bar's words last showed.
    labelled: f64,
}

/// Where Play is in time, from the frame clock's frame times (µs): GTK-free, so its rule is
/// tested alone. While running, the time shown is `base_ms` plus the time since the first frame
/// after it started; paused, `base_ms`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Timing {
    base_ms: f64,
    from_us: Option<i64>,
    running: bool,
}

impl Timing {
    /// The time at a frame at `frame_us`.
    fn at(&mut self, frame_us: i64) -> f64 {
        if !self.running {
            return self.base_ms;
        }
        let from = *self.from_us.get_or_insert(frame_us);
        self.base_ms + (frame_us - from).max(0) as f64 / 1000.0
    }

    /// Hold at `at_ms`.
    fn pause(&mut self, at_ms: f64) {
        *self = Timing { base_ms: at_ms, from_us: None, running: false };
    }

    /// Run on from where it holds, counting from the next frame.
    fn resume(&mut self) {
        self.running = true;
        self.from_us = None;
    }

    /// Show `ms` (and run on from it, if running).
    fn seek(&mut self, ms: f64) {
        self.base_ms = ms;
        self.from_us = None;
    }
}

/// The span Play runs over: its longest clock, and whether any clock loops (then it never ends,
/// and the time shown starts again at the end).
fn play_span(clocks: &Clocks) -> (f64, bool) {
    (clocks.span().0, clocks.clocks().iter().any(|c| c.looped))
}

/// Whether a frame at `t_ms` ends Play: no clock loops, no shown picture animates itself, and
/// the span has run.
fn ends(t_ms: f64, span: f64, looped: bool, pictures: bool) -> bool {
    !looped && !pictures && span > 0.0 && t_ms >= span
}

/// The time bar's words: "1.20 / 3.00 s", or the time alone when only pictures move.
fn time_text(t_ms: f64, span_ms: f64) -> String {
    if span_ms > 0.0 {
        format!("{:.2} / {:.2} s", t_ms / 1000.0, span_ms / 1000.0)
    } else {
        format!("{:.2} s", t_ms / 1000.0)
    }
}

/// The bar under the canvas while Play runs: start again, pause or play, the time, a scrubber
/// and stop.
struct TimeBar {
    root: gtk::Box,
    restart: gtk::Button,
    pause: gtk::Button,
    time: gtk::Label,
    scrub: gtk::Scale,
    stop: gtk::Button,
}

fn time_bar() -> TimeBar {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    root.add_css_class("design-timebar");
    root.set_widget_name("design-timebar");
    root.set_halign(gtk::Align::Center);
    root.set_valign(gtk::Align::End);
    root.set_margin_bottom(12);
    root.set_visible(false);
    let restart = crate::app::icon_button("resume", "Start again");
    let pause = crate::app::icon_button("pause", "Pause");
    let time = label("0.00 s", "design-timebar-time");
    time.set_xalign(0.5);
    let scrub = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 10.0);
    scrub.set_draw_value(false);
    scrub.set_hexpand(true);
    scrub.set_tooltip_text(Some("Drag to a moment of the motion"));
    let stop = crate::app::icon_button("stop", "Stop playing (Esc)");
    for w in [restart.upcast_ref::<gtk::Widget>(), pause.upcast_ref(), time.upcast_ref(), scrub.upcast_ref(), stop.upcast_ref()] {
        w.set_valign(gtk::Align::Center);
        root.append(w);
    }
    TimeBar { root, restart, pause, time, scrub, stop }
}

/// "Export selection as" PNG, SVG or PDF, or its motion as an animation or frames, for the
/// menus that offer it.
pub(super) fn export_menu() -> gio::Menu {
    let section = gio::Menu::new();
    section.append(Some("Export selection as PNG (2×)…"), Some("design.export"));
    section.append(Some("Export selection as SVG…"), Some("design.export-svg"));
    section.append(Some("Export selection as PDF…"), Some("design.export-pdf"));
    section.append(Some("Export selection as animation (APNG)…"), Some("design.export-apng"));
    section.append(Some("Export selection as PNG frames…"), Some("design.export-frames"));
    section
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, design: i64, undo: bool) -> Value {
        let undo_op = if undo { json!({"op": "design.apply", "payload": {"design_id": design}}) } else { Value::Null };
        json!({"id": id, "kind": "ok", "undone_by": null, "payload": {"design_id": design}, "undo_op": undo_op,
               "result_summary": {"design_id": design, "rev": id + 1, "prev_rev": id}})
    }

    #[test]
    fn undo_agent_takes_the_newest_change_or_says_it_is_too_large() {
        let rows = vec![row(3, 7, true), row(9, 8, true), row(5, 7, true)];
        assert_eq!(pick_agent_change(&rows, 7), Some(AgentChange::Undoable(5)));
        assert_eq!(pick_agent_change(&rows, 1), None, "none of this design on the page: read further");
        // The newest kept no undo: an older one is not undone in its place (TM-11).
        let rows = vec![row(3, 7, true), row(6, 7, false)];
        assert_eq!(pick_agent_change(&rows, 7), Some(AgentChange::TooLarge));
        // Undone, refused, or a list that changed nothing do not count.
        let mut undone = row(8, 7, true);
        undone["undone_by"] = json!(12);
        let mut refused = row(9, 7, true);
        refused["kind"] = json!("error");
        let mut nothing = row(10, 7, false);
        nothing["result_summary"]["rev"] = json!(10);
        assert_eq!(pick_agent_change(&[row(3, 7, true), undone, refused, nothing], 7), Some(AgentChange::Undoable(3)));
        // A payload too large to keep names its design in the undo.
        let mut big = row(4, 7, true);
        big["payload"] = Value::Null;
        assert_eq!(pick_agent_change(&[big], 7), Some(AgentChange::Undoable(4)));
    }

    fn rect(id: &str, fill: &str) -> Node {
        let mut n = Node::new(NodeKind::Rect);
        n.id = id.into();
        n.parent = Some("p1".into());
        n.fill = Some(fill.into());
        n
    }

    #[test]
    fn a_paste_names_this_designs_colours_only_when_it_holds_them() {
        let clip = json!({"relay_design": 1, "design": 4, "nodes": written(&[rect("a", "#FF0000")]), "linked": written(&[rect("a", "$brand")])});
        let mut styles = relay_bus::design::Styles::default();
        assert_eq!(clip_nodes(&clip, 4, &styles)[0].fill.as_deref(), Some("#FF0000"), "the name is gone: as it drew");
        styles.colors.insert("brand".into(), "#FF0000".into());
        assert_eq!(clip_nodes(&clip, 4, &styles)[0].fill.as_deref(), Some("$brand"), "same design: as stored");
        assert_eq!(clip_nodes(&clip, 5, &styles)[0].fill.as_deref(), Some("#FF0000"), "another design: as it drew");
        let old = json!({"relay_design": 1, "nodes": [{"id": "a", "kind": "rect", "parent": "p1"}]});
        assert_eq!(clip_nodes(&old, 4, &styles).len(), 1, "an older clipboard reads");
    }

    #[test]
    fn a_pasted_style_keeps_the_fill_and_corners_its_layer_showed() {
        let mut frame = Node::new(NodeKind::Frame);
        frame.id = "f".into();
        let mut style = relay_client::design::flip::style_of(&Scene::new(vec![frame.clone()]), "f").unwrap();
        style.insert(SHOWN_FILL.into(), json!("#FFFFFF"));
        style.insert(SHOWN_RADIUS.into(), json!(0.0));
        let set = |pairs: &[(&str, Value)]| pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<Map<String, Value>>();
        let mut ops = vec![
            NodeOp::Update { id: "r".into(), set: set(&[("fill", Value::Null)]) },
            NodeOp::Update { id: "s".into(), set: set(&[("radius", Value::Null)]) },
            NodeOp::Update { id: "g".into(), set: set(&[("fill", Value::Null)]) },
        ];
        let kinds = |id: &str| Some(match id { "r" => NodeKind::Rect, "s" => NodeKind::Sticky, _ => NodeKind::Frame });
        keep_shown_defaults(&mut ops, &style, kinds);
        let fill = |op: &NodeOp| match op { NodeOp::Update { set, .. } => set.clone(), _ => Map::new() };
        assert_eq!(fill(&ops[0])["fill"], json!("#FFFFFF"), "a white frame's style on a rect is white");
        assert_eq!(fill(&ops[1])["radius"], json!(0.0), "square corners stay square on a sticky");
        assert_eq!(fill(&ops[2])["fill"], Value::Null, "the same kind keeps its default");
    }

    #[test]
    fn the_editor_wraps_where_the_canvas_does() {
        // A paragraph wrapped 120.4 px wide from x 10.6: the words start at 11 (a margin of 1
        // on the pixel at 10), and wrap 120 px on.
        assert_eq!(editor_box(&Rect::new(10.6, 20.2, 120.4, 40.0), true), (10, 20, 121, 41, 1, 0));
        // Hugging: room for the caret past the last letter.
        assert_eq!(editor_box(&Rect::new(10.0, 20.0, 50.0, 18.0), false), (10, 20, 52, 18, 0, 0));
    }

    #[test]
    fn a_kept_view_reads_back() {
        let cam = Camera { x: -120.5, y: 40.0, zoom: 2.0 };
        let (page, back) = read_view(&view_value("p2", cam)).unwrap();
        assert_eq!(page, "p2");
        assert_eq!((back.x, back.y, back.zoom), (cam.x, cam.y, cam.zoom));
        assert!(read_view(&json!({"page": "p2", "x": 0, "y": 0, "zoom": 0})).is_none());
        assert!(read_view(&Value::Null).is_none());
    }

    #[test]
    fn toast_time_counts_characters_and_is_capped() {
        assert_eq!(toast_ms("中文消息"), toast_ms("abcd"));
        assert!(toast_ms(&"x".repeat(5000)) <= 6200);
        assert!(toast_ms("a longer message") > toast_ms("short"));
    }

    #[test]
    fn a_title_is_sent_trimmed_plain_and_only_when_changed() {
        assert_eq!(title_set("Home", "Home"), None);
        assert_eq!(title_set("  Home ", "Home"), None);
        assert_eq!(title_set("   ", "Home"), None);
        assert_eq!(title_set("\u{0}", "Home"), None);
        assert_eq!(title_set(" Hero\u{0}x\n", "Home"), Some("Herox".into()));
        assert_eq!(title_set("Landing", "Home"), Some("Landing".into()));
    }

    #[test]
    fn alt_wheel_scrolls_a_frame_within_its_content() {
        // A 350 px carousel holding 804 px of cards.
        let r = Rect::new(10.0, 20.0, 350.0, 200.0);
        let s = Scrolled { offset: (0.0, 0.0), content: Rect::new(10.0, 20.0, 804.0, 200.0) };
        assert_eq!(scroll_to(&r, &s, Scroll::X, (100.0, 0.0)), (100.0, 0.0));
        assert_eq!(scroll_to(&r, &s, Scroll::X, (-40.0, 0.0)), (0.0, 0.0), "not before its start");
        assert_eq!(scroll_to(&r, &s, Scroll::X, (9_000.0, 0.0)), (454.0, 0.0), "not past its last card");
        assert_eq!(scroll_to(&r, &s, Scroll::Both, (0.0, 30.0)), (0.0, 0.0), "nothing below to scroll to");
        assert_eq!(scroll_to(&r, &s, Scroll::Y, (100.0, 0.0)), (0.0, 0.0), "a feed does not scroll across");
        // The wheel turned down scrolls a carousel across; a feed takes a sideways swipe down.
        assert_eq!(scroll_delta(Scroll::X, 0.0, 48.0), (48.0, 0.0));
        assert_eq!(scroll_delta(Scroll::Y, 30.0, 2.0), (0.0, 30.0));
        assert_eq!(scroll_delta(Scroll::Both, 3.0, 4.0), (3.0, 4.0));
    }

    #[test]
    fn a_scrollbar_is_as_long_as_the_box_against_its_content() {
        let r = Rect::new(0.0, 0.0, 400.0, 300.0);
        let s = Scrolled { offset: (200.0, 0.0), content: Rect::new(0.0, 0.0, 800.0, 300.0) };
        let bars = scroll_bars(&r, &s, Scroll::Both, 1.0);
        assert_eq!(bars.len(), 1, "only across: the content fits down");
        let bar = bars[0];
        assert_eq!((bar.w, bar.h), (200.0, 4.0), "400² / 800");
        assert_eq!(bar.x, 100.0, "200 of 800 along a 400 box");
        assert_eq!(bar.bottom(), 298.0, "2 px inside the bottom edge");
        assert_eq!(scroll_bars(&r, &s, Scroll::X, 0.5)[0].h, 2.0, "4 screen px at 200%");
        let fits = Scrolled { offset: (0.0, 0.0), content: r };
        assert!(scroll_bars(&r, &fits, Scroll::Both, 1.0).is_empty());
    }

    #[test]
    fn a_frames_label_says_how_it_scrolls() {
        let mut n = Node::new(NodeKind::Frame);
        n.id = "f".into();
        n.name = Some("Trails".into());
        assert_eq!(frame_label(&n), "Trails");
        n.scroll = Some(Scroll::X);
        assert_eq!(frame_label(&n), "Trails ⇆");
        n.scroll = Some(Scroll::Both);
        assert_eq!(frame_label(&n), "Trails ⇆⇅");
    }

    #[test]
    fn the_insertion_bar_sits_on_its_place_at_any_zoom_and_beside_a_wrapped_child() {
        let pr = Rect::new(0.0, 0.0, 300.0, 200.0);
        let k = Rect::new(100.0, 60.0, 40.0, 30.0);
        // E-board-3: 2 screen px centred on the place, not 8 px off at 800%.
        let bar = insertion_bar(Layout::Row, &pr, Some(k), None, 1.0 / 8.0);
        assert_eq!((bar.cx(), bar.w * 8.0), (98.0, 2.0));
        assert_eq!((bar.y, bar.h), (4.0, 192.0), "a row's spans the frame");
        let bar = insertion_bar(Layout::Wrap, &pr, Some(k), None, 1.0);
        assert_eq!((bar.cx(), bar.y, bar.h), (98.0, 60.0, 30.0), "a wrap's beside the child, its height");
        let bar = insertion_bar(Layout::Wrap, &pr, None, Some(k), 1.0);
        assert_eq!((bar.cx(), bar.y), (142.0, 60.0), "after the last on its line");
        let bar = insertion_bar(Layout::Column, &pr, Some(k), None, 1.0);
        assert_eq!((bar.cy(), bar.w), (58.0, 292.0));
    }

    #[test]
    fn pin_marks_run_from_each_pinned_edge_to_the_frame() {
        use relay_bus::design::{PinX, PinY};
        let pr = Rect::new(0.0, 0.0, 300.0, 200.0);
        let r = Rect::new(100.0, 50.0, 40.0, 20.0);
        let mut n = Node::new(NodeKind::Rect);
        n.pin_x = Some(PinX::Right);
        n.pin_y = Some(PinY::Center);
        let lines = pin_lines(&n, &r, &pr, 1.0);
        assert_eq!(lines, [((140.0, 60.0), (300.0, 60.0)), ((116.0, 100.0), (124.0, 100.0))]);
        n.pin_x = Some(PinX::LeftRight);
        n.pin_y = Some(PinY::Top);
        let lines = pin_lines(&n, &r, &pr, 1.0);
        assert_eq!(lines, [((0.0, 60.0), (100.0, 60.0)), ((140.0, 60.0), (300.0, 60.0)), ((120.0, 0.0), (120.0, 50.0))]);
    }

    #[test]
    fn plays_time_runs_from_its_first_frame_and_holds_while_paused() {
        let mut t = Timing::default();
        assert_eq!(t.at(5_000_000), 0.0, "stopped: where it was");
        t.resume();
        assert_eq!(t.at(5_000_000), 0.0, "the first frame after a start is its 0");
        assert_eq!(t.at(5_500_000), 500.0);
        assert_eq!(t.at(4_900_000), 0.0, "a frame time from before the start is the start");
        let at = t.at(6_200_000);
        t.pause(at);
        assert_eq!((t.at(9_000_000), t.at(12_000_000)), (1200.0, 1200.0), "paused, it holds");
        t.resume();
        assert_eq!((t.at(20_000_000), t.at(20_016_000)), (1200.0, 1216.0), "and goes on from there");
        t.seek(300.0);
        assert_eq!((t.at(30_000_000), t.at(30_100_000)), (300.0, 400.0), "a seek while running runs on from it");
        t.pause(2000.0);
        t.seek(50.0);
        assert_eq!(t.at(40_000_000), 50.0, "a seek while paused shows that moment");
    }

    #[test]
    fn play_ends_only_after_a_motion_that_never_loops() {
        assert!(ends(3000.0, 3000.0, false, false), "the end itself is the end");
        assert!(ends(3016.0, 3000.0, false, false));
        assert!(!ends(2990.0, 3000.0, false, false));
        assert!(!ends(9000.0, 3000.0, true, false), "a loop plays on");
        assert!(!ends(9000.0, 3000.0, false, true), "so does a picture that animates itself");
        assert!(!ends(9000.0, 0.0, false, true), "pictures alone have no end");
    }

    #[test]
    fn plays_time_reads_in_seconds() {
        assert_eq!(time_text(1200.0, 3000.0), "1.20 / 3.00 s");
        assert_eq!(time_text(0.0, 450.0), "0.00 / 0.45 s");
        assert_eq!(time_text(65_432.0, 0.0), "65.43 s");
    }

    #[test]
    fn the_span_loops_when_any_clock_does() {
        use relay_bus::design::Motion;
        let motion = |last: f64, looped: bool| -> Motion {
            serde_json::from_value(json!({"loop": looped, "keys": [{"at_ms": 0, "opacity": 0}, {"at_ms": last, "opacity": 1}]})).unwrap()
        };
        let mut page = Node::new(NodeKind::Page);
        page.id = "p".into();
        let mut a = rect("a", "#FFFFFF");
        a.parent = Some("p".into());
        a.motion = Some(motion(800.0, false));
        let mut f = Node::new(NodeKind::Frame);
        (f.id, f.parent) = ("f".into(), Some("p".into()));
        f.timeline = serde_json::from_value(json!({"duration_ms": 2000, "loop": false})).unwrap();
        let mut b = rect("b", "#FFFFFF");
        b.parent = Some("f".into());
        b.motion = Some(motion(500.0, true));
        let scene = Scene::new(vec![page.clone(), a.clone(), f.clone(), b.clone()]);
        // The page's own clock loops by default; the frame's is told not to.
        assert_eq!(play_span(&Clocks::new(&scene, "p")), (2000.0, true));
        let mut page = page;
        page.timeline = serde_json::from_value(json!({"loop": false})).unwrap();
        let scene = Scene::new(vec![page, a, f, b]);
        assert_eq!(play_span(&Clocks::new(&scene, "p")), (2000.0, false), "a looping motion on a clock that ends ends with it");
    }

    #[test]
    fn fit_frames_between_the_panels_that_show() {
        assert_eq!(fit_area(1200.0, 800.0, None, None), Rect::new(0.0, 76.0, 1200.0, 724.0));
        // E-board-21: the layers panel's 260 px and the inspector's 256 px left out.
        let area = fit_area(1200.0, 800.0, Some(280.0), Some(924.0));
        assert_eq!((area.x, area.w), (280.0, 644.0));
        let r = Rect::new(0.0, 0.0, 390.0, 844.0);
        let mut cam = Camera::fit(&r, area.w, area.h, 56.0, 1.0);
        cam.x -= area.x / cam.zoom;
        cam.y -= area.y / cam.zoom;
        let (sx, _) = cam.to_screen(r.cx(), r.cy());
        assert!((sx - (280.0 + 644.0 / 2.0)).abs() < 1e-6, "centred between them: {sx}");
        assert_eq!(fit_area(500.0, 800.0, Some(280.0), Some(220.0)).w, 500.0, "too narrow between them: the whole width");
    }
}
