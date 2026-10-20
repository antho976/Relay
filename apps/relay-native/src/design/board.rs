//! One design open on the canvas: the top bar, the canvas and what floats over it (the tool
//! dock, the selection's toolbar, the layers and the inspector, zoom), and the commands they
//! share. Input is `tools.rs`; editing a text in place is `textedit.rs`.

use super::canvas::Canvas;
use super::doc::{Change, Doc};
use super::glyphs;
use super::paint::{self, fill_of, grect, rgba, Painter, CANVAS};
use super::widgets::tip;
use crate::app::{label, Ui};
use gtk::prelude::*;
use gtk::{gdk, gio, graphene, gsk, pango};
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp};
use relay_client::design::edit;
use relay_client::design::geom::step_zoom;
use relay_client::design::layout::{self, Placed};
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
        /// A container the nodes would land in, when it is not their parent.
        target: Option<String>,
        /// In an auto-layout parent: the place the node would take.
        reorder: Option<(String, u32)>,
        /// The node to select if the press ends as a click.
        click: Option<String>,
        at: (f64, f64),
    },
    Resize { handle: Handle, bounds: Rect, orig: Vec<(String, Node, Rect)>, moved: bool },
    Endpoint { id: String, which: usize, orig: Node, moved: bool },
    /// `from`/`to`: the nodes a line's ends attach to, a connector's.
    Create { tool: Tool, parent: String, a: (f64, f64), b: (f64, f64), from: Option<String>, to: Option<String> },
    Draw { parent: String, points: Vec<[f64; 2]> },
    /// Turning one node about its centre (page coordinates), from the pointer's first angle.
    Rotate { id: String, centre: (f64, f64), start: f64, orig: Node, moved: bool },
    /// Rubbing out strokes: each touched one with the opacity it had.
    Erase { hits: Vec<(String, Option<f64>)> },
}

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
    pub(super) content: RefCell<Option<gsk::RenderNode>>,
    pub(super) cam: Cell<Camera>,
    pub(super) cams: RefCell<HashMap<String, Camera>>,
    pub(super) size: Cell<(f64, f64)>,
    fitted: Cell<bool>,
    pub(super) selection: RefCell<Vec<String>>,
    pub(super) hover: RefCell<Option<String>>,
    pub(super) tool: Cell<Tool>,
    pub(super) tool_lock: Cell<bool>,
    pub(super) gesture: RefCell<Gesture>,
    pub(super) guides: RefCell<Vec<Guide>>,
    pub(super) space: Cell<bool>,
    pub(super) pointer: Cell<(f64, f64)>,
    /// The last primary press, for telling a double click: time in µs and where.
    pub(super) last_press: Cell<(i64, f64, f64)>,
    flash: RefCell<(HashSet<String>, i64)>,
    flash_tick: RefCell<Option<gtk::TickCallbackId>>,
    labels: RefCell<HashMap<String, pango::Layout>>,
    pub(super) pen: RefCell<String>,
    pub(super) clip: RefCell<Vec<Node>>,
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
    agent_undo: gtk::Button,
    /// Whose change that Undo reverts.
    agent_last: RefCell<Option<String>>,
    pub(super) ask: gtk::Button,
    /// What was typed to Ask and not sent, for the next time it opens.
    pub(super) ask_draft: RefCell<String>,
    pub(super) zoom_label: gtk::Label,
    pub(super) tool_keys: RefCell<Vec<(Tool, gtk::ToggleButton)>>,
    pub(super) shape_key: gtk::ToggleButton,
    pub(super) pen_key: gtk::ToggleButton,
    pub(super) page_label: gtk::Label,
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
    pub(super) inspector: gtk::Box,
    pub(super) inspector_scroll: gtk::ScrolledWindow,
    /// The tools along the top left, which the inspector keeps clear of.
    dock: gtk::Box,
    pub(super) inspector_key: gtk::ToggleButton,
    pub(super) layers: RefCell<Option<Rc<super::layers::Layers>>>,
    pub(super) layers_panel: gtk::Revealer,
    pub(super) edit: RefCell<Option<super::textedit::Edit>>,
    toast: gtk::Revealer,
    toast_label: gtk::Label,
    toast_serial: Cell<u64>,
    chrome_pending: Cell<bool>,
    pub(super) actions: gio::SimpleActionGroup,
    /// Back to the gallery, when the board sits in the Design view.
    pub(super) back: RefCell<Option<Rc<dyn Fn()>>>,
    pub(super) windowed: bool,
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
        let ask = gtk::Button::new();
        ask.add_css_class("design-ask");
        let ask_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        ask_row.append(&glyphs::image("spark", 14));
        ask_row.append(&label("Ask agent", ""));
        ask.set_child(Some(&ask_row));
        ask.set_tooltip_text(Some("Ask the terminal's agent to change the selection or this page (Ctrl+Enter)"));
        ask.set_widget_name("design-ask");
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

        let shape_key = gtk::ToggleButton::new();
        let pen_key = gtk::ToggleButton::new();
        let painter = Painter::new(&canvas);
        *painter.assets_dir.borrow_mut() = doc.assets_dir.borrow().clone();
        let first_page = doc.scene.borrow().pages().first().cloned().unwrap_or_default();
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
            content: RefCell::default(),
            cam: Cell::new(Camera::default()),
            cams: RefCell::default(),
            size: Cell::new((0.0, 0.0)),
            fitted: Cell::new(false),
            selection: RefCell::default(),
            hover: RefCell::default(),
            tool: Cell::new(Tool::Select),
            tool_lock: Cell::new(false),
            gesture: RefCell::new(Gesture::Idle),
            guides: RefCell::default(),
            space: Cell::new(false),
            pointer: Cell::new((0.0, 0.0)),
            last_press: Cell::new((0, 0.0, 0.0)),
            flash: RefCell::new((HashSet::new(), 0)),
            flash_tick: RefCell::default(),
            labels: RefCell::default(),
            pen: RefCell::new(String::new()),
            clip: RefCell::default(),
            style_clip: RefCell::default(),
            vector: RefCell::default(),
            title,
            terminal,
            terminal_label,
            terminal_lamp,
            lamp_state: RefCell::default(),
            status,
            status_serial: Cell::new(0),
            agent_undo,
            agent_last: RefCell::default(),
            ask,
            ask_draft: RefCell::default(),
            zoom_label,
            tool_keys: RefCell::default(),
            shape_key,
            pen_key,
            page_label,
            context: RefCell::default(),
            context_at: Cell::new((0, 0)),
            context_full: Cell::new(None),
            context_compact: Cell::new(false),
            context_kind: Cell::new(""),
            context_refit: Cell::new(false),
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
        board.build_title_menu(&title_key);
        let layers = super::layers::Layers::new(&board);
        board.layers_panel.set_child(Some(&layers.root));
        *board.layers.borrow_mut() = Some(layers);
        super::tools::install(&board);

        let weak = Rc::downgrade(&board);
        back_key.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
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
                if let Some(ui) = b.ui.upgrade() {
                    b.end_edit(true);
                    super::window::pop_out(&ui, b.doc.clone());
                    if let Some(back) = b.back.borrow().clone() {
                        back();
                    }
                }
            }
        });
        let weak = Rc::downgrade(&board);
        board.ask.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                b.ask_agent(key.upcast_ref());
            }
        });
        let weak = Rc::downgrade(&board);
        board.agent_undo.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.undo_agent();
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
        board
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
        let placed = Rc::new(layout::place(&scene, &self.page.borrow(), &*self.painter));
        if let Some(t) = started {
            println!("DESIGN_TRACE layout {:.3} ms {} nodes", t.elapsed().as_secs_f64() * 1e3, placed.rects.len());
        }
        *self.placed.borrow_mut() = Some(placed.clone());
        placed
    }

    /// The scene or the layout changed: lay out again and redraw, and refresh the chrome
    /// once this turn of the main loop is done.
    pub(super) fn invalidate(&self) {
        self.placed.borrow_mut().take();
        self.content.borrow_mut().take();
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
        let snap = gtk::Snapshot::new();
        self.painter.paint_page(&snap, scene, placed, &self.page.borrow(), None);
        let node = snap.to_node();
        if let Some(t) = started {
            println!("DESIGN_TRACE content {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
        }
        self.content.borrow_mut().clone_from(&node);
        node
    }

    pub(super) fn page_bounds(&self) -> Option<Rect> {
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let page = self.page.borrow();
        Rect::bounds(scene.children(&page).iter().filter_map(|id| placed.rects.get(id)))
    }

    pub(super) fn selection_bounds(&self) -> Option<Rect> {
        let placed = self.placed();
        Rect::bounds(self.selection.borrow().iter().filter_map(|id| placed.rects.get(id)))
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
        // The dock sits over the top left: fit below it, and keep the frame names in view.
        let top = 76.0;
        match r {
            Some(r) => {
                let mut cam = Camera::fit(&r, w, (h - top).max(1.0), 56.0, if closer { 4.0 } else { 1.0 });
                cam.y -= top / cam.zoom;
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
                // An undo can take away the page on show (one just added, or duplicated).
                self.keep_page();
                self.prune_selection();
                self.invalidate();
            }
            Change::Reloaded { changed, by } => {
                *self.painter.assets_dir.borrow_mut() = self.doc.assets_dir.borrow().clone();
                self.painter.retain(&self.doc.scene.borrow());
                self.prune_selection();
                self.invalidate();
                if by != "user" && !changed.is_empty() {
                    // "Just now" for a minute; then who it was.
                    let serial = self.set_status(&format!("{by} edited it just now"), Some("design-status-agent"));
                    let (weak, who) = (self.me.clone(), by.clone());
                    glib::timeout_add_local_once(std::time::Duration::from_secs(60), move || {
                        if let Some(b) = weak.upgrade().filter(|b| b.status_serial.get() == serial) {
                            b.set_status(&format!("Last edited by {who}"), None);
                        }
                    });
                    self.start_flash(changed.clone());
                    *self.agent_last.borrow_mut() = Some(by.clone());
                    self.agent_undo.set_tooltip_text(Some(&format!("Undo {by}'s last change to this design")));
                    self.agent_undo.set_visible(true);
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

    /// Show the first page when the one on show is gone, and bring the page list up to date.
    fn keep_page(&self) {
        let gone = self.doc.scene.borrow().node(&self.page.borrow()).is_none();
        if gone {
            let first = self.doc.scene.borrow().pages().first().cloned().unwrap_or_default();
            self.set_page(&first);
        }
        if let Some(layers) = self.layers.borrow().as_ref() {
            layers.pages_changed();
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
        let design = self.doc.design.borrow().clone();
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
        glib::timeout_add_local_once(std::time::Duration::from_millis(2600 + text.len() as u64 * 30), move || {
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
        self.overlays(snap, &scene, &placed);
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
            let name = n.name.clone().unwrap_or_else(|| "Frame".into());
            let layout = self.label_layout(&format!("frame:{id}"), &name, 12.0, pango::Weight::Medium, false);
            layout.set_width((s.w * pango::SCALE as f64) as i32);
            layout.set_ellipsize(pango::EllipsizeMode::End);
            let (_, lh) = layout.pixel_size();
            let lit = selection.contains(id) || hover.as_deref() == Some(id.as_str());
            let color = if lit { rgba(SELECT) } else { gdk::RGBA::new(0.62, 0.6, 0.57, 1.0) };
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
            let name = n.name.clone().unwrap_or_else(|| "Frame".into());
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
                if let Some(r) = placed.rect(id) {
                    let s = self.screen(&r).inflate(2.0);
                    let rr = gsk::RoundedRect::from_rect(grect(&s), 3.0);
                    snap.append_border(&rr, &[2.0; 4], &[c, c, c, c]);
                }
            }
        }
        // Hover.
        if let Some(id) = self.hover.borrow().as_ref() {
            if !self.selection.borrow().contains(id) && matches!(*self.gesture.borrow(), Gesture::Idle) {
                if let Some(r) = placed.rect(id) {
                    self.outline(snap, &self.screen(&r), SELECT, 1.5);
                }
            }
        }
        let gesture = self.gesture.borrow();
        // Where a dragged node would land.
        if let Gesture::Move { target: Some(t), moved: true, .. } = &*gesture {
            if let Some(r) = placed.rect(t) {
                self.outline(snap, &self.screen(&r), SELECT, 2.0);
            }
        }
        if let Gesture::Move { reorder: Some((parent, index)), moved: true, ids, .. } = &*gesture {
            self.insertion(snap, scene, placed, parent, *index as usize, ids);
        }
        // The selection: each node, then the box with its handles and size.
        let selection = self.selection.borrow();
        let resizing = matches!(*gesture, Gesture::Resize { .. } | Gesture::Endpoint { .. });
        if !selection.is_empty() && editing.is_none() {
            for id in selection.iter() {
                if let Some(r) = placed.rect(id) {
                    self.outline(snap, &self.screen(&r), SELECT, 1.0);
                }
            }
            if let Some((_, r, turn)) = self.single_turn() {
                // One turned node: its outline and handles turn with it.
                let s = self.screen(&r);
                let at = |x: f64, y: f64| relay_client::design::shapes::rotate(x, y, s.cx(), s.cy(), turn);
                let pb = gsk::PathBuilder::new();
                let corners = [at(s.x, s.y), at(s.right(), s.y), at(s.right(), s.bottom()), at(s.x, s.bottom())];
                pb.move_to(corners[0].0 as f32, corners[0].1 as f32);
                for c in &corners[1..] {
                    pb.line_to(c.0 as f32, c.1 as f32);
                }
                pb.close();
                snap.append_stroke(&pb.to_path(), &gsk::Stroke::new(1.0), &rgba(SELECT));
                if !matches!(*gesture, Gesture::Move { moved: true, .. }) {
                    for h in HANDLES {
                        let (x, y) = handle_point(&s, h);
                        let (x, y) = at(x, y);
                        self.handle(snap, x, y, false);
                    }
                }
                let shown = relay_client::design::shapes::rotated_bounds(&s, turn);
                self.size_pill(snap, &shown, &r);
            } else if let Some(b) = Rect::bounds(selection.iter().filter_map(|id| placed.rects.get(id))) {
                let s = self.screen(&b);
                if selection.len() > 1 {
                    self.outline(snap, &s, SELECT, 1.0);
                }
                let line = selection.len() == 1
                    && scene.node(&selection[0]).is_some_and(|n| n.kind == NodeKind::Line);
                if line {
                    for p in self.endpoints(&selection[0]) {
                        self.handle(snap, p.0, p.1, true);
                    }
                } else if !matches!(*gesture, Gesture::Move { moved: true, .. }) && s.w > 2.0 && s.h > 2.0 {
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
        }
        // Guides.
        for g in self.guides.borrow().iter() {
            let c = rgba(GUIDE);
            let cam = self.cam.get();
            if g.vertical {
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
                            let pts = relay_client::design::shapes::polygon(&r, sides, star);
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
            Gesture::Draw { points, .. } if points.len() > 1 => {
                let cam = self.cam.get();
                let pb = gsk::PathBuilder::new();
                let (x0, y0) = cam.to_screen(points[0][0], points[0][1]);
                pb.move_to(x0 as f32, y0 as f32);
                for p in &points[1..] {
                    let (x, y) = cam.to_screen(p[0], p[1]);
                    pb.line_to(x as f32, y as f32);
                }
                let highlighter = self.tool.get() == Tool::Highlighter;
                let width = if highlighter { 16.0 } else { 3.0 } * cam.zoom;
                let stroke = gsk::Stroke::new(width as f32);
                stroke.set_line_cap(gsk::LineCap::Round);
                stroke.set_line_join(gsk::LineJoin::Round);
                snap.append_stroke(&pb.to_path(), &stroke, &rgba(&self.pen_color(highlighter)));
            }
            _ => {}
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
        let to = if closing { first } else { pointer };
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

    fn insertion(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, parent: &str, index: usize, moving: &[String]) {
        let Some(p) = scene.node(parent) else {
            return;
        };
        let row = p.layout_mode() == relay_bus::design::Layout::Row;
        let kids: Vec<Rect> = scene
            .children(parent)
            .iter()
            .filter(|c| !moving.contains(c))
            .filter_map(|c| placed.rect(c))
            .collect();
        let Some(pr) = placed.rect(parent) else {
            return;
        };
        let at = if kids.is_empty() {
            if row { pr.x + 8.0 } else { pr.y + 8.0 }
        } else if index >= kids.len() {
            let last = kids.last().copied().unwrap_or_default();
            if row { last.right() + 2.0 } else { last.bottom() + 2.0 }
        } else {
            let k = kids[index];
            if row { k.x - 2.0 } else { k.y - 2.0 }
        };
        let r = if row {
            Rect::new(at - 1.0, pr.y + 4.0, 2.0 / self.cam.get().zoom, pr.h - 8.0)
        } else {
            Rect::new(pr.x + 4.0, at - 1.0, pr.w - 8.0, 2.0 / self.cam.get().zoom)
        };
        let s = self.screen(&r);
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

    /// The one selected node when it is turned: its id, unturned page box and turn.
    pub(super) fn single_turn(&self) -> Option<(String, Rect, f64)> {
        let sel = self.selection.borrow();
        let [id] = sel.as_slice() else {
            return None;
        };
        let scene = self.doc.scene.borrow();
        let n = scene.node(id).filter(|n| n.kind != NodeKind::Line)?;
        let turn = relay_client::design::shapes::rotation(n);
        if turn == 0.0 {
            return None;
        }
        Some((id.clone(), self.placed().rect(id)?, turn))
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
            let (pts, _) = relay_client::design::shapes::line_points(&scene, &placed, n);
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

    pub(super) fn pen_color(&self, highlighter: bool) -> String {
        if highlighter {
            return "#FFD60A66".into();
        }
        let chosen = self.pen.borrow().clone();
        if !chosen.is_empty() {
            return chosen;
        }
        let scene = self.doc.scene.borrow();
        let ground = scene.node(&self.page.borrow()).and_then(fill_of).unwrap_or_else(|| CANVAS.into());
        if paint::luminance(&ground) > 0.35 { "#1A1A1A".into() } else { "#EDE9E2".into() }
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
        }
        self.refresh_inspector_values();
        if let Some(layers) = self.layers.borrow().as_ref() {
            layers.refresh();
        }
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
        self.tool.set(tool);
        for (t, key) in self.tool_keys.borrow().iter() {
            key.set_active(*t == tool);
        }
        let shapes = tool.is_shape();
        self.shape_key.set_active(shapes);
        let pens = matches!(tool, Tool::Pen | Tool::Vector | Tool::Highlighter | Tool::Eraser);
        self.pen_key.set_active(pens);
        if shapes {
            self.shape_key.set_child(Some(&glyphs::image(tool.glyph(), 16)));
            tip(&self.shape_key, &tool.tip());
        }
        if pens {
            self.pen_key.set_child(Some(&glyphs::image(tool.glyph(), 16)));
            tip(&self.pen_key, &tool.tip());
        }
        if tool != Tool::Select {
            self.end_edit(true);
        }
        super::tools::update_cursor(self, None);
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
                let target = b.selected().into_iter().find(|id| b.doc.scene.borrow().node(id).is_some_and(|n| n.kind == NodeKind::Icon));
                pop.set_child(Some(&super::icons::picker(&b, pop, target)));
            }
        });
        dock.append(&icon_key);
        let image_key = gtk::Button::new();
        image_key.add_css_class("design-tool");
        image_key.set_child(Some(&glyphs::image("image", 16)));
        tip(&image_key, "Place an image (Ctrl+Shift+K)");
        let weak = Rc::downgrade(self);
        image_key.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                super::assets::choose_images(&b);
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
        let weak = Rc::downgrade(self);
        key.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                b.set_tool(tool);
                key.set_active(true);
            }
        });
        key
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
                        *b.pen.borrow_mut() = color.to_string();
                        b.set_tool(Tool::Pen);
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
                        *doc.design.borrow_mut() = row;
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
        let view: Group = ("View", &[("Ctrl scroll / pinch", "Zoom at the pointer"), ("Ctrl − / Ctrl =", "Zoom out / in"), ("Shift 1", "Zoom to fit"), ("Shift 2", "Zoom to selection"), ("Shift 0 or Ctrl 0", "Zoom to 100%"), ("Alt 1", "Pages and layers"), ("Ctrl .", "Inspector")]);
        let selection: Group = ("Selection", &[("Click", "Select what is under the pointer"), ("Esc", "Select the parent"), ("Enter", "Edit text, or select inside"), ("Tab / Shift Tab", "Next / previous layer"), ("Shift click", "Add or remove"), ("Ctrl A", "Select all"), ("Alt drag", "Duplicate while moving")]);
        let edit: Group = ("Edit", &[("Ctrl Z / Ctrl Shift Z", "Undo / redo"), ("Ctrl C / X / V", "Copy / cut / paste"), ("Ctrl D", "Duplicate"), ("Delete / Backspace", "Delete"), ("Ctrl G / Ctrl Shift G", "Group / ungroup"), ("Ctrl Alt G", "Frame selection"), ("Ctrl ] / Ctrl [", "Forward / backward"), ("Shift H / Shift V", "Flip horizontally / vertically"), ("Ctrl Alt C / V", "Copy / paste style"), ("Alt A / D / W / S", "Align left / right / top / bottom"), ("Alt H / Alt V", "Align centres / middles"), ("Ctrl Shift L / H", "Lock / hide"), ("Ctrl Shift K", "Place an image"), ("Ctrl Enter", "Ask the agent"), ("Arrows", "Nudge 1 px (Shift: 10)")]);
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
        if let Some(edit) = self.edit.borrow().as_ref() {
            if edit.view.upcast_ref::<gtk::Widget>() == widget {
                let Some(r) = self.edit_rect() else {
                    return Some(gdk::Rectangle::new(-10_000, -10_000, 10, 10));
                };
                let r = self.screen(&r);
                return Some(gdk::Rectangle::new(r.x.floor() as i32, r.y.floor() as i32, r.w.ceil().max(4.0) as i32 + 2, r.h.ceil().max(4.0) as i32));
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
        if self.selection.borrow().is_empty() || !matches!(*self.gesture.borrow(), Gesture::Idle) {
            return Some(away);
        }
        let Some(b) = self.selection_bounds() else {
            return Some(away);
        };
        let s = self.screen(&b);
        let (cw, ch) = self.size.get();
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
            let r = b.selection_bounds().or_else(|| b.page_bounds());
            b.fit(r, true);
        });
        add("zoom-50", |b| b.zoom_to(0.5));
        add("zoom-100", |b| b.zoom_to(1.0));
        add("zoom-200", |b| b.zoom_to(2.0));
        use super::export::{export_selection, Format};
        add("export", |b| export_selection(b, false, Format::Png(2.0)));
        add("export-svg", |b| export_selection(b, false, Format::Svg));
        add("export-pdf", |b| export_selection(b, false, Format::Pdf));
        add("export-page", |b| export_selection(b, true, Format::Png(2.0)));
        add("export-page-svg", |b| export_selection(b, true, Format::Svg));
        add("export-page-pdf", |b| export_selection(b, true, Format::Pdf));
        add("flip-h", |b| b.flip(true));
        add("flip-v", |b| b.flip(false));
        add("copy-style", |b| b.copy_style());
        add("paste-style", |b| b.paste_style());
        add("ask", |b| b.ask_agent(b.ask.upcast_ref()));
        add("rename", |b| b.rename_design());
        add("duplicate-design", super::duplicate_design);
        add("delete-design", super::delete_design);
        add("pop-out", |b| {
            if let Some(ui) = b.ui.upgrade() {
                super::window::pop_out(&ui, b.doc.clone());
                if let Some(back) = b.back.borrow().clone() {
                    back();
                }
            }
        });
        add("add-page", |b| b.add_page());
    }

    fn refresh_actions(&self) {
        let has = !self.selection.borrow().is_empty();
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

    /// The newest change `agent` made to this design that is still standing: an audited
    /// `design.apply` with its undo kept and not undone yet.
    async fn agent_change(ui: &Rc<Ui>, design: i64, project: i64, agent: &str) -> Option<i64> {
        let listed = ui
            .call("audit.list", json!({"project_id": project, "actor": format!("agent:{agent}"), "op_prefix": "design.apply", "limit": 500}))
            .await
            .ok()?;
        let mut rows = crate::app::rows(&listed, "rows");
        rows.sort_by_key(|r| std::cmp::Reverse(r["id"].as_i64().unwrap_or(0)));
        rows.into_iter()
            .find(|r| {
                r["kind"] == "ok"
                    && r["undone_by"].is_null()
                    && r["undo_op"]["payload"]["design_id"].as_i64() == Some(design)
            })
            .and_then(|r| r["id"].as_i64())
    }

    /// Revert the last change the agent named by the status made, through `audit.undo`: the
    /// person's veto over what an agent drew. Each press undoes one more of its changes.
    pub(super) fn undo_agent(&self) {
        let (Some(ui), Some(agent)) = (self.ui.upgrade(), self.agent_last.borrow().clone()) else {
            return;
        };
        let design = self.doc.id;
        let project = self.doc.design.borrow().project_id;
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let Some(audit) = Self::agent_change(&ui, design, project, &agent).await else {
                if let Some(b) = weak.upgrade() {
                    b.agent_undo.set_visible(false);
                    b.toast(&format!("Nothing of {agent}'s left to undo here"));
                }
                return;
            };
            let undone = ui.call("audit.undo", json!({"audit_id": audit})).await;
            let more = Self::agent_change(&ui, design, project, &agent).await.is_some();
            let Some(b) = weak.upgrade() else {
                return;
            };
            match undone {
                Ok(_) => {
                    b.set_status(&format!("Undid {agent}'s change"), None);
                    b.agent_undo.set_visible(more);
                }
                Err(e) => b.toast(&format!("Could not undo {agent}'s change: {e}")),
            }
        });
    }

    pub(super) fn undo(&self) {
        self.end_edit(true);
        self.doc.undo(self.selection.borrow().clone());
    }

    pub(super) fn redo(&self) {
        self.end_edit(true);
        self.doc.redo(self.selection.borrow().clone());
    }

    pub(super) fn delete_selection(&self) {
        let ids = self.selected();
        // A text being typed for the first time is not on the engine yet: its edit drops it.
        let fresh = self.edit.borrow().as_ref().filter(|e| e.created).map(|e| e.id.clone());
        if fresh.is_some_and(|f| ids.contains(&f)) {
            self.end_edit(false);
        }
        let ids = self.selected();
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let ids: Vec<String> = ids.into_iter().filter(|id| scene.node(id).is_some_and(|n| n.kind != NodeKind::Page)).collect();
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

    /// Keep the first selected layer's style, to paste on others.
    pub(super) fn copy_style(&self) {
        let first = self.selected().into_iter().next();
        let style = first.and_then(|id| relay_client::design::flip::style_of(&self.doc.scene.borrow(), &id));
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
        let ops = relay_client::design::flip::paste_style(&self.doc.scene.borrow(), &self.selected(), &style);
        self.commit(ops);
    }

    pub(super) fn copy(&self) {
        if let Some(count) = self.copy_quietly() {
            self.toast(&format!("Copied {count} {}", if count == 1 { "layer" } else { "layers" }));
        }
    }

    /// Copy the selection and take it away.
    pub(super) fn cut(&self) {
        if let Some(count) = self.copy_quietly() {
            self.delete_selection();
            self.toast(&format!("Cut {count} {}", if count == 1 { "layer" } else { "layers" }));
        }
    }

    /// Put the selection on the clipboard; how many layers that was.
    fn copy_quietly(&self) -> Option<usize> {
        let placed = self.placed();
        let nodes = edit::gather(&self.doc.scene.borrow(), &placed, &self.selection.borrow());
        if nodes.is_empty() {
            return None;
        }
        let texts: Vec<String> = nodes.iter().filter_map(|n| n.text.clone()).collect();
        let bytes = serde_json::to_vec(&json!({"relay_design": 1, "nodes": nodes})).unwrap_or_default();
        *self.clip.borrow_mut() = nodes;
        CLIP.with(|c| *c.borrow_mut() = self.clip.borrow().clone());
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
        let clipboard = self.canvas.clipboard();
        let formats = clipboard.formats();
        let weak = self.me.clone();
        if formats.contain_mime_type(MIME) && clipboard.is_local() {
            let nodes = CLIP.with(|c| c.borrow().clone());
            self.paste_nodes(nodes);
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
                let nodes = serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|v| serde_json::from_value::<Vec<Node>>(v["nodes"].clone()).ok())
                    .unwrap_or_default();
                if let Some(b) = weak.upgrade() {
                    b.paste_nodes(nodes);
                }
            });
            return;
        }
        if formats.contains_type(gdk::Texture::static_type()) {
            glib::spawn_future_local(async move {
                if let Ok(Some(texture)) = clipboard.read_texture_future().await {
                    if let Some(b) = weak.upgrade() {
                        super::assets::place_texture(&b, texture, None);
                    }
                }
            });
            return;
        }
        glib::spawn_future_local(async move {
            if let Ok(Some(text)) = clipboard.read_text_future().await {
                if let Some(b) = weak.upgrade() {
                    b.paste_text(text.trim_end());
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

    fn paste_nodes(&self, nodes: Vec<Node>) {
        if nodes.is_empty() {
            return;
        }
        let target = self.paste_target();
        let placed = self.placed();
        let roots: Vec<&Node> = nodes.iter().filter(|n| n.parent.as_deref().is_none_or(|p| !nodes.iter().any(|o| o.id == p))).collect();
        let Some(min_x) = roots.iter().map(|n| n.x).reduce(f64::min) else {
            return;
        };
        let min_y = roots.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
        let same = roots.iter().all(|n| n.parent.as_deref() == Some(target.as_str()))
            && roots.iter().all(|n| self.doc.scene.borrow().node(&n.id).is_some());
        let (dx, dy) = if same {
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

    fn paste_text(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        let target = self.paste_target();
        let (w, h) = self.size.get();
        let (cx, cy) = self.to_page(w / 2.0, h / 2.0);
        let origin = self.origin_of(&target);
        let mut n = Node::new(NodeKind::Text);
        n.id = self.doc.scene.borrow_mut().fresh_id(NodeKind::Text);
        n.parent = Some(target);
        n.text = Some(text.chars().take(relay_bus::design::MAX_TEXT).collect());
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
        entry.set_text(&self.doc.title());
        entry.set_width_chars(32);
        pop.set_child(Some(&entry));
        let weak = self.me.clone();
        let p = pop.downgrade();
        entry.connect_activate(move |entry| {
            let title = entry.text().trim().to_string();
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            let Some(b) = weak.upgrade() else {
                return;
            };
            if title.is_empty() || title == b.doc.title() {
                return;
            }
            let Some(ui) = b.ui.upgrade() else {
                return;
            };
            let id = b.doc.id;
            glib::spawn_future_local(async move {
                match ui.call("design.update", json!({"design_id": id, "title": title})).await {
                    Ok(v) => {
                        if let Ok(row) = serde_json::from_value(v) {
                            *b.doc.design.borrow_mut() = row;
                        }
                        b.design_changed();
                        super::gallery_changed(&ui);
                    }
                    Err(e) => b.toast(&e.to_string()),
                }
            });
        });
        pop.connect_closed(|p| p.unparent());
        pop.popup();
        entry.grab_focus();
    }
}

thread_local! {
    /// The nodes last copied in this process, for a paste whose clipboard is our own.
    static CLIP: RefCell<Vec<Node>> = const { RefCell::new(Vec::new()) };
    /// Whether the inspector and the pages and layers panel show, kept for the session from
    /// one design (and window) to the next.
    static INSPECTOR_ON: Cell<bool> = const { Cell::new(true) };
    static LAYERS_ON: Cell<bool> = const { Cell::new(false) };
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

/// A size as the pill shows it: whole pixels, or one decimal under 10.
pub fn fmt(v: f64) -> String {
    // Up to two decimals, as typed: 13.5 stays 13.5 and 1.25 stays 1.25.
    let text = format!("{:.2}", (v * 100.0).round() / 100.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".into() } else { text.into() }
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

#[allow(dead_code)]
pub fn kind_caption(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Page => "Page",
        NodeKind::Frame => "Frame",
        NodeKind::Group => "Group",
        NodeKind::Rect => "Rectangle",
        NodeKind::Ellipse => "Ellipse",
        NodeKind::Line => "Line",
        NodeKind::Text => "Text",
        NodeKind::Sticky => "Sticky",
        NodeKind::Path => "Drawing",
        NodeKind::Image => "Image",
        NodeKind::Polygon => "Polygon",
        NodeKind::Icon => "Icon",
    }
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

/// A node's name as the layers and the breadcrumb show it.
pub fn display_name(n: &Node) -> String {
    if let Some(name) = n.name.as_deref().filter(|s| !s.is_empty()) {
        return name.to_string();
    }
    if let Some(text) = n.text.as_deref().filter(|t| !t.trim().is_empty()) {
        let first = text.lines().next().unwrap_or("");
        let mut line: String = first.chars().take(40).collect();
        if line.len() < first.len() {
            line.push('…');
        }
        return line;
    }
    kind_caption(n.kind).to_string()
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

/// "Export selection as" PNG, SVG or PDF, for the menus that offer it.
pub(super) fn export_menu() -> gio::Menu {
    let section = gio::Menu::new();
    section.append(Some("Export selection as PNG (2×)…"), Some("design.export"));
    section.append(Some("Export selection as SVG…"), Some("design.export-svg"));
    section.append(Some("Export selection as PDF…"), Some("design.export-pdf"));
    section
}
