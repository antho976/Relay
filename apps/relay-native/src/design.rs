//! Design: a project's design canvases, beside Agents in the project's views. The gallery
//! lists the project's designs (each optionally a terminal's); opening one puts it on the
//! board, an infinite canvas of pages, frames, text, shapes, stickies, pen strokes and
//! pictures that everything on it can be clicked, edited and moved on. Agents make and change
//! designs through the same `design.*` ops, and an open board shows their changes as they land.
//! A design can also open in a window of its own.

mod agent_panel;
mod ask;
mod assets;
mod board;
mod canvas;
mod context;
mod doc;
mod drive;
mod export;
mod gallery;
mod glyphs;
mod icons;
mod inspector;
mod layers;
mod motion_panel;
mod paint;
mod pictures;
pub(crate) mod render;
mod shape_panels;
mod textedit;
mod tools;
mod widgets;
mod window;

use crate::app::Ui;
use board::Board;
use doc::Doc;
use gallery::{Filter, Gallery, Origin};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

/// The Design view of the main window: the gallery, or one design on the board.
pub struct View {
    pub root: gtk::Stack,
    gallery: Rc<Gallery>,
    host: gtk::Box,
    board: RefCell<Option<Rc<Board>>>,
    opening: Opening,
}

/// Which open of the main view's board is the one asked for last (B79): an open that finishes
/// after a newer one was asked for, or after the person went elsewhere, shows nothing.
#[derive(Default)]
struct Opening(Cell<u64>);

impl Opening {
    /// A new open; those before it no longer show what they open.
    fn begin(&self) -> u64 {
        self.cancel();
        self.0.get()
    }

    /// Whether open `ticket` is still the one asked for.
    fn current(&self, ticket: u64) -> bool {
        self.0.get() == ticket
    }

    /// The person went elsewhere: no open under way shows its design.
    fn cancel(&self) {
        self.0.set(self.0.get() + 1);
    }
}

/// What runs once a board opened for it has its size (E-gallery-6).
type Ready = Box<dyn FnOnce(&Rc<Board>)>;

thread_local! {
    static VIEW: RefCell<Option<Rc<View>>> = const { RefCell::new(None) };
    /// Every open board, the main view's and the windows', for routing `design.changed`.
    static BOARDS: RefCell<Vec<Weak<Board>>> = const { RefCell::new(Vec::new()) };
    /// A design.create is in flight: New waits for it (E-gallery-7).
    static CREATING: Cell<bool> = const { Cell::new(false) };
}

fn view() -> Option<Rc<View>> {
    VIEW.with(|v| v.borrow().clone())
}

/// Build the view and mount it beside the agent wall.
pub fn install(ui: &Rc<Ui>) {
    let gallery = Gallery::new(ui);
    let root = gtk::Stack::new();
    root.add_css_class("design-view");
    root.set_hhomogeneous(false);
    root.set_vhomogeneous(false);
    root.set_transition_type(gtk::StackTransitionType::Crossfade);
    root.set_transition_duration(120);
    root.add_named(&gallery.root, Some("gallery"));
    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.set_hexpand(true);
    host.set_vexpand(true);
    root.add_named(&host, Some("board"));
    root.set_visible_child_name("gallery");
    let view = Rc::new(View { root, gallery, host, board: RefCell::default(), opening: Opening::default() });
    let weak = Rc::downgrade(ui);
    *view.gallery.open.borrow_mut() = Some(Rc::new(move |id| {
        if let Some(ui) = weak.upgrade() {
            open(&ui, id);
        }
    }));
    ui.editor.mount_design(&view.root);
    VIEW.with(|v| *v.borrow_mut() = Some(view));
    let weak = Rc::downgrade(ui);
    ui.editor.connect_view_changed(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.editor.design_visible() {
                shown(&ui);
            }
        }
    });
    // The design windows open when Relay last closed come back (I-15).
    window::restore(ui);
}

/// The view came on screen, or its project changed: read the project's designs.
fn shown(ui: &Rc<Ui>) {
    let Some(view) = view() else {
        return;
    };
    let project = ui.project.get();
    if view.gallery.project.get() != project {
        view.gallery.switch(project);
        // An open under way is of the project left (B79).
        view.opening.cancel();
        close_board(&view);
    }
    gallery_refresh(ui, &view, true);
    let board = view.board.borrow().clone();
    if let Some(b) = board {
        b.canvas.grab_focus();
    }
}

fn close_board(view: &View) {
    if let Some(b) = view.board.borrow_mut().take() {
        b.end_edit(true);
        b.close_agent_panel();
        view.host.remove(&b.root);
        // The gallery comes back before its pictures are drawn again (TM-80).
        view.gallery.hold_thumbs(Duration::from_secs(1));
    }
    view.root.set_visible_child_name("gallery");
}

/// Whether the person sees the gallery: the Design view is up, and shows no board.
fn gallery_on_screen(ui: &Ui, view: &View) -> bool {
    ui.editor.design_visible() && view.root.visible_child_name().as_deref() == Some("gallery")
}

/// The gallery reads its designs again (with `sessions`, its terminals too) now if it is on
/// screen, else when it next shows: nothing is read or drawn behind a board (B53, TM-80).
fn gallery_refresh(ui: &Ui, view: &View, sessions: bool) {
    match (gallery_on_screen(ui, view), sessions) {
        (true, true) => view.gallery.refresh(),
        (true, false) => view.gallery.refresh_designs(),
        (false, _) => {
            view.gallery.stale.set(true);
            if sessions {
                view.gallery.owe_sessions();
            }
        }
    }
}

/// Show the Design view of the current project, filtered to `terminal`'s designs.
pub fn show(ui: &Rc<Ui>, terminal: Option<String>) {
    ui.navigate("design");
    if let Some(view) = view() {
        view.opening.cancel();
        close_board(&view);
        view.gallery.choose(match terminal {
            Some(t) => Filter::Terminal(t),
            None => Filter::All,
        });
        // Opened afresh: a search left from before would hide what was asked for (TM-52).
        view.gallery.clear_search();
        view.gallery.refresh();
    }
}

/// Open design `id` on the main view's board.
pub fn open(ui: &Rc<Ui>, id: i64) {
    open_on(ui, id, None);
}

/// Open design `id` on the main view's board and run `ready` with the board once it has its
/// size, to put back what a board showed before (E-gallery-6).
// native-canvas restores a board's camera and selection through it.
#[allow(dead_code)]
pub(crate) fn open_with(ui: &Rc<Ui>, id: i64, ready: impl FnOnce(&Rc<Board>) + 'static) {
    open_on(ui, id, Some(Box::new(ready)));
}

fn open_on(ui: &Rc<Ui>, id: i64, ready: Option<Ready>) {
    let Some(view) = view() else {
        return;
    };
    let shown = view.board.borrow().clone().filter(|b| b.doc.id == id);
    if let Some(b) = shown {
        // An open asked for before this one must not replace it when it lands.
        view.opening.cancel();
        view.root.set_visible_child_name("board");
        when_placed(&b, ready);
        return;
    }
    let ticket = view.opening.begin();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let opened = Doc::open(&ui, id).await;
        // A newer open, or the person leaving for the gallery or another project, made this
        // one moot (B79).
        if !view.opening.current(ticket) {
            return;
        }
        let doc = match opened {
            Ok(doc) => doc,
            Err(e) => {
                view.gallery.toast(&format!("Could not open the design: {e}"), None);
                return;
            }
        };
        close_board(&view);
        let b = Board::new(&ui, doc, false);
        let weak = Rc::downgrade(&view);
        *b.back.borrow_mut() = Some(Rc::new(move || {
            if let Some(view) = weak.upgrade() {
                view.opening.cancel();
                close_board(&view);
                view.gallery.refresh_designs();
            }
        }));
        view.host.append(&b.root);
        register(&b);
        *view.board.borrow_mut() = Some(b.clone());
        view.root.set_visible_child_name("board");
        b.canvas.grab_focus();
        when_placed(&b, ready);
    });
}

/// Run `ready` once `b` is laid out: on the frame after the one that gave it its size.
pub(super) fn when_placed(b: &Rc<Board>, ready: Option<Ready>) {
    let Some(ready) = ready else {
        return;
    };
    let ready = Cell::new(Some(ready));
    let weak = Rc::downgrade(b);
    b.root.add_tick_callback(move |root, _| {
        if root.width() <= 0 {
            return glib::ControlFlow::Continue;
        }
        if let (Some(b), Some(ready)) = (weak.upgrade(), ready.take()) {
            ready(&b);
        }
        glib::ControlFlow::Break
    });
}

/// A new design in `project`, tied to `session`, opened on the board.
pub fn create(ui: &Weak<Ui>, project: i64, session: Option<String>) {
    let Some(ui) = ui.upgrade() else {
        return;
    };
    if CREATING.with(|c| c.replace(true)) {
        return;
    }
    let creating = |on: bool| {
        CREATING.with(|c| c.set(on));
        if let Some(view) = view() {
            view.gallery.set_creating(on);
        }
    };
    creating(true);
    glib::spawn_future_local(async move {
        // The first name no design has: "Untitled design", then "Untitled design 2" and on.
        let taken = view().map(|v| v.gallery.titles()).unwrap_or_default();
        let title = (1..)
            .map(|n| if n == 1 { "Untitled design".to_string() } else { format!("Untitled design {n}") })
            .find(|t| !taken.contains(t))
            .unwrap_or_default();
        let made = ui.call("design.create", json!({"project_id": project, "title": title, "session": session})).await;
        creating(false);
        match made {
            Ok(v) => {
                if let Some(id) = v["design"]["id"].as_i64() {
                    if ui.project.get() != project {
                        ui.open_project(project, "design");
                    } else if !ui.editor.design_visible() {
                        ui.navigate("design");
                    }
                    open(&ui, id);
                }
            }
            Err(e) => ui.show_error(&format!("Could not start a design: {e}")),
        }
    });
}

pub(crate) fn register(b: &Rc<Board>) {
    BOARDS.with(|list| {
        let mut list = list.borrow_mut();
        list.retain(|w| w.strong_count() > 0);
        list.push(Rc::downgrade(b));
    });
}

/// Every open board, the main view's and the windows'.
fn boards() -> Vec<Rc<Board>> {
    BOARDS.with(|list| {
        let mut list = list.borrow_mut();
        list.retain(|w| w.strong_count() > 0);
        list.iter().filter_map(Weak::upgrade).collect()
    })
}

/// Every open board showing design `design_id`, the main view's and the windows' (the render job
/// says on them that an agent looked at it).
pub(crate) fn boards_showing(design_id: i64) -> Vec<Rc<Board>> {
    boards().into_iter().filter(|b| b.doc.id == design_id).collect()
}

/// TM-50: design `id` was deleted while words were being typed into it. Each board's edit ends
/// without committing (into a design that takes no edits now) and its words go on the
/// clipboard. Whether there were any.
pub(super) fn rescue(ui: &Ui, id: i64) -> bool {
    let mut typed = Vec::new();
    for b in boards_showing(id) {
        let text = b.edit.borrow().as_ref().map(|e| {
            let buffer = e.view.buffer();
            buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string()
        });
        b.end_edit(false);
        typed.extend(text.filter(|t| !t.trim().is_empty()));
    }
    if typed.is_empty() {
        return false;
    }
    ui.window.clipboard().set_text(&typed.join("\n\n"));
    true
}

/// What the person hears of design `title` deleted elsewhere: whether what they were typing
/// went to the clipboard, and whether edits of theirs were still on their way (TM-50).
fn deleted_note(title: &str, typed: bool, lost: bool) -> String {
    let mut note = format!("“{title}” was deleted");
    if lost {
        note.push_str(" before your last changes were saved");
    }
    if typed {
        note.push_str(". What you were typing is on the clipboard.");
    }
    note
}

/// `design.changed` / `design.deleted`, whatever project the main window shows: a design in a
/// window keeps its own.
pub fn event(ui: &Rc<Ui>, ev: &str, payload: &Value) {
    let id = payload["id"].as_i64().unwrap_or(0);
    let deleted = ev == "design.deleted" || payload["deleted_at"].is_string();
    // Edits still on their way to a design deleted elsewhere go with it (TM-50).
    let open = doc::find(id);
    let lost = deleted && open.as_ref().is_some_and(|d| d.busy());
    let title = open.map(|d| d.title()).or_else(|| payload["title"].as_str().map(String::from)).unwrap_or_default();
    doc::event(ev, payload);
    if deleted {
        let typed = rescue(ui, id);
        window::close(id);
        let on_board = view().is_some_and(|view| view.board.borrow().as_ref().is_some_and(|b| b.doc.id == id));
        match view().filter(|_| on_board) {
            Some(view) => {
                close_board(&view);
                // Deleted elsewhere (another window, an agent): say so, and offer it back while
                // it is only in Deleted.
                let gallery = Rc::downgrade(&view.gallery);
                // Restored, it opens on the board again (E-gallery-4).
                let restore: Option<Rc<dyn Fn()>> = (ev != "design.deleted").then(|| {
                    Rc::new(move || {
                        if let Some(g) = gallery.upgrade() {
                            g.restore(id, Origin::Board);
                        }
                    }) as Rc<dyn Fn()>
                });
                view.gallery.toast_with(&deleted_note(&title, typed, lost), "Restore", restore);
            }
            // Only in a window, which has closed: the main window says what went with it.
            None if typed || lost => ui.show_info(&deleted_note(&title, typed, lost)),
            None => {}
        }
    }
    // Only the designs: no terminal changed (TM-40).
    if let Some(view) = view() {
        if payload["project_id"].as_i64() == Some(view.gallery.project.get()) {
            gallery_refresh(ui, &view, false);
        }
    }
}

/// Something about a design changed from the board: the gallery reads again when it shows.
pub(crate) fn gallery_changed(ui: &Rc<Ui>) {
    if let Some(view) = view() {
        gallery_refresh(ui, &view, false);
    }
}

/// The copy opens where the person is: in a window of its own from a window (TM-49), else on
/// the main view's board; either says so.
fn duplicate_design(b: &Board) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let (id, project, windowed) = (b.doc.id, b.doc.design.borrow().project_id, b.windowed);
    let source = Rc::downgrade(&b.me());
    glib::spawn_future_local(async move {
        match ui.call("design.duplicate", json!({"design_id": id})).await {
            Ok(v) => {
                let Some(new) = v["id"].as_i64() else {
                    return;
                };
                if windowed {
                    window::open(&ui, project, new);
                    if let Some(b) = source.upgrade() {
                        b.toast("Opened the copy in a new window");
                    }
                } else {
                    // The board swaps to the copy, and says so by its name (E-gallery-5).
                    let note = format!("Now editing the copy “{}”", crate::app::text(&v, "title"));
                    open_on(&ui, new, Some(Box::new(move |b: &Rc<Board>| b.toast(&note))));
                }
            }
            Err(e) => ui.show_error(&e.to_string()),
        }
    });
}

/// Wait until what `doc` has queued is answered, at most five seconds.
async fn drained(doc: &Doc) {
    for _ in 0..100 {
        if !doc.busy() {
            return;
        }
        glib::timeout_future(Duration::from_millis(50)).await;
    }
}

/// TM-50: the words being typed go in, and the edits queued go out, before the design goes:
/// sent after the delete, the engine would refuse them.
fn delete_design(b: &Board) {
    b.end_edit(true);
    let doc = b.doc.clone();
    let (id, title, project) = (doc.id, doc.title(), doc.design.borrow().project_id);
    let Some(view) = view() else {
        return;
    };
    // The window closes as the design goes; the main window says so, with an Undo that opens
    // it in a window again (E-gallery-3, E-gallery-4).
    let origin = if b.windowed { Origin::Window(project) } else { Origin::Board };
    if !b.windowed {
        close_board(&view);
    }
    glib::spawn_future_local(async move {
        drained(&doc).await;
        view.gallery.delete(id, &title, origin);
    });
}

/// Relay is connected again: every open design is read again, since events were missed
/// meanwhile and the engine may hold an older copy (a store restored), and the gallery too
/// (TM-54).
pub(crate) fn reconnected(ui: &Rc<Ui>) {
    doc::reread_all();
    if let Some(view) = view() {
        gallery_refresh(ui, &view, true);
    }
}

/// Before the main window closes: every board's words in progress are committed, and while any
/// design still has edits on their way, the refusal to show (TM-39).
pub(crate) fn unsaved() -> Option<String> {
    let boards = boards();
    for b in &boards {
        b.end_edit(true);
    }
    boards.iter().any(|b| b.doc.busy()).then(|| "Designs are still saving. Close again in a moment.".to_string())
}

thread_local! {
    /// The person agreed to lose the design edits `unsent` asked about: set for the close that
    /// answer makes (`close_anyway`) and no longer.
    static LOSE_UNSENT: Cell<bool> = const { Cell::new(false) };
}

/// Before the main window closes, after `unsaved`: the question to ask when a design holds edits
/// that have not reached Relay (held back while it is not connected, or behind a refusal), which
/// closing loses. Asked, not refused: while Relay is down the edits cannot go, and a refusal
/// would keep the window open for as long as it stays down. `None` once the person agreed
/// (`close_anyway`), for that close only.
pub(crate) fn unsent() -> Option<(String, &'static str)> {
    if LOSE_UNSENT.with(Cell::get) {
        return None;
    }
    // A design open on several boards is one design (B28).
    let mut seen = std::collections::HashSet::new();
    let (mut edits, mut refused) = (0, false);
    for b in boards() {
        if seen.insert(b.doc.id) {
            edits += b.doc.unsent();
            refused |= b.doc.unsaved();
        }
    }
    unsent_text(edits, refused)
}

/// The person said to close and lose what `unsent` asked about: `close` runs without the
/// question. A close another check holds asks again next time.
pub(crate) fn close_anyway(close: impl FnOnce()) {
    LOSE_UNSENT.with(|l| l.set(true));
    close();
    LOSE_UNSENT.with(|l| l.set(false));
}

/// What closing asks, and its detail, of `edits` design edits held back, and a refusal
/// (`refused`: the engine refused an edit and nothing was saved since).
fn unsent_text(edits: usize, refused: bool) -> Option<(String, &'static str)> {
    const WAIT: &str = "Keep Relay open and they go out once it is connected again.";
    match (edits, refused) {
        (0, false) => None,
        (0, true) => Some(("A design edit was refused, and nothing has been saved since. Close anyway?".into(), "The design shows “Not saved” until its next edit is.")),
        (1, _) => Some(("1 design edit hasn't reached Relay yet. Close and lose it?".into(), "Keep Relay open and it goes out once it is connected again.")),
        (n, _) => Some((format!("{n} design edits haven't reached Relay yet. Close and lose them?"), WAIT)),
    }
}

/// Every design window closed, each board's words in progress committed first: the main window
/// is closing (TM-39).
pub(crate) fn close_windows() {
    window::close_all();
}

/// `session.changed`: the lamp of each board whose design belongs to that terminal follows its
/// state (E-ask-3).
pub(crate) fn session_changed(_ui: &Rc<Ui>, payload: &Value) {
    let (Some(name), Some(state)) = (payload["name"].as_str(), payload["state"].as_str()) else {
        return;
    };
    let project = payload["project_id"].as_i64();
    if let Some(view) = view().filter(|v| project.is_none_or(|p| p == v.gallery.project.get())) {
        view.gallery.session_changed(name, state);
    }
    for b in boards() {
        let ours = {
            let design = b.doc.design.borrow();
            design.session.as_deref() == Some(name) && project.is_none_or(|p| p == design.project_id)
        };
        if ours {
            b.set_lamp(state);
        }
    }
}

/// For the smoke capture: open the Design view on the first design, or the gallery.
/// `RELAY_SMOKE_DESIGN_ID` names the design to open instead of the gallery's first, which the
/// harness cannot order reliably (TM-58: an unchanged update no longer moves a design up).
pub fn smoke_open(ui: &Rc<Ui>) {
    ui.navigate("design");
    let ui = ui.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
        let named = std::env::var("RELAY_SMOKE_DESIGN_ID").ok().and_then(|id| id.parse::<i64>().ok());
        let first = named.or_else(|| view().and_then(|v| v.gallery_first()));
        if let Some(id) = first.filter(|_| std::env::var("RELAY_SMOKE_DESIGN_BOARD").is_ok()) {
            // Once the board has the design and its size, and a few frames have drawn: a scripted
            // run of gestures and commands (drive.rs), checked by the harness, or a layer to show
            // selected, with its toolbar and the inspector.
            let script = std::env::var("RELAY_SMOKE_DESIGN_SCRIPT").ok();
            let node = std::env::var("RELAY_SMOKE_DESIGN_SELECT").ok();
            open_with(&ui, id, move |b| {
                let b = b.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || match (script, node) {
                    (Some(script), _) => drive::run(b, script),
                    (None, Some(node)) => b.select(vec![node]),
                    (None, None) => {}
                });
            });
        }
    });
}

impl View {
    fn gallery_first(&self) -> Option<i64> {
        self.gallery.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// B79: of two opens under way, only the last asked for shows its design, and none does once
    /// the person went elsewhere.
    #[test]
    fn only_the_last_open_asked_for_shows() {
        let opening = Opening::default();
        let a = opening.begin();
        let b = opening.begin();
        assert!(!opening.current(a) && opening.current(b), "A answering last does not replace B");
        opening.cancel();
        assert!(!opening.current(b), "back to the gallery, or another project");
        assert!(opening.current(opening.begin()));
    }

    /// Closing asks before it loses edits held back from Relay, and says nothing when none are.
    #[test]
    fn closing_asks_before_it_loses_edits_relay_has_not_had() {
        assert_eq!(unsent_text(0, false), None);
        let ask = |edits, refused| unsent_text(edits, refused).map(|(q, _)| q);
        assert_eq!(ask(1, true).as_deref(), Some("1 design edit hasn't reached Relay yet. Close and lose it?"));
        assert_eq!(ask(3, false).as_deref(), Some("3 design edits haven't reached Relay yet. Close and lose them?"));
        assert!(ask(0, true).is_some_and(|t| t.contains("refused")));
        // Agreed once, the close under way goes; the next asks again.
        close_anyway(|| assert!(LOSE_UNSENT.with(Cell::get) && unsent().is_none()));
        assert!(!LOSE_UNSENT.with(Cell::get));
    }

    /// TM-50: what the person hears of a design deleted elsewhere.
    #[test]
    fn a_design_deleted_elsewhere_says_what_went_with_it() {
        assert_eq!(deleted_note("Home", false, false), "“Home” was deleted");
        assert_eq!(deleted_note("Home", true, false), "“Home” was deleted. What you were typing is on the clipboard.");
        assert_eq!(deleted_note("Home", false, true), "“Home” was deleted before your last changes were saved");
        assert_eq!(
            deleted_note("Home", true, true),
            "“Home” was deleted before your last changes were saved. What you were typing is on the clipboard."
        );
    }
}
