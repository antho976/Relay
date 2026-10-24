//! The Notes window: a persistent shell (toolbar, library sidebar, tabbed editors) that
//! survives refreshes, built around one `doc::Doc` per open note.
#[path = "note_pages/doc.rs"]
mod doc;
#[path = "note_pages/glyphs.rs"]
mod glyphs;
#[path = "note_pages/inline.rs"]
mod inline;
#[path = "note_pages/menu.rs"]
mod menu;
#[path = "note_pages/text.rs"]
mod text;

use super::task_pages::{buffer_text, Draft};
use super::*;
use crate::editor::readable;
use doc::Doc;
use glyphs::{glyph, tool_glyph};
use std::cell::{Cell, OnceCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// View and editing preferences, persisted with the window in `native.notes.window`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Prefs {
    pub autosave: bool,
    pub wrap: bool,
    pub line_numbers: bool,
    pub current_line: bool,
    pub toolbar: bool,
    pub monospace: bool,
    pub markdown: bool,
    pub zoom: u16,
    pub sort_title: bool,
    /// Toolbar keys show their names beside their icons.
    pub toolbar_labels: bool,
    /// Toolbar keys taken off the bar: bit `i` is `TOOLS[i]`.
    pub toolbar_hidden: u32,
}
impl Default for Prefs {
    fn default() -> Self {
        // Autosave is opt-in: D72 notes save explicitly unless the person turns it on.
        Self {
            autosave: false,
            wrap: true,
            line_numbers: true,
            current_line: true,
            toolbar: true,
            monospace: true,
            markdown: true,
            zoom: 100,
            sort_title: false,
            toolbar_labels: false,
            toolbar_hidden: 0,
        }
    }
}
impl Prefs {
    fn to_json(self) -> Value {
        json!({"autosave":self.autosave,"wrap":self.wrap,"line_numbers":self.line_numbers,
            "current_line":self.current_line,"toolbar":self.toolbar,"monospace":self.monospace,
            "markdown":self.markdown,"zoom":self.zoom,"sort":if self.sort_title {"title"} else {"modified"},
            "toolbar_labels":self.toolbar_labels,
            "toolbar_hidden":TOOLS.iter().enumerate().filter(|(i, _)| self.toolbar_hidden & (1 << i) != 0).map(|(_, t)| t.id).collect::<Vec<_>>()})
    }
    fn from_json(value: &Value) -> Self {
        let d = Self::default();
        let flag = |key: &str, fallback: bool| value[key].as_bool().unwrap_or(fallback);
        Self {
            autosave: flag("autosave", d.autosave),
            wrap: flag("wrap", d.wrap),
            line_numbers: flag("line_numbers", d.line_numbers),
            current_line: flag("current_line", d.current_line),
            toolbar: flag("toolbar", d.toolbar),
            monospace: flag("monospace", d.monospace),
            markdown: flag("markdown", d.markdown),
            zoom: value["zoom"].as_u64().map_or(d.zoom, |z| z.clamp(50, 300) as u16),
            sort_title: value["sort"] == "title",
            toolbar_labels: flag("toolbar_labels", d.toolbar_labels),
            toolbar_hidden: rows(value, "toolbar_hidden").iter().filter_map(Value::as_str).fold(0, |mask, id| {
                mask | TOOLS.iter().position(|t| t.id == id).map_or(0, |i| 1 << i)
            }),
        }
    }
}

/// One toolbar key. `id` is its `notes.*` action (Heading opens a menu instead), persisted in
/// `toolbar_hidden`; `icon` is a glyph, or a text mark such as "B" when it starts with `#`.
pub(super) struct Tool {
    pub id: &'static str,
    icon: &'static str,
    pub caption: &'static str,
    tip: &'static str,
    group: u8,
}

const fn t(id: &'static str, icon: &'static str, caption: &'static str, tip: &'static str, group: u8) -> Tool {
    Tool { id, icon, caption, tip, group }
}

pub(super) const TOOLS: [Tool; 21] = [
    t("new", "note-new", "New", "New note (Ctrl+N)", 0),
    t("save", "save", "Save", "Save (Ctrl+S)", 0),
    t("undo", "undo", "Undo", "Undo (Ctrl+Z)", 1),
    t("redo", "redo", "Redo", "Redo (Ctrl+Shift+Z)", 1),
    t("cut", "cut", "Cut", "Cut (Ctrl+X)", 2),
    t("copy", "copy", "Copy", "Copy (Ctrl+C)", 2),
    t("paste", "paste", "Paste", "Paste (Ctrl+V)", 2),
    t("find", "search", "Find", "Find (Ctrl+F)", 3),
    t("replace", "replace", "Replace", "Replace (Ctrl+H)", 3),
    t("bold", "#B", "Bold", "Bold (Ctrl+B)", 4),
    t("italic", "#I", "Italic", "Italic (Ctrl+I)", 4),
    t("heading", "#H", "Heading", "Heading", 4),
    t("bullets", "list", "Bullets", "Bulleted list", 4),
    t("numbers", "list-numbered", "Numbers", "Numbered list", 4),
    t("checklist", "checklist", "Checklist", "Checklist (Ctrl+Enter ticks an item)", 4),
    t("quote", "quote", "Quote", "Quote", 4),
    t("code", "code-inline", "Code", "Inline code", 4),
    t("code-block", "code-block", "Code block", "Code block", 4),
    t("link", "link", "Link", "Link (Ctrl+K)", 4),
    t("image", "image", "Image", "Insert an image (or paste or drop one)", 5),
    t("to-task", "task", "To task", "Create a task from this note, or from the selected text", 5),
];

/// The toolbar group of formatting keys (bold to link), which never show names.
const FORMAT_GROUP: u8 = 4;

/// A toolbar key and the name shown under its icon when labels are on.
struct ToolKey {
    key: gtk::Widget,
    caption: gtk::Label,
}

/// One library row. `note` is the listed copy (`notes.list {summary}`: the body cut to a
/// preview); the name, preview and search text come from the whole note once it is at hand.
struct Row {
    row: gtk::ListBoxRow,
    note: Value,
    /// Title and body, lowercased once when the row is built: the search filter reads it on
    /// every keystroke, for every row.
    haystack: String,
    dot: gtk::Box,
    heading: gtk::Label,
    date: gtk::Label,
    shown: gtk::Label,
    name: String,
    preview: String,
    /// Built from the whole note, not just its preview.
    whole: bool,
}

struct Shell {
    root: gtk::Box,
    toolbar: gtk::Box,
    tools: Vec<ToolKey>,
    /// The line before each group of keys but the first, by group.
    tool_lines: Vec<gtk::Separator>,
    customize: gtk::Popover,
    customize_checks: Vec<gtk::CheckButton>,
    customize_labels: gtk::Switch,
    picker: gtk::Box,
    split: gtk::Paned,
    library: gtk::Box,
    search: gtk::SearchEntry,
    list: gtk::ListBox,
    list_scroll: gtk::ScrolledWindow,
    placeholder: gtk::Label,
    row_menu: gtk::PopoverMenu,
    banner: gtk::Box,
    banner_text: gtk::Label,
    /// Names unsaved notes of projects not on screen; its Show key opens `elsewhere_target`.
    elsewhere: gtk::Box,
    elsewhere_text: gtk::Label,
    elsewhere_target: Cell<i64>,
    welcome: gtk::Box,
    rows: RefCell<Vec<Row>>,
    notes: RefCell<Vec<Value>>,
    /// Some row is pinned, so the list shows Pinned / Notes headers.
    any_pinned: Cell<bool>,
    /// What the project picker was last built from; it is rebuilt only when that changes.
    picker_key: RefCell<String>,
    project: Cell<i64>,
    rail_applied: Cell<bool>,
}

thread_local! {
    /// A note to bring forward once its project's library is shown.
    static FOCUS: Cell<i64> = const { Cell::new(0) };
    static SHELL: RefCell<Option<Rc<Shell>>> = const { RefCell::new(None) };
    static DOCS: RefCell<BTreeMap<i64, Rc<Doc>>> = const { RefCell::new(BTreeMap::new()) };
    static PREFS: Cell<Prefs> = Cell::new(Prefs::default());
    static SESSION: RefCell<serde_json::Map<String, Value>> = RefCell::new(serde_json::Map::new());
    static RESTORED: RefCell<BTreeSet<i64>> = const { RefCell::new(BTreeSet::new()) };
    /// The last search a note's find bar ran, its options with it, which every note's bar
    /// starts from.
    static LAST_QUERY: RefCell<crate::editor::find_bar::Memory> =
        const { RefCell::new(crate::editor::find_bar::Memory::new()) };
    static FONT: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static TASKS_TIMER: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
    static SCHEME: OnceCell<Option<sourceview5::StyleScheme>> = const { OnceCell::new() };
    /// Whole notes by id, as `notes.get` returned them: the library lists only previews. A
    /// copy is current while its `updated_at` matches the listed one, as every change sets it.
    static FULL: RefCell<HashMap<i64, Value>> = RefCell::new(HashMap::new());
    /// The library is fetching whole notes in the background; `FILL_AGAIN` asks it to look
    /// again once it is done.
    static FILLING: Cell<bool> = const { Cell::new(false) };
    static FILL_AGAIN: Cell<bool> = const { Cell::new(false) };
    /// A whole note changed a row's search text while a search was shown.
    static REFILTER: Cell<bool> = const { Cell::new(false) };
    /// The day the row dates were last written for ("14:02" today, "Oct 5" after).
    static DATED: Cell<(i32, i32)> = const { Cell::new((0, 0)) };
}

/// `notes.list {summary}` cuts each body to this many characters (relay-core's
/// `PREVIEW_CHARS`); a shorter one is the whole body.
const PREVIEW_CHARS: usize = 240;

fn note_id(note: &Value) -> i64 {
    note["id"].as_i64().unwrap_or(0)
}

/// The listed copy is the whole note: its body was not cut.
fn complete(listed: &Value) -> bool {
    text(listed, "body").chars().count() < PREVIEW_CHARS
}

/// The whole of the listed note is at hand, without asking the engine.
fn has_full(listed: &Value) -> bool {
    complete(listed)
        || FULL.with(|full| {
            full.borrow()
                .get(&note_id(listed))
                .is_some_and(|f| f["updated_at"] == listed["updated_at"])
        })
}

/// The whole stored note behind a listed one, if it is at hand.
fn full_note(listed: &Value) -> Option<Value> {
    if complete(listed) {
        return Some(listed.clone());
    }
    FULL.with(|full| {
        full.borrow()
            .get(&note_id(listed))
            .filter(|f| f["updated_at"] == listed["updated_at"])
            .cloned()
    })
}

/// Call `f` with the listed note's title and the best body at hand (the whole one when known),
/// and whether that body is whole.
fn with_display<R>(listed: &Value, f: impl FnOnce(&str, &str, bool) -> R) -> R {
    if complete(listed) {
        return f(text(listed, "title"), text(listed, "body"), true);
    }
    FULL.with(|full| {
        let full = full.borrow();
        match full.get(&note_id(listed)).filter(|n| n["updated_at"] == listed["updated_at"]) {
            Some(whole) => f(text(listed, "title"), text(whole, "body"), true),
            None => f(text(listed, "title"), text(listed, "body"), false),
        }
    })
}

/// Read note `id` whole and keep it for the library.
pub(super) async fn fetch_full(ui: &Rc<Ui>, id: i64) -> Result<Value, crate::client::Error> {
    let note = ui.call("notes.get", json!({"note_id":id})).await?;
    FULL.with(|full| full.borrow_mut().insert(id, note.clone()));
    Ok(note)
}

/// The listed notes `workspace` will hand to an editor when it renders `notes` for `project`
/// (open tabs it reconciles, remembered tabs it restores), whose whole text is not at hand.
pub(super) fn needs_full(project: i64, notes: &[Value]) -> Vec<i64> {
    let missing = |id: i64| notes.iter().find(|n| note_id(n) == id).filter(|n| !has_full(n)).map(|_| id);
    let mine: Vec<i64> = docs().iter().filter(|d| d.project == project).map(|d| d.id).collect();
    let mut ids: Vec<i64> = mine.iter().copied().filter_map(missing).collect();
    if !RESTORED.with(|r| r.borrow().contains(&project)) {
        match SESSION.with(|s| s.borrow().get(&project.to_string()).cloned()) {
            Some(entry) => ids.extend(
                rows(&entry, "open")
                    .iter()
                    .filter_map(Value::as_i64)
                    .filter(|id| doc_by_id(*id).is_none())
                    .filter_map(missing),
            ),
            None if mine.is_empty() => ids.extend(notes.first().and_then(|n| missing(note_id(n)))),
            None => {}
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Run `then` with note `id`'s whole stored copy: at once when it is at hand, else once
/// `notes.get` has read it.
fn with_full(ui: &Rc<Ui>, id: i64, then: impl FnOnce(&Rc<Ui>, Value) + 'static) {
    let Some(listed) = listed(id) else { return };
    if let Some(note) = full_note(&listed) {
        then(ui, note);
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        match fetch_full(&ui, id).await {
            Ok(note) => then(&ui, note),
            Err(error) => shell_error(&ui, &format!("Could not read the note: {}", readable(&error))),
        }
    });
}

/// Read, in the background, every listed note whose whole text is not at hand, so the
/// search finds words past each preview and every row names and previews its note in full.
pub(super) fn fill(ui: &Rc<Ui>) {
    if FILLING.with(|f| f.replace(true)) {
        FILL_AGAIN.with(|a| a.set(true));
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        loop {
            FILL_AGAIN.with(|a| a.set(false));
            let wanted: Vec<i64> = shell_if_built().map_or_else(Vec::new, |shell| {
                shell.notes.borrow().iter().filter(|n| !has_full(n)).map(note_id).collect()
            });
            for id in wanted {
                // Listed still, and still not whole: the library may have moved on.
                if !listed(id).is_some_and(|n| !has_full(&n)) {
                    continue;
                }
                match fetch_full(&ui, id).await {
                    Ok(_) => full_arrived(&ui, id),
                    Err(crate::client::Error::Disconnected) => break,
                    Err(_) => {}
                }
            }
            if !FILL_AGAIN.with(Cell::get) {
                break;
            }
        }
        FILLING.with(|f| f.set(false));
        if REFILTER.with(|r| r.replace(false)) {
            if let Some(shell) = shell_if_built() {
                shell.list.invalidate_filter();
                update_placeholder(&shell);
            }
        }
    });
}

/// Note `id`'s whole text arrived: its row searches all of it, and shows what it changed.
fn full_arrived(ui: &Rc<Ui>, id: i64) {
    let Some(shell) = shell_if_built() else { return };
    let Some(index) = shell.rows.borrow().iter().position(|r| note_id(&r.note) == id) else { return };
    let listed = shell.rows.borrow()[index].note.clone();
    if shell.rows.borrow()[index].whole || !has_full(&listed) {
        return;
    }
    let searching = !shell.search.text().trim().is_empty();
    let renamed = {
        let mut rows = shell.rows.borrow_mut();
        let name = rows[index].name.clone();
        rebind(&mut rows[index], &listed);
        rows[index].name != name
    };
    // The name sorts the list.
    if renamed && prefs().sort_title {
        render_rows(ui, &shell);
    }
    if searching {
        REFILTER.with(|r| r.set(true));
    }
}

/// A dark ink for text on a search match's yellow. The paper itself is `@notes_paper`, a
/// terminal's ground per appearance mode (shell.rs).
const NOTES_PAPER: &str = "#08090a";

const NOTES_SCHEME: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<!-- Relay Notes: the Code editor's matte palette, with quieter Markdown. -->
<style-scheme id="relay-notes" name="Relay Notes" version="1.0" parent-scheme="Adwaita-dark">
  <!-- The text view is clear: css/notes.css paints the paper under it, translucent over the
       wallpaper, so the scheme's own backgrounds are transparent and the current line a tint. -->
  <style name="text" foreground="#dcdcda" background="#00000000"/>
  <style name="selection" background="#3a3a3f"/>
  <style name="cursor" foreground="#ececea"/>
  <style name="current-line" background="#ffffff08"/>
  <style name="line-numbers" foreground="#47474c" background="#00000000"/>
  <style name="current-line-number" foreground="#a5a5a3" background="#00000000"/>
  <style name="search-match" foreground="NOTES_PAPER" background="#e0b04a"/>
  <style name="def:heading" foreground="#f4f4f2" bold="true"/>
  <style name="def:emphasis" italic="true"/>
  <style name="def:strong-emphasis" bold="true"/>
  <style name="def:list-marker" foreground="#e0b04a" bold="true"/>
  <style name="def:inline-code" foreground="#d9b46a" background="#17171a"/>
  <style name="def:preformatted-section" foreground="#c9c4b4"/>
  <style name="def:link-text" foreground="#7aa7f0" underline="single"/>
  <style name="def:link-destination" foreground="#6e6e72"/>
  <style name="def:link-symbol" foreground="#6e6e72"/>
  <style name="def:shebang" foreground="#a5a5a3" italic="true"/>
  <style name="def:thematic-break" foreground="#5a5a60"/>
  <style name="def:special-char" foreground="#77777a"/>
  <style name="def:comment" foreground="#6e6e70"/>
  <style name="def:string" foreground="#e0b04a"/>
  <style name="def:keyword" foreground="#c979d6"/>
  <!-- As in resources/relay-editor.xml (RA-530): Adwaita-dark styles some languages' tokens by
       their own ids, which a lookup finds before the def:* style the language maps them to.
       Own every def:* style those point at, then point each language id back at it. -->
  <style name="def:statement" foreground="#c979d6"/>
  <style name="def:preprocessor" foreground="#c979d6"/>
  <style name="def:type" foreground="#3ec5cf"/>
  <style name="def:function" foreground="#5b9cf6"/>
  <style name="def:character" foreground="#e0b04a"/>
  <style name="def:number" foreground="#f2cf6b"/>
  <style name="def:operator" foreground="#a5a5a3"/>
  <style name="def:identifier" foreground="#dcdcda"/>
  <style name="def:doc-comment" foreground="#6e6e70"/>
  <style name="def:doc-comment-element" use-style="def:doc-comment"/>
  <style name="def:error" foreground="#e5382e"/>
  <style name="def:constant" use-style="def:number"/>
  <style name="def:special-constant" use-style="def:number"/>
  <style name="def:net-address" foreground="#7aa7f0"/>
  <style name="def:note" foreground="#e0b04a" bold="true"/>
  <style name="rust:attribute" use-style="def:preprocessor"/>
  <style name="rust:macro" use-style="def:preprocessor"/>
  <style name="rust:scope" use-style="def:preprocessor"/>
  <style name="rust:lifetime" use-style="def:keyword"/>
  <style name="c:printf" use-style="def:special-char"/>
  <style name="c:signal-name" use-style="def:constant"/>
  <style name="c:storage-class" use-style="def:type"/>
  <style name="c:type-keyword" use-style="def:keyword"/>
  <style name="c-sharp:format" use-style="def:special-char"/>
  <style name="c-sharp:preprocessor" use-style="def:preprocessor"/>
  <style name="go:printf" use-style="def:special-char"/>
  <style name="vala:attributes" use-style="def:function"/>
  <style name="python:builtin-function" use-style="def:type"/>
  <style name="python:class-name" use-style="def:function"/>
  <style name="python:module-handler" use-style="def:preprocessor"/>
  <style name="css:id-selector" use-style="def:statement"/>
  <style name="css:property-name" use-style="def:keyword"/>
  <style name="css:pseudo-selector" use-style="def:function"/>
  <style name="css:selector-symbol" use-style="def:operator"/>
  <style name="css:type-selector" use-style="def:type"/>
  <style name="css:vendor-specific" use-style="def:keyword"/>
  <style name="xml:attribute-name" use-style="def:type"/>
  <style name="xml:attribute-value" use-style="def:string"/>
  <style name="xml:element-name" use-style="def:identifier"/>
  <style name="xml:namespace" use-style="def:identifier"/>
  <style name="xml:processing-instruction" use-style="def:preprocessor"/>
  <style name="diff:added-line" foreground="#2ec469"/>
  <style name="diff:removed-line" foreground="#e5382e"/>
  <style name="diff:changed-line" foreground="#f0a828"/>
  <style name="diff:location" use-style="def:function"/>
  <style name="diff:diff-file" use-style="def:keyword"/>
</style-scheme>
"##;

pub(super) fn scheme() -> Option<sourceview5::StyleScheme> {
    SCHEME.with(|cell| {
        cell.get_or_init(|| {
            let manager = sourceview5::StyleSchemeManager::default();
            let directory = glib::user_cache_dir().join("relay-v4/notes-styles");
            let path = directory.join("relay-notes.xml");
            let scheme = NOTES_SCHEME.replace("NOTES_PAPER", NOTES_PAPER);
            let written = std::fs::create_dir_all(&directory).and_then(|_| {
                if std::fs::read_to_string(&path).ok().as_deref() == Some(scheme.as_str()) {
                    Ok(())
                } else {
                    std::fs::write(&path, &scheme)
                }
            });
            match written {
                Ok(()) => manager.append_search_path(&directory.to_string_lossy()),
                Err(error) => tracing::warn!(%error, "Could not install the Notes colour scheme"),
            }
            manager
                .scheme("relay-notes")
                .or_else(|| manager.scheme("relay-matte"))
                .or_else(|| manager.scheme("Adwaita-dark"))
        })
        .clone()
    })
}

pub(super) fn prefs() -> Prefs {
    PREFS.with(Cell::get)
}

fn docs() -> Vec<Rc<Doc>> {
    DOCS.with(|docs| docs.borrow().values().cloned().collect())
}

fn doc_by_id(id: i64) -> Option<Rc<Doc>> {
    DOCS.with(|docs| docs.borrow().get(&id).cloned())
}

/// The Notes preferences, for the smoke harness.
pub fn notes_prefs() -> Prefs {
    prefs()
}

pub fn set_notes_prefs(ui: &Rc<Ui>, next: Prefs) {
    set_prefs(ui, next);
}

/// Close note `id`'s tab without saving it.
pub fn discard_note(ui: &Rc<Ui>, id: i64) {
    if let Some(doc) = doc_by_id(id) {
        doc::discard_close(ui, &doc);
    }
}

/// Why the main window stays open for Notes, if it does: some open note is saving or has
/// unsaved changes. Autosave saves what it can first, and a note still unsaved comes forward
/// in Notes, named in the answer.
pub fn unsaved_notes(ui: &Rc<Ui>) -> Option<String> {
    flush(ui);
    let doc = docs().into_iter().find(|doc| doc.draft.busy.get() || doc.dirty())?;
    let name = doc.name();
    if doc.draft.busy.get() {
        return Some(format!("Notes is saving “{name}”. Close again once it is saved."));
    }
    reveal(ui, doc.id);
    Some(format!("“{name}” has unsaved changes in Notes. Save or discard them, then close again."))
}

/// The editor of note `id` while its tab is open.
pub fn open_draft(id: i64) -> Option<Rc<Draft>> {
    doc_by_id(id).map(|doc| doc.draft.clone())
}

/// The project whose library Notes shows; 0 before the first render or once it is removed.
pub fn shown_project() -> i64 {
    shell_if_built().map_or(0, |shell| shell.project.get())
}

/// Close every open note, as the main window closes.
pub fn close_all_notes() {
    for doc in docs() {
        doc.draft.close();
    }
}

fn owner(ui: &Rc<Ui>) -> Option<Rc<NotesWindow>> {
    ui.notes_window.borrow().clone()
}

pub(super) fn window_of(ui: &Rc<Ui>) -> Option<gtk::Window> {
    owner(ui).map(|owner| owner.window.clone())
}

fn persist(ui: &Rc<Ui>) {
    if let Some(owner) = owner(ui) {
        owner.persist(ui);
    }
}

pub(super) fn project_name(ui: &Rc<Ui>, project: i64) -> String {
    ui.projects
        .borrow()
        .iter()
        .find(|p| p["id"].as_i64() == Some(project))
        .map(|p| text::clean(text(p, "name")).into_owned())
        .unwrap_or_else(|| format!("Project {project}"))
}

/// "14:02" today, "Oct 5" this year, "Oct 5, 2025" before.
pub(super) fn fmt_date(iso: &str) -> String {
    let Some(date) = glib::DateTime::from_iso8601(iso, None).ok().and_then(|d| d.to_local().ok()) else {
        return String::new();
    };
    let now = glib::DateTime::now_local().ok();
    let format = match &now {
        Some(now) if now.year() == date.year() && now.day_of_year() == date.day_of_year() => "%H:%M",
        Some(now) if now.year() == date.year() => "%b %-d",
        _ => "%b %-d, %Y",
    };
    date.format(format).map(|s| s.to_string()).unwrap_or_default()
}

fn apply_font(prefs: &Prefs) {
    let css = format!(
        "textview.notes-source {{ font-family: {}; font-size: {:.1}px; }}",
        if prefs.monospace { "'Fira Mono', monospace" } else { "'Fira Sans', sans-serif" },
        13.0 * f64::from(prefs.zoom) / 100.0
    );
    FONT.with(|font| {
        let mut font = font.borrow_mut();
        if font.is_none() {
            if let Some(display) = gtk::gdk::Display::default() {
                let provider = gtk::CssProvider::new();
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
                );
                *font = Some(provider);
            }
        }
        if let Some(provider) = font.as_ref() {
            provider.load_from_string(&css);
        }
    });
}

fn apply_prefs(ui: &Rc<Ui>, prefs: &Prefs, rows: bool) {
    apply_font(prefs);
    for doc in docs() {
        doc.apply_prefs(prefs);
        inline::schedule(&doc);
    }
    if let Some(shell) = shell_if_built() {
        shell.toolbar.set_visible(prefs.toolbar);
        apply_toolbar(&shell, prefs);
        if rows {
            render_rows(ui, &shell);
        }
    }
}

pub(super) fn set_prefs(ui: &Rc<Ui>, next: Prefs) {
    let before = prefs();
    if before == next {
        return;
    }
    PREFS.with(|p| p.set(next));
    apply_prefs(ui, &next, before.sort_title != next.sort_title);
    // Either way: re-arming with autosave off cancels a timer already running.
    if next.autosave != before.autosave {
        for doc in docs() {
            doc.schedule_autosave(ui);
        }
    }
    persist(ui);
}

pub(super) fn zoom(ui: &Rc<Ui>, direction: i32) {
    let mut next = prefs();
    next.zoom = if direction == 0 { 100 } else { text::zoom_step(next.zoom, direction) };
    set_prefs(ui, next);
}

/// What the window persists besides its rail: preferences and open tabs per project.
pub(super) fn saved_state() -> Value {
    json!({"prefs": prefs().to_json(), "session": SESSION.with(|s| Value::Object(s.borrow().clone()))})
}

/// Settings arrived: adopt preferences and the remembered tabs, before the first render.
pub(super) fn restore_state(ui: &Rc<Ui>, value: &Value) {
    let restored = Prefs::from_json(&value["prefs"]);
    PREFS.with(|p| p.set(restored));
    if let Some(session) = value["session"].as_object() {
        SESSION.with(|s| *s.borrow_mut() = session.clone());
    }
    let sidebar = owner(ui).is_none_or(|o| !o.rail_collapsed.get());
    menu::sync_prefs(&restored, sidebar);
    apply_prefs(ui, &restored, true);
}

/// The window is hiding: with autosave on, nothing waits for the timer.
pub(super) fn flush(ui: &Rc<Ui>) {
    if !prefs().autosave {
        return;
    }
    for doc in docs() {
        if doc.dirty() && !doc.deleted.get() && !doc.conflict.get() {
            doc::save(ui, &doc, None);
        }
    }
}

/// Install the actions, shortcuts and menu bar on the Notes window.
pub(super) fn window_menu(ui: &Rc<Ui>, window: &gtk::Window) -> gtk::Widget {
    menu::install(ui, window).upcast()
}

fn shell_if_built() -> Option<Rc<Shell>> {
    SHELL.with(|s| s.borrow().clone())
}

fn current_project(ui: &Rc<Ui>) -> i64 {
    let shown = shell_if_built().map_or(0, |s| s.project.get());
    if shown != 0 {
        shown
    } else {
        owner(ui).map_or(ui.project.get(), |o| o.project.get())
    }
}

/// A small icon key outside the toolbar (the library's New note key).
fn tool(icon: &str, tip: &str, action: &str) -> gtk::Button {
    let key = gtk::Button::new();
    key.set_child(Some(&tool_glyph(icon, 16)));
    key.add_css_class("notes-tool");
    key.set_focus_on_click(false);
    key.set_tooltip_text(Some(tip));
    key.update_property(&[gtk::accessible::Property::Label(tip)]);
    key.set_action_name(Some(&format!("notes.{action}")));
    key
}

fn tool_key(tool: &Tool) -> ToolKey {
    // The name sits under the icon when labels are on.
    let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
    content.set_halign(gtk::Align::Center);
    match tool.icon.strip_prefix('#') {
        Some(mark) => {
            let mark = label(mark, "notes-tool-mark");
            mark.add_css_class(&format!("notes-tool-mark-{}", tool.id));
            content.append(&mark);
        }
        None => content.append(&tool_glyph(tool.icon, 16)),
    }
    let caption = label(tool.caption, "notes-tool-caption");
    caption.set_visible(false);
    content.append(&caption);
    let key: gtk::Widget = if tool.id == "heading" {
        let menu = gtk::MenuButton::new();
        menu.set_always_show_arrow(false);
        menu.add_css_class("notes-tool-menu");
        menu.set_menu_model(Some(&menu::headings_menu()));
        menu.set_child(Some(&content));
        menu.upcast()
    } else {
        let key = gtk::Button::new();
        key.add_css_class("notes-tool");
        key.set_focus_on_click(false);
        key.set_action_name(Some(&format!("notes.{}", tool.id)));
        key.set_child(Some(&content));
        key.upcast()
    };
    key.set_tooltip_text(Some(tool.tip));
    key.update_property(&[gtk::accessible::Property::Label(tool.caption)]);
    ToolKey { key, caption }
}

fn tool_separator() -> gtk::Separator {
    let line = gtk::Separator::new(gtk::Orientation::Vertical);
    line.add_css_class("notes-tool-sep");
    line
}

/// Show the keys the person kept, with or without names, and the line before each group
/// that still shows something (the library key always stands before the first).
fn apply_toolbar(shell: &Shell, prefs: &Prefs) {
    let mut groups = [false; 6];
    for (i, (tool, key)) in TOOLS.iter().zip(&shell.tools).enumerate() {
        let visible = prefs.toolbar_hidden & (1 << i) == 0;
        groups[usize::from(tool.group)] |= visible;
        key.key.set_visible(visible);
        // Formatting keys keep to their icons, as in a word processor: named, the bar would
        // outgrow the window.
        let named = prefs.toolbar_labels && tool.group != FORMAT_GROUP;
        key.caption.set_visible(named);
        if named {
            key.key.add_css_class("labelled");
        } else {
            key.key.remove_css_class("labelled");
        }
        if let Some(check) = shell.customize_checks.get(i) {
            if check.is_active() != visible {
                check.set_active(visible);
            }
        }
    }
    for (line, shows) in shell.tool_lines.iter().zip(groups) {
        line.set_visible(shows);
    }
    if shell.customize_labels.is_active() != prefs.toolbar_labels {
        shell.customize_labels.set_active(prefs.toolbar_labels);
    }
}

/// The toolbar's own settings: names under icons, and which keys it shows.
fn customize_popover(ui: &Rc<Ui>, toolbar: &gtk::Box) -> (gtk::Popover, Vec<gtk::CheckButton>, gtk::Switch) {
    let popover = gtk::Popover::new();
    popover.add_css_class("notes-customize");
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    popover.set_parent(toolbar);
    let owner = popover.downgrade();
    toolbar.connect_destroy(move |_| {
        if let Some(popover) = owner.upgrade() {
            popover.unparent();
        }
    });
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&label("Toolbar", "notes-customize-title"));
    let labels_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    labels_row.add_css_class("notes-customize-row");
    let caption = label("Show names under icons", "notes-customize-caption");
    caption.set_hexpand(true);
    labels_row.append(&caption);
    let labels = gtk::Switch::new();
    labels.set_valign(gtk::Align::Center);
    labels.set_active(prefs().toolbar_labels);
    labels_row.append(&labels);
    body.append(&labels_row);
    body.append(&label("Keys on the toolbar", "notes-customize-section"));
    let grid = gtk::Grid::new();
    grid.add_css_class("notes-customize-grid");
    grid.set_column_homogeneous(true);
    grid.set_column_spacing(4);
    let weak = Rc::downgrade(ui);
    let mut checks = Vec::new();
    let hidden = prefs().toolbar_hidden;
    for (i, tool) in TOOLS.iter().enumerate() {
        let check = gtk::CheckButton::new();
        check.add_css_class("notes-customize-check");
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        match tool.icon.strip_prefix('#') {
            Some(mark) => {
                let mark = label(mark, "notes-tool-mark");
                mark.add_css_class(&format!("notes-tool-mark-{}", tool.id));
                mark.set_width_chars(2);
                content.append(&mark);
            }
            None => content.append(&tool_glyph(tool.icon, 16)),
        }
        content.append(&label(tool.caption, "notes-customize-caption"));
        check.set_child(Some(&content));
        check.set_active(hidden & (1 << i) == 0);
        let target = weak.clone();
        check.connect_toggled(move |check| {
            let Some(ui) = target.upgrade() else { return };
            let mut next = prefs();
            if check.is_active() {
                next.toolbar_hidden &= !(1 << i);
            } else {
                next.toolbar_hidden |= 1 << i;
            }
            set_prefs(&ui, next);
        });
        grid.attach(&check, (i % 2) as i32, (i / 2) as i32, 1, 1);
        checks.push(check);
    }
    body.append(&grid);
    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    foot.add_css_class("notes-customize-foot");
    let hint = label("Right-click the toolbar to come back here.", "notes-customize-hint");
    hint.set_hexpand(true);
    foot.append(&hint);
    let reset = button("Show all", "notes-customize-reset");
    let target = weak.clone();
    reset.connect_clicked(move |_| {
        if let Some(ui) = target.upgrade() {
            let mut next = prefs();
            next.toolbar_hidden = 0;
            set_prefs(&ui, next);
        }
    });
    foot.append(&reset);
    body.append(&foot);
    let target = weak.clone();
    labels.connect_active_notify(move |switch| {
        if let Some(ui) = target.upgrade() {
            let mut next = prefs();
            next.toolbar_labels = switch.is_active();
            set_prefs(&ui, next);
            if let Some(action) = menu::lookup("toolbar-labels") {
                action.set_state(&next.toolbar_labels.to_variant());
            }
        }
    });
    popover.set_child(Some(&body));
    (popover, checks, labels)
}

/// Open the toolbar settings, under the pointer when right-clicked.
pub(super) fn customize_toolbar(ui: &Rc<Ui>, at: Option<(f64, f64)>) {
    let shell = shell(ui);
    if !shell.toolbar.is_visible() {
        let mut next = prefs();
        next.toolbar = true;
        set_prefs(ui, next);
        if let Some(action) = menu::lookup("toolbar") {
            action.set_state(&true.to_variant());
        }
    }
    let rect = match at {
        Some((x, y)) => gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1),
        None => gtk::gdk::Rectangle::new(shell.toolbar.width() / 2, shell.toolbar.height(), 1, 1),
    };
    shell.customize.set_pointing_to(Some(&rect));
    shell.customize.popup();
}

fn shell(ui: &Rc<Ui>) -> Rc<Shell> {
    if let Some(shell) = shell_if_built() {
        return shell;
    }
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("notes-shell");
    root.set_vexpand(true);

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    toolbar.add_css_class("notes-toolbar");
    let collapse = gtk::Button::new();
    collapse.set_child(Some(&tool_glyph("sidebar", 16)));
    collapse.add_css_class("notes-tool");
    collapse.set_focus_on_click(false);
    collapse.set_widget_name("notes-library-toggle");
    collapse.set_tooltip_text(Some("Show or hide the library (F9)"));
    collapse.update_property(&[gtk::accessible::Property::Label("Toggle library")]);
    toolbar.append(&collapse);
    let mut tools = Vec::new();
    let mut tool_lines = Vec::new();
    for tool in &TOOLS {
        if usize::from(tool.group) == tool_lines.len() {
            let line = tool_separator();
            toolbar.append(&line);
            tool_lines.push(line);
        }
        let key = tool_key(tool);
        toolbar.append(&key.key);
        tools.push(key);
    }
    let (customize, customize_checks, customize_labels) = customize_popover(ui, &toolbar);
    let right = gtk::GestureClick::new();
    right.set_button(3);
    let target = Rc::downgrade(ui);
    right.connect_pressed(move |gesture, _, x, y| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        if let Some(ui) = target.upgrade() {
            customize_toolbar(&ui, Some((x, y)));
        }
    });
    toolbar.add_controller(right);
    // Named keys can outgrow a narrow window: the bar scrolls sideways (wheel or touchpad)
    // instead of setting the window's minimum width.
    let toolbar_scroll = gtk::ScrolledWindow::new();
    toolbar_scroll.set_policy(gtk::PolicyType::External, gtk::PolicyType::Never);
    toolbar_scroll.add_css_class("notes-toolbar-scroll");
    toolbar_scroll.set_child(Some(&toolbar));
    let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let along = toolbar_scroll.hadjustment();
    wheel.connect_scroll(move |_, _, dy| {
        along.set_value(along.value() + dy * 40.0);
        glib::Propagation::Stop
    });
    toolbar_scroll.add_controller(wheel);
    root.append(&toolbar_scroll);

    let split = gtk::Paned::new(gtk::Orientation::Horizontal);
    split.set_widget_name("notes-library-split");
    split.add_css_class("notes-split");
    split.set_vexpand(true);
    split.set_resize_start_child(false);
    split.set_shrink_start_child(false);
    split.set_shrink_end_child(false);
    split.set_position(owner(ui).map_or(270, |o| o.rail_width.get()));

    let library = gtk::Box::new(gtk::Orientation::Vertical, 0);
    library.add_css_class("notes-library");
    library.set_size_request(180, -1);
    // The project the library shows, full width so its name is never cut short.
    let picker = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    picker.add_css_class("notes-picker");
    library.append(&picker);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    head.add_css_class("notes-library-head");
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search notes"));
    search.add_css_class("notes-library-search");
    search.set_hexpand(true);
    head.append(&search);
    let sort = gtk::MenuButton::new();
    sort.set_child(Some(&tool_glyph("sort", 16)));
    sort.add_css_class("notes-library-key");
    sort.set_tooltip_text(Some("Sort notes"));
    sort.set_menu_model(Some(&menu::sort_menu()));
    head.append(&sort);
    let add = tool("plus", "New note (Ctrl+N)", "new");
    add.add_css_class("notes-library-key");
    head.append(&add);
    library.append(&head);
    let list = gtk::ListBox::new();
    list.add_css_class("notes-list");
    list.set_selection_mode(gtk::SelectionMode::Single);
    let placeholder = label("", "notes-list-empty");
    placeholder.set_wrap(true);
    placeholder.set_justify(gtk::Justification::Center);
    placeholder.set_xalign(0.5);
    list.set_placeholder(Some(&placeholder));
    let list_scroll = crate::app::scrolled(&list);
    list_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    library.append(&list_scroll);
    split.set_start_child(Some(&library));

    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.add_css_class("notes-host");
    host.add_css_class("empty");
    host.set_vexpand(true);
    host.set_hexpand(true);
    let banner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    banner.add_css_class("notes-notice");
    banner.set_visible(false);
    let banner_text = label("", "notes-notice-text");
    banner_text.set_wrap(true);
    banner_text.set_hexpand(true);
    banner.append(&banner_text);
    let dismiss = gtk::Button::new();
    dismiss.set_child(Some(&glyph("close", 12)));
    dismiss.add_css_class("notes-notice-close");
    dismiss.set_tooltip_text(Some("Dismiss"));
    let hide = banner.downgrade();
    dismiss.connect_clicked(move |_| {
        if let Some(banner) = hide.upgrade() {
            banner.set_visible(false)
        }
    });
    banner.append(&dismiss);
    host.append(&banner);
    let elsewhere = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    elsewhere.add_css_class("notes-notice");
    elsewhere.set_visible(false);
    let elsewhere_text = label("", "notes-notice-text");
    elsewhere_text.set_wrap(true);
    elsewhere_text.set_hexpand(true);
    elsewhere.append(&elsewhere_text);
    let reveal_key = button("Show", "notes-notice-button");
    reveal_key.set_focus_on_click(false);
    reveal_key.set_valign(gtk::Align::Center);
    elsewhere.append(&reveal_key);
    host.append(&elsewhere);
    let tabs = &ui.note_tabs;
    if let Some(parent) = tabs.parent() {
        if let Ok(parent) = parent.downcast::<gtk::Box>() {
            parent.remove(tabs);
        }
    }
    tabs.add_css_class("notes-tabs");
    tabs.set_scrollable(true);
    tabs.set_show_border(false);
    tabs.set_show_tabs(true);
    tabs.set_vexpand(true);
    tabs.set_visible(false);
    host.append(tabs);
    let welcome = gtk::Box::new(gtk::Orientation::Vertical, 10);
    welcome.add_css_class("notes-welcome");
    welcome.set_valign(gtk::Align::Center);
    welcome.set_halign(gtk::Align::Center);
    welcome.set_vexpand(true);
    let mark = glyph("notes", 36);
    mark.add_css_class("notes-welcome-mark");
    welcome.append(&mark);
    let title = label("No note open", "notes-welcome-title");
    title.set_xalign(0.5);
    welcome.append(&title);
    let hint = label("Pick a note from the library, or start a new one.", "notes-welcome-text");
    hint.set_xalign(0.5);
    welcome.append(&hint);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::Center);
    let create = button("New note", "primary");
    create.set_action_name(Some("notes.new"));
    let find = button("Search the library", "");
    find.set_action_name(Some("notes.open"));
    actions.append(&create);
    actions.append(&find);
    welcome.append(&actions);
    let keys = label("Ctrl+N new note  ·  Ctrl+O search  ·  F9 library", "notes-welcome-keys");
    keys.set_xalign(0.5);
    welcome.append(&keys);
    host.append(&welcome);
    split.set_end_child(Some(&host));
    root.append(&split);

    let row_menu = gtk::PopoverMenu::from_model(None::<&gtk::gio::MenuModel>);
    row_menu.set_parent(&list);
    row_menu.set_has_arrow(false);
    row_menu.set_halign(gtk::Align::Start);
    let menu_owner = row_menu.downgrade();
    list.connect_destroy(move |_| {
        if let Some(menu) = menu_owner.upgrade() {
            menu.unparent();
        }
    });

    let shell = Rc::new(Shell {
        root,
        toolbar,
        tools,
        tool_lines,
        customize,
        customize_checks,
        customize_labels,
        picker,
        split,
        library,
        search,
        list,
        list_scroll,
        placeholder,
        row_menu,
        banner,
        banner_text,
        elsewhere,
        elsewhere_text,
        elsewhere_target: Cell::new(0),
        welcome,
        rows: RefCell::new(Vec::new()),
        notes: RefCell::new(Vec::new()),
        any_pinned: Cell::new(false),
        picker_key: RefCell::new(String::new()),
        project: Cell::new(0),
        rail_applied: Cell::new(false),
    });
    SHELL.with(|s| *s.borrow_mut() = Some(shell.clone()));
    let weak_ui = Rc::downgrade(ui);
    let weak = Rc::downgrade(&shell);

    let target = weak_ui.clone();
    let source = weak.clone();
    reveal_key.connect_clicked(move |_| {
        if let (Some(ui), Some(shell)) = (target.upgrade(), source.upgrade()) {
            reveal(&ui, shell.elsewhere_target.get());
        }
    });

    let target = weak_ui.clone();
    collapse.connect_clicked(move |_| {
        if let Some(ui) = target.upgrade() {
            let visible = shell_if_built().is_some_and(|s| s.library.is_visible());
            set_sidebar(&ui, !visible);
        }
    });
    let target = weak_ui.clone();
    shell.split.connect_position_notify(move |split| {
        if let Some(ui) = target.upgrade() {
            if let Some(owner) = owner(&ui) {
                if !owner.rail_collapsed.get() {
                    owner.rail_width.set(split.position().max(180));
                }
                owner.persist(&ui);
            }
        }
    });
    let filter = weak.clone();
    shell.list.set_filter_func(move |row| {
        let Some(shell) = filter.upgrade() else { return true };
        let query = shell.search.text().trim().to_lowercase();
        query.is_empty()
            || shell
                .rows
                .borrow()
                .get(row.index() as usize)
                .is_some_and(|r| r.haystack.contains(&query))
    });
    let headers = weak.clone();
    shell.list.set_header_func(move |row, before| {
        let Some(shell) = headers.upgrade() else { return };
        let rows = shell.rows.borrow();
        let pinned = |row: &gtk::ListBoxRow| {
            rows.get(row.index() as usize).is_some_and(|r| r.note["pinned"] == true)
        };
        let this = pinned(row);
        let show = shell.any_pinned.get()
            && before.is_none_or(|before| pinned(before) != this);
        // A header already saying the same is kept: a new one restyles and remeasures.
        let wanted = show.then_some(if this { "Pinned" } else { "Notes" });
        let shown = row.header().and_downcast::<gtk::Label>().map(|l| l.text());
        if shown.as_deref() != wanted {
            match wanted {
                Some(text) => row.set_header(Some(&label(text, "notes-list-header"))),
                None => row.set_header(gtk::Widget::NONE),
            }
        }
    });
    let target = weak_ui.clone();
    let rows = weak.clone();
    shell.list.connect_row_activated(move |_, row| {
        let (Some(ui), Some(shell)) = (target.upgrade(), rows.upgrade()) else { return };
        let id = shell.rows.borrow().get(row.index() as usize).map(|r| note_id(&r.note));
        if let Some(id) = id {
            with_full(&ui, id, |ui, note| {
                edit(ui, note);
                if let Some(doc) = current_doc(ui) {
                    doc.view.grab_focus();
                }
            });
        }
    });
    let context = gtk::GestureClick::new();
    context.set_button(3);
    let menu_shell = weak.clone();
    context.connect_pressed(move |gesture, _, x, y| {
        let Some(shell) = menu_shell.upgrade() else { return };
        let Some(row) = shell.list.row_at_y(y as i32) else { return };
        let target = shell
            .rows
            .borrow()
            .get(row.index() as usize)
            .map(|r| (r.note["id"].as_i64().unwrap_or(0), r.note["pinned"] == true));
        if let Some((id, pinned)) = target {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            shell.row_menu.set_menu_model(Some(&menu::row_menu(id, pinned)));
            shell
                .row_menu
                .set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            shell.row_menu.popup();
        }
    });
    shell.list.add_controller(context);
    let searching = weak.clone();
    shell.search.connect_search_changed(move |_| {
        if let Some(shell) = searching.upgrade() {
            shell.list.invalidate_filter();
            shell.list.invalidate_headers();
            update_placeholder(&shell);
        }
    });
    let target = weak_ui.clone();
    let first = weak.clone();
    shell.search.connect_activate(move |_| {
        let (Some(ui), Some(shell)) = (target.upgrade(), first.upgrade()) else { return };
        if let Some(row) = first_visible(&shell) {
            shell.list.select_row(Some(&row));
            row.activate();
        }
        if let Some(doc) = current_doc(&ui) {
            doc.view.grab_focus();
        }
    });
    let target = weak_ui.clone();
    shell.search.connect_stop_search(move |search| {
        search.set_text("");
        if let Some(doc) = target.upgrade().and_then(|ui| current_doc(&ui)) {
            doc.view.grab_focus();
        }
    });
    let down = gtk::EventControllerKey::new();
    let first = weak.clone();
    down.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Down {
            if let Some(row) = first.upgrade().and_then(|shell| first_visible(&shell)) {
                row.grab_focus();
                return glib::Propagation::Stop;
            }
        }
        glib::Propagation::Proceed
    });
    shell.search.add_controller(down);
    let target = weak_ui.clone();
    ui.note_tabs.connect_page_notify(move |_| {
        if let Some(ui) = target.upgrade() {
            tabs_changed(&ui);
        }
    });
    let target = weak_ui.clone();
    ui.note_tabs.connect_page_reordered(move |_, _, _| {
        if let Some(ui) = target.upgrade() {
            record_session(&ui);
        }
    });
    let prefs = prefs();
    apply_font(&prefs);
    shell.toolbar.set_visible(prefs.toolbar);
    apply_toolbar(&shell, &prefs);
    update_placeholder(&shell);
    shell
}

fn first_visible(shell: &Shell) -> Option<gtk::ListBoxRow> {
    let query = shell.search.text().trim().to_lowercase();
    shell
        .rows
        .borrow()
        .iter()
        .find(|r| query.is_empty() || r.haystack.contains(&query))
        .map(|r| r.row.clone())
}

fn update_placeholder(shell: &Shell) {
    shell.placeholder.set_text(if shell.rows.borrow().is_empty() {
        "No notes in this project yet.\nPress Ctrl+N to start one."
    } else {
        "No notes match your search."
    });
}

fn mount(ui: &Rc<Ui>, shell: &Shell) {
    let page = &ui.pages["notes"];
    page.add_css_class("notes-page");
    page.set_spacing(0);
    if shell.root.parent().as_ref() != Some(page.upcast_ref::<gtk::Widget>()) {
        clear(page);
        page.append(&shell.root);
    }
}

fn shell_error(ui: &Rc<Ui>, message: &str) {
    let shell = shell(ui);
    shell.banner_text.set_text(message);
    shell.banner.set_visible(true);
}

/// A library refresh failed: say so in the Notes banner once the shell is on screen (it may
/// hold open notes), else in place of the "Opening Notes…" placeholder.
pub(super) fn load_error(ui: &Rc<Ui>, message: &str) {
    let page = &ui.pages["notes"];
    match shell_if_built() {
        Some(shell) if shell.root.parent().as_ref() == Some(page.upcast_ref::<gtk::Widget>()) => {
            shell_error(ui, message)
        }
        _ => {
            clear(page);
            page.append(&label(message, "error"));
        }
    }
}

fn build_row(note: &Value) -> Row {
    with_display(note, |title, body, whole| build_row_from(note, title, body, whole))
}

fn build_row_from(note: &Value, title: &str, body: &str, whole: bool) -> Row {
    let (title, body) = (text::clean(title), text::clean(body));
    let (title, body) = (title.as_ref(), body.as_ref());
    let id = note["id"].as_i64().unwrap_or(0);
    let row = gtk::ListBoxRow::new();
    row.add_css_class("notes-row");
    let content = gtk::Box::new(gtk::Orientation::Vertical, 3);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("notes-dirty-dot");
    dot.set_valign(gtk::Align::Center);
    dot.set_visible(doc_by_id(id).is_some_and(|d| d.shown_dirty()));
    dot.set_tooltip_text(Some("Unsaved changes"));
    top.append(&dot);
    let name = text::display_title(title, body);
    let heading = label(&name, "notes-row-title");
    heading.set_hexpand(true);
    heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
    heading.set_valign(gtk::Align::Baseline);
    top.append(&heading);
    if note["pinned"] == true {
        let pin = glyph("pin", 12);
        pin.add_css_class("notes-row-pin");
        pin.set_valign(gtk::Align::Center);
        pin.set_tooltip_text(Some("Pinned to the top of the library"));
        top.append(&pin);
    }
    // The date sits at the end of the title line, so the preview has the whole second line.
    let date = label(&fmt_date(text(note, "updated_at")), "notes-row-date");
    date.set_valign(gtk::Align::Baseline);
    top.append(&date);
    content.append(&top);
    let preview = text::preview(title, body);
    let shown = label(if preview.is_empty() { "No additional text" } else { &preview }, "notes-row-preview");
    shown.set_ellipsize(gtk::pango::EllipsizeMode::End);
    shown.set_xalign(0.);
    content.append(&shown);
    row.set_child(Some(&content));
    row.update_property(&[gtk::accessible::Property::Label(&name)]);
    Row { row, note: note.clone(), haystack: format!("{title}\n{body}").to_lowercase(), dot, heading, date, shown, name, preview, whole }
}

/// What a row shows of one note, and its search text.
struct Content {
    note: Value,
    haystack: String,
    name: String,
    preview: String,
    whole: bool,
}

fn content(note: &Value) -> Content {
    with_display(note, |title, body, whole| {
        let (title, body) = (text::clean(title), text::clean(body));
        Content {
            note: note.clone(),
            haystack: format!("{title}\n{body}").to_lowercase(),
            name: text::display_title(&title, &body),
            preview: text::preview(&title, &body),
            whole,
        }
    })
}

/// Show `content` in `row`'s widgets, setting only the text that differs: new text in a row
/// costs that row, where a row added to or taken from the list restyles all its siblings.
fn show(row: &mut Row, content: Content, redate: bool) {
    if content.name != row.name {
        row.heading.set_text(&content.name);
        row.row.update_property(&[gtk::accessible::Property::Label(&content.name)]);
    }
    if content.preview != row.preview {
        row.shown.set_text(if content.preview.is_empty() { "No additional text" } else { &content.preview });
    }
    if redate || row.note["updated_at"] != content.note["updated_at"] {
        row.date.set_text(&fmt_date(text(&content.note, "updated_at")));
    }
    let id = note_id(&content.note);
    if note_id(&row.note) != id {
        row.dot.set_visible(doc_by_id(id).is_some_and(|d| d.shown_dirty()));
    }
    row.note = content.note;
    row.haystack = content.haystack;
    row.name = content.name;
    row.preview = content.preview;
    row.whole = content.whole;
}

/// The row is current for `note`: the same listed copy, and the whole text if it is known.
fn current_for(row_note: &Value, whole: bool, note: &Value) -> bool {
    row_note == note && (whole || !has_full(note))
}

/// Show `note` in `row`, which showed an older copy of it. `false` when the pin changed,
/// which `build_row` lays out.
fn rebind(row: &mut Row, note: &Value) -> bool {
    if (row.note["pinned"] == true) != (note["pinned"] == true) {
        return false;
    }
    show(row, content(note), false);
    true
}

/// Show `shell.notes` in the list. A row whose note did not change is kept as it is (its
/// widgets, its search text), so a change to one note costs one row, not the library.
fn render_rows(ui: &Rc<Ui>, shell: &Shell) {
    let adjustment = shell.list_scroll.vadjustment();
    let keep = adjustment.value();
    let mut notes = shell.notes.borrow().clone();
    if prefs().sort_title {
        notes.sort_by_cached_key(|n| {
            (n["pinned"] != true, with_display(n, |title, body, _| text::display_title(title, body).to_lowercase()))
        });
    }
    // A kept row's date was written for the day it was built: "14:02" reads "Oct 5" tomorrow.
    let today = glib::DateTime::now_local().map_or((0, 0), |now| (now.year(), now.day_of_year()));
    let redate = DATED.with(|d| d.replace(today)) != today;
    let mut rows: Vec<Row> = shell.rows.borrow_mut().drain(..).collect();
    // The same rows with the same pins in the same places (one note edited, or moved up the
    // list by its edit): each row takes the note now in its place, carrying what was worked
    // out for it, and the list itself does not change.
    let fits = rows.len() == notes.len()
        && rows.iter().zip(&notes).all(|(row, n)| (row.note["pinned"] == true) == (n["pinned"] == true));
    if fits {
        let mut carried: HashMap<i64, Content> = rows
            .iter_mut()
            .map(|row| {
                let content = Content {
                    note: row.note.clone(),
                    haystack: std::mem::take(&mut row.haystack),
                    name: row.name.clone(),
                    preview: row.preview.clone(),
                    whole: row.whole,
                };
                (note_id(&row.note), content)
            })
            .collect();
        for (row, n) in rows.iter_mut().zip(&notes) {
            let content = match carried.remove(&note_id(n)) {
                Some(c) if current_for(&c.note, c.whole, n) => c,
                _ => content(n),
            };
            show(row, content, redate);
        }
        *shell.rows.borrow_mut() = rows;
    } else {
        let mut old: HashMap<i64, Row> = rows.drain(..).map(|r| (note_id(&r.note), r)).collect();
        let mut stale = Vec::new();
        let rows: Vec<Row> = notes
            .iter()
            .map(|n| match old.remove(&note_id(n)) {
                Some(row) if current_for(&row.note, row.whole, n) => {
                    if redate {
                        row.date.set_text(&fmt_date(text(n, "updated_at")));
                    }
                    row
                }
                Some(mut row) => {
                    if rebind(&mut row, n) {
                        if redate {
                            row.date.set_text(&fmt_date(text(n, "updated_at")));
                        }
                        row
                    } else {
                        stale.push(row.row);
                        build_row(n)
                    }
                }
                None => build_row(n),
            })
            .collect();
        stale.extend(old.into_values().map(|r| r.row));
        let widgets: Vec<gtk::ListBoxRow> = rows.iter().map(|r| r.row.clone()).collect();
        *shell.rows.borrow_mut() = rows;
        // Rows only: `remove_all` would also try the row menu parented to the list and, in GTK
        // 4.22, retry it forever ("Tried to remove non-child").
        for row in &stale {
            shell.list.remove(row);
        }
        for (index, row) in widgets.iter().enumerate() {
            if shell.list.row_at_index(index as i32).as_ref() != Some(row) {
                if row.parent().is_some() {
                    shell.list.remove(row);
                }
                shell.list.insert(row, index as i32);
            }
        }
    }
    let hint = match notes.len() {
        0 => "Search notes".to_string(),
        1 => "Search 1 note".to_string(),
        n => format!("Search {n} notes"),
    };
    if shell.search.placeholder_text().as_deref() != Some(hint.as_str()) {
        shell.search.set_placeholder_text(Some(&hint));
    }
    shell.any_pinned.set(notes.iter().any(|n| n["pinned"] == true));
    update_placeholder(shell);
    // Each inserted row was filtered as it went in, and a kept row's text did not change.
    if !shell.search.text().trim().is_empty() {
        shell.list.invalidate_filter();
    }
    shell.list.invalidate_headers();
    select_active_row(ui, shell);
    glib::idle_add_local_once(move || adjustment.set_value(keep));
}

fn select_active_row(ui: &Rc<Ui>, shell: &Shell) {
    let active = current_doc(ui).map(|d| d.id);
    let row = shell
        .rows
        .borrow()
        .iter()
        .find(|r| active.is_some() && r.note["id"].as_i64() == active)
        .map(|r| r.row.clone());
    match row {
        Some(row) => shell.list.select_row(Some(&row)),
        None => shell.list.unselect_all(),
    }
}

/// The note in the visible tab, if any.
pub(super) fn current_doc(ui: &Rc<Ui>) -> Option<Rc<Doc>> {
    let tabs = &ui.note_tabs;
    let widget = tabs.nth_page(tabs.current_page())?;
    if !widget.is_visible() {
        return None;
    }
    docs().into_iter().find(|d| d.draft.layout.upcast_ref::<gtk::Widget>() == &widget)
}

pub(super) fn update_chrome(ui: &Rc<Ui>) {
    let Some(owner) = owner(ui) else { return };
    let (heading, title) = match current_doc(ui) {
        Some(doc) => {
            let name = doc.name();
            if doc.shown_dirty() {
                (format!("{name}  •"), format!("{name} • — Notes · Relay"))
            } else {
                (name.clone(), format!("{name} — Notes · Relay"))
            }
        }
        None => ("Notes".to_string(), "Notes · Relay".to_string()),
    };
    if owner.heading.text() != heading {
        owner.heading.set_text(&heading);
    }
    if owner.window.title().as_deref() != Some(title.as_str()) {
        owner.window.set_title(Some(&title));
    }
}

pub(super) fn dirty_changed(ui: &Rc<Ui>, id: i64, dirty: bool) {
    if let Some(shell) = shell_if_built() {
        for row in shell.rows.borrow().iter() {
            if row.note["id"].as_i64() == Some(id) {
                row.dot.set_visible(dirty);
            }
        }
    }
    menu::sync(current_doc(ui).as_ref());
    update_elsewhere(ui);
}

fn update_welcome(ui: &Rc<Ui>) {
    let Some(shell) = shell_if_built() else { return };
    let project = current_project(ui);
    let open = docs().iter().any(|d| {
        (project == 0 || d.project == project || d.orphan.get())
            && ui.note_tabs.page_num(&d.draft.layout).is_some()
    });
    ui.note_tabs.set_visible(open);
    shell.welcome.set_visible(!open);
    // With no document the host is the paper; with one, the document paints its own.
    if let Some(host) = shell.welcome.parent() {
        if open {
            host.remove_css_class("empty");
        } else {
            host.add_css_class("empty");
        }
    }
}

/// Tabs of other projects are hidden, so unsaved ones among them are named in a bar above
/// the editor, whose Show key switches to the project and brings the note forward.
fn update_elsewhere(ui: &Rc<Ui>) {
    let Some(shell) = shell_if_built() else { return };
    let shown = shell.project.get();
    let hidden: Vec<Rc<Doc>> = docs()
        .into_iter()
        .filter(|d| d.project != shown && !d.orphan.get() && d.shown_dirty())
        .collect();
    shell.elsewhere.set_visible(!hidden.is_empty());
    let Some(first) = hidden.first() else { return };
    shell.elsewhere_target.set(first.id);
    let named: Vec<String> = hidden
        .iter()
        .take(3)
        .map(|d| format!("“{}” in {}", d.name(), project_name(ui, d.project)))
        .collect();
    let more = match hidden.len() {
        n if n > 3 => format!(" and {} more", n - 3),
        _ => String::new(),
    };
    shell
        .elsewhere_text
        .set_text(&format!("Unsaved changes in another project: {}{more}.", named.join(", ")));
}

/// Switch Notes to note `id`'s project and bring the note forward.
fn reveal(ui: &Rc<Ui>, id: i64) {
    let Some(doc) = doc_by_id(id) else { return };
    FOCUS.with(|f| f.set(id));
    super::notes_window::show_project(ui, doc.project);
}

/// `dead` was removed, its notes with it. Clean tabs of it close; tabs with unsaved text stay
/// on screen whatever project is shown, so the text can be saved under another project or
/// dropped, and the library empties when it was showing `dead`.
pub(super) fn project_gone(ui: &Rc<Ui>, dead: i64) {
    for doc in docs().into_iter().filter(|d| d.project == dead && !d.orphan.get()) {
        if doc.draft.busy.get() || doc.dirty() {
            doc.orphan.set(true);
            doc.deleted.set(true);
            doc::deleted_notice(ui, &doc);
            doc.draft.layout.set_visible(true);
            doc.refresh_state(ui);
        } else {
            doc.draft.close();
        }
    }
    if let Some(doc) = doc_by_id(FOCUS.with(Cell::get)).filter(|d| d.orphan.get()) {
        FOCUS.with(|f| f.set(0));
        if let Some(page) = ui.note_tabs.page_num(&doc.draft.layout) {
            ui.note_tabs.set_current_page(Some(page));
        }
    }
    RESTORED.with(|r| r.borrow_mut().remove(&dead));
    if SESSION.with(|s| s.borrow_mut().remove(&dead.to_string())).is_some() {
        persist(ui);
    }
    if let Some(shell) = shell_if_built() {
        if shell.project.get() == dead {
            shell.project.set(0);
            shell.notes.borrow_mut().clear();
            render_rows(ui, &shell);
        }
        refresh_picker(ui, &shell, shell.project.get());
    }
    tabs_changed(ui);
}

/// The project picker, rebuilt only when it would show something new: a rebuild closes an
/// open picker under the pointer.
fn refresh_picker(ui: &Rc<Ui>, shell: &Shell, project: i64) {
    let key = {
        let mut key = project.to_string();
        for p in ui.projects.borrow().iter() {
            key.push_str(&format!("|{}:{}:{}", p["id"], p["workspace_id"], text(p, "name")));
        }
        for w in ui.workspaces.borrow().iter() {
            key.push_str(&format!("|w{}:{}", w["id"], text(w, "name")));
        }
        key
    };
    if *shell.picker_key.borrow() == key {
        return;
    }
    *shell.picker_key.borrow_mut() = key;
    clear(&shell.picker);
    let picker = super::workspace_picker(ui, project, "notes");
    picker.set_hexpand(true);
    // Drawn as the search field's twin: a leading folder where the field has its magnifier,
    // the project's name, then its workspace faint after it. The shared caption reads
    // "Workspace / Project" and ellipsizes at its end, which would cut the project first.
    let content = picker.child().and_downcast::<gtk::Box>();
    if let Some(caption) = content.as_ref().and_then(|c| c.first_child()).and_downcast::<gtk::Label>() {
        let full = caption.text().to_string();
        let (space, name) = full.split_once(" / ").unwrap_or(("", full.as_str()));
        picker.set_tooltip_text(Some(&format!("{name} · {space}\nSwitch project")));
        caption.set_text(name);
        caption.set_xalign(0.);
        caption.set_max_width_chars(-1);
        let mut after = caption.clone().upcast::<gtk::Widget>();
        // A workspace named like its project would only say the name twice.
        if !space.is_empty() && space != name {
            let workspace = label(space, "notes-picker-space");
            workspace.set_ellipsize(gtk::pango::EllipsizeMode::End);
            workspace.set_max_width_chars(14);
            workspace.set_xalign(0.);
            if let Some(content) = &content {
                content.insert_child_after(&workspace, Some(&caption));
            }
            after = workspace.upcast();
        }
        after.set_hexpand(true);
        if let Some(content) = &content {
            let mark = crate::icons::image("folder", 14);
            mark.add_css_class("notes-picker-mark");
            content.prepend(&mark);
        }
    }
    shell.picker.append(&picker);
}

fn tabs_changed(ui: &Rc<Ui>) {
    update_welcome(ui);
    update_elsewhere(ui);
    if let Some(shell) = shell_if_built() {
        select_active_row(ui, &shell);
    }
    let current = current_doc(ui);
    menu::sync(current.as_ref());
    update_chrome(ui);
    record_session(ui);
}

/// Remember the open tabs and the active one for the project on screen.
fn record_session(ui: &Rc<Ui>) {
    let project = shell_if_built().map_or(0, |s| s.project.get());
    if project == 0 || !RESTORED.with(|r| r.borrow().contains(&project)) {
        return;
    }
    let all = docs();
    let tabs = &ui.note_tabs;
    let open: Vec<i64> = (0..tabs.n_pages())
        .filter_map(|i| tabs.nth_page(Some(i)))
        .filter_map(|w| {
            all.iter()
                .find(|d| d.project == project && d.draft.layout.upcast_ref::<gtk::Widget>() == &w)
                .map(|d| d.id)
        })
        .collect();
    let active = current_doc(ui).filter(|d| d.project == project).map(|d| d.id);
    let entry = json!({"open":open,"active":active});
    let changed = SESSION.with(|s| {
        let mut s = s.borrow_mut();
        let changed = s.get(&project.to_string()) != Some(&entry);
        s.insert(project.to_string(), entry);
        changed
    });
    if changed {
        persist(ui);
    }
}

/// Build a tab for `note` (at `position`, else last) and show it.
pub(super) fn open_doc(ui: &Rc<Ui>, note: Value, position: Option<u32>) -> Rc<Doc> {
    let shell = shell(ui);
    mount(ui, &shell);
    let doc = doc::build(ui, &note);
    let id = doc.id;
    let tabs = &ui.note_tabs;
    let page = match position {
        Some(position) => tabs.insert_page(&doc.draft.layout, Some(&doc.tab), Some(position)),
        None => tabs.append_page(&doc.draft.layout, Some(&doc.tab)),
    };
    tabs.set_tab_reorderable(&doc.draft.layout, true);
    let (weak_ui, weak) = (Rc::downgrade(ui), Rc::downgrade(&doc));
    *doc.draft.on_close.borrow_mut() = Some(Box::new(move || {
        let Some(ui) = weak_ui.upgrade() else { return };
        let mut typing = false;
        let mut fresh = false;
        if let Some(doc) = weak.upgrade() {
            doc.stop_timers();
            fresh = doc.fresh.get() && !doc.deleted.get();
            // The keyboard was in this note: the next one's text takes it, where removing the
            // page put it in the next note's title, all of it selected, for the next key to
            // replace.
            typing = window_of(&ui)
                .and_then(|window| gtk::prelude::GtkWindowExt::focus(&window))
                .is_some_and(|focus| focus.is_ancestor(&doc.draft.layout));
            if let Some(page) = ui.note_tabs.page_num(&doc.draft.layout) {
                ui.note_tabs.remove_page(Some(page));
            }
        }
        DOCS.with(|docs| docs.borrow_mut().remove(&id));
        // New note made it at once; closed untouched, it is not kept as an "Untitled" note.
        if fresh {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                if ui.call("notes.delete", json!({"note_id":id})).await.is_ok() {
                    refresh_notes(&ui);
                }
            });
        }
        if let Some(shell) = shell_if_built() {
            for row in shell.rows.borrow().iter() {
                if row.note["id"].as_i64() == Some(id) {
                    row.dot.set_visible(false);
                }
            }
        }
        tabs_changed(&ui);
        if typing {
            if let Some(doc) = current_doc(&ui) {
                doc.view.grab_focus();
            }
        }
    }));
    DOCS.with(|docs| docs.borrow_mut().insert(id, doc.clone()));
    tabs.set_current_page(Some(page));
    tabs_changed(ui);
    doc::load_tasks(ui, &doc);
    doc
}

/// A task changed somewhere: every open note lists its tasks again, once the burst settles.
pub fn tasks_changed(ui: &Rc<Ui>) {
    if !owner(ui).is_some_and(|o| o.window.is_visible()) || docs().is_empty() {
        return;
    }
    if let Some(timer) = TASKS_TIMER.with(|t| t.borrow_mut().take()) {
        timer.remove();
    }
    let weak = Rc::downgrade(ui);
    let timer = glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
        TASKS_TIMER.with(|t| t.borrow_mut().take());
        if let Some(ui) = weak.upgrade() {
            for doc in docs() {
                doc::load_tasks(&ui, &doc);
            }
        }
    });
    TASKS_TIMER.with(|t| *t.borrow_mut() = Some(timer));
}

pub fn edit(ui: &Rc<Ui>, note: Value) {
    let shell = shell(ui);
    mount(ui, &shell);
    let id = note["id"].as_i64().unwrap_or(0);
    if let Some(doc) = doc_by_id(id) {
        doc.draft.layout.set_visible(true);
        if let Some(page) = ui.note_tabs.page_num(&doc.draft.layout) {
            ui.note_tabs.set_current_page(Some(page));
        }
        tabs_changed(ui);
        return;
    }
    open_doc(ui, note, None);
}

pub(super) fn new_note(ui: &Rc<Ui>, title: Option<String>, body: String) {
    let project = current_project(ui);
    if project == 0 {
        // Ctrl+N and File > New note said nothing here (RA-488).
        load_error(ui, "No project yet. Add one from the main window to create a note in it.");
        return;
    }
    if let Some(message) = doc::oversize(&body) {
        shell_error(ui, &format!("Could not create a note: {message}"));
        return;
    }
    let empty = title.is_none() && body.is_empty();
    let mut payload = json!({"project_id":project,"body":body,"pinned":false});
    if let Some(title) = title {
        payload["title"] = json!(title);
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        match ui.call("notes.create", payload).await {
            Ok(note) => {
                let doc = open_doc(&ui, note, None);
                doc.fresh.set(empty);
                doc.view.grab_focus();
                refresh_notes(&ui);
            }
            Err(error) => shell_error(&ui, &format!("Could not create a note: {}", readable(&error))),
        }
    });
}

pub(super) fn set_sidebar(ui: &Rc<Ui>, visible: bool) {
    let shell = shell(ui);
    if let Some(owner) = owner(ui) {
        owner.rail_collapsed.set(!visible);
        owner.persist(ui);
    }
    shell.library.set_visible(visible);
    if let Some(action) = menu::lookup("sidebar") {
        action.set_state(&visible.to_variant());
    }
}

pub(super) fn focus_library(ui: &Rc<Ui>) {
    let shell = shell(ui);
    if !shell.library.is_visible() {
        set_sidebar(ui, true);
    }
    shell.search.grab_focus();
    shell.search.select_region(0, -1);
}

pub(super) fn toggle_pin(doc: &Rc<Doc>) {
    doc.pin.set_active(!doc.pin.is_active());
}

pub(super) fn cycle_tab(ui: &Rc<Ui>, direction: i32) {
    let tabs = &ui.note_tabs;
    let visible: Vec<u32> = (0..tabs.n_pages())
        .filter(|i| tabs.nth_page(Some(*i)).is_some_and(|w| w.is_visible()))
        .collect();
    if visible.len() < 2 {
        return;
    }
    let at = visible.iter().position(|i| Some(*i) == tabs.current_page()).unwrap_or(0) as i32;
    let next = (at + direction).rem_euclid(visible.len() as i32) as usize;
    tabs.set_current_page(Some(visible[next]));
    // The notebook hands a page it has not focused before to its first field, the title,
    // selecting it all for the next key to replace: the text takes the keyboard instead.
    if let Some(doc) = current_doc(ui) {
        doc.view.grab_focus();
    }
}

/// The note of the library row the keyboard is on, if it is on one.
pub(super) fn focused_row(ui: &Rc<Ui>) -> Option<i64> {
    let shell = shell_if_built()?;
    let focus = window_of(ui).and_then(|window| gtk::prelude::GtkWindowExt::focus(&window))?;
    let row = std::iter::successors(Some(focus), |w| w.parent()).find_map(|w| w.downcast::<gtk::ListBoxRow>().ok())?;
    if row.parent().as_ref() != Some(shell.list.upcast_ref::<gtk::Widget>()) {
        return None;
    }
    let id = shell.rows.borrow().get(row.index() as usize).map(|r| note_id(&r.note));
    id
}

fn listed(id: i64) -> Option<Value> {
    shell_if_built()?
        .notes
        .borrow()
        .iter()
        .find(|n| n["id"].as_i64() == Some(id))
        .cloned()
}

pub(super) fn open_id(ui: &Rc<Ui>, id: i64, rename: bool) {
    open_then(ui, id, rename, |_, _| {});
}

/// Open note `id` (reading it whole first when it is not at hand), focus it, then `then`.
fn open_then(ui: &Rc<Ui>, id: i64, rename: bool, then: impl FnOnce(&Rc<Ui>, i64) + 'static) {
    let shown = move |ui: &Rc<Ui>, note: Value| {
        edit(ui, note);
        if let Some(doc) = doc_by_id(id) {
            if rename {
                doc.title.grab_focus();
                doc.title.select_region(0, -1);
            } else {
                doc.view.grab_focus();
            }
        }
        then(ui, id);
    };
    match doc_by_id(id) {
        Some(doc) => {
            let note = doc.draft.base.borrow().clone();
            shown(ui, note);
        }
        None => with_full(ui, id, shown),
    }
}

pub(super) fn pin_id(ui: &Rc<Ui>, id: i64) {
    if let Some(doc) = doc_by_id(id) {
        toggle_pin(&doc);
        return;
    }
    let Some(note) = listed(id) else { return };
    let pinned = note["pinned"] != true;
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        match ui.call("notes.pin", json!({"note_id":id,"pinned":pinned})).await {
            Ok(_) => refresh_notes(&ui),
            Err(error) => shell_error(&ui, &format!("Could not {}: {}", if pinned { "pin" } else { "unpin" }, readable(&error))),
        }
    });
}

pub(super) fn duplicate_id(ui: &Rc<Ui>, id: i64) {
    match doc_by_id(id) {
        Some(doc) => new_note(ui, text::copy_title(&doc.title.text()), doc.body()),
        None => with_full(ui, id, |ui, note| {
            new_note(ui, text::copy_title(text(&note, "title")), text(&note, "body").to_string())
        }),
    }
}

/// The library's "Create task from note": open the note, then file it as a task.
pub(super) fn task_id(ui: &Rc<Ui>, id: i64) {
    open_then(ui, id, false, |ui, id| {
        if let Some(doc) = doc_by_id(id) {
            doc::to_task(ui, &doc);
        }
    });
}

pub(super) fn copy_id(ui: &Rc<Ui>, id: i64) {
    let copy = |ui: &Rc<Ui>, body: &str| {
        if let Some(shell) = shell_if_built() {
            shell.list.clipboard().set_text(body);
        }
        if let Some(doc) = current_doc(ui) {
            doc.flash("Note text copied");
        }
    };
    match doc_by_id(id) {
        Some(doc) => copy(ui, &doc.body()),
        None => with_full(ui, id, move |ui, note| copy(ui, text(&note, "body"))),
    }
}

pub(super) fn delete_id(ui: &Rc<Ui>, id: i64) {
    match doc_by_id(id) {
        Some(doc) => confirm_delete(ui, id, doc.name()),
        None if listed(id).is_none() => confirm_delete(ui, id, "this note".into()),
        None => with_full(ui, id, move |ui, n| {
            let name = text::display_title(&text::clean(text(&n, "title")), &text::clean(text(&n, "body")));
            confirm_delete(ui, id, name);
        }),
    }
}

pub(super) fn confirm_delete(ui: &Rc<Ui>, id: i64, name: String) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let dialog = gtk::AlertDialog::builder()
            .message(format!("Delete “{name}”?"))
            .detail("It leaves this project's library. Tasks made from it keep their text.")
            .buttons(["Cancel", "Delete"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        if dialog.choose_future(window_of(&ui).as_ref()).await.ok() != Some(1) {
            return;
        }
        match ui.call("notes.delete", json!({"note_id":id})).await {
            Ok(_) => {
                if let Some(doc) = doc_by_id(id) {
                    doc.fresh.set(false);
                    doc::discard_close(&ui, &doc);
                }
                refresh_notes(&ui);
            }
            Err(error) => match doc_by_id(id) {
                Some(doc) => doc.show_notice(&ui, &format!("Not deleted: {}", readable(&error)), Vec::new()),
                None => shell_error(&ui, &format!("Not deleted: {}", readable(&error))),
            },
        }
    });
}

/// Render the library for `project`, reconcile open notes with the stored copies, and
/// restore the project's remembered tabs the first time it is shown. The shell itself is
/// built once, so the search text, scroll position and editors survive every refresh.
pub fn workspace(ui: &Rc<Ui>, _name: &str, project: i64, notes: &[Value]) {
    let shell = shell(ui);
    mount(ui, &shell);
    // Read the remembered tabs before hiding other projects' tabs records anything.
    let saved = SESSION.with(|s| s.borrow().get(&project.to_string()).cloned());
    let changed = shell.project.replace(project) != project;
    let owner = owner(ui);
    if let Some(owner) = &owner {
        if !shell.rail_applied.replace(true) {
            shell.split.set_position(owner.rail_width.get());
            shell.library.set_visible(!owner.rail_collapsed.get());
            if let Some(action) = menu::lookup("sidebar") {
                action.set_state(&(!owner.rail_collapsed.get()).to_variant());
            }
        }
    }
    refresh_picker(ui, &shell, project);
    // Rebuilding every row loses keyboard focus and costs work in all note text: only when
    // the library shows something new.
    let fresh = changed || shell.notes.borrow().as_slice() != notes;
    if fresh {
        *shell.notes.borrow_mut() = notes.to_vec();
        // Whole copies are kept for what the library lists and what is open, no more.
        let listed: HashSet<i64> = notes.iter().map(note_id).collect();
        FULL.with(|full| full.borrow_mut().retain(|id, _| listed.contains(id) || doc_by_id(*id).is_some()));
    }
    for doc in docs() {
        let mine = doc.project == project;
        doc.draft.layout.set_visible(mine || doc.orphan.get());
        if mine {
            // The list holds previews: an open note is reconciled with its whole stored copy,
            // which the refresh read first. One it could not read waits for the next refresh.
            match notes.iter().find(|n| n["id"].as_i64() == Some(doc.id)) {
                Some(listed) => {
                    if let Some(latest) = full_note(listed) {
                        doc::reconcile(ui, &doc, Some(&latest));
                    }
                }
                None => doc::reconcile(ui, &doc, None),
            }
        }
    }
    if changed {
        // Another project's list starts at its top, without the last project's error.
        shell.list_scroll.vadjustment().set_value(0.0);
        shell.banner.set_visible(false);
    }
    if fresh {
        render_rows(ui, &shell);
    }
    let first_time = RESTORED.with(|r| r.borrow_mut().insert(project));
    if first_time {
        match &saved {
            Some(entry) => {
                for id in rows(entry, "open").iter().filter_map(Value::as_i64) {
                    if doc_by_id(id).is_none() {
                        if let Some(note) = notes.iter().find(|n| n["id"].as_i64() == Some(id)).and_then(full_note) {
                            open_doc(ui, note, None);
                        }
                    }
                }
            }
            None => {
                if !docs().iter().any(|d| d.project == project) {
                    if let Some(note) = notes.first().and_then(full_note) {
                        open_doc(ui, note, None);
                    }
                }
            }
        }
    }
    if first_time || changed {
        let wanted = saved
            .as_ref()
            .and_then(|s| s["active"].as_i64())
            .and_then(doc_by_id)
            .filter(|d| d.project == project)
            .or_else(|| {
                (0..ui.note_tabs.n_pages())
                    .filter_map(|i| ui.note_tabs.nth_page(Some(i)))
                    .find(|w| w.is_visible())
                    .and_then(|w| docs().into_iter().find(|d| d.draft.layout.upcast_ref::<gtk::Widget>() == &w))
            });
        if let Some(doc) = wanted {
            if let Some(page) = ui.note_tabs.page_num(&doc.draft.layout) {
                ui.note_tabs.set_current_page(Some(page));
            }
        }
    }
    // The unsaved-elsewhere bar's Show key asked for this note.
    if let Some(doc) = doc_by_id(FOCUS.with(Cell::get)).filter(|d| d.project == project) {
        FOCUS.with(|f| f.set(0));
        if let Some(page) = ui.note_tabs.page_num(&doc.draft.layout) {
            ui.note_tabs.set_current_page(Some(page));
        }
        doc.view.grab_focus();
    }
    tabs_changed(ui);
    if let Some(owner) = owner {
        owner.renders.set(owner.renders.get() + 1);
    }
}

/// Fixture checks for what needs GtkSourceView itself (the language, the scheme and search),
/// run by smoke. The pure list, prefix and go-to helpers are unit-tested in `note_pages/text.rs`.
pub fn verify_tools() {
    use sourceview5::prelude::*;
    assert!(sourceview5::LanguageManager::default().language("markdown").is_some());
    assert!(scheme().is_some(), "Notes colour scheme");
    let buffer = sourceview5::Buffer::new(None);
    buffer.set_text("one ONE one");
    let settings = sourceview5::SearchSettings::new();
    settings.set_wrap_around(true);
    settings.set_search_text(Some("one"));
    let search = sourceview5::SearchContext::new(&buffer, Some(&settings));
    let (start, end, _) = search.forward(&buffer.start_iter()).expect("first match");
    assert_eq!((start.offset(), end.offset()), (0, 3));
    let (mut start, mut end, _) = search.backward(&buffer.end_iter()).expect("last match");
    assert_eq!((start.offset(), end.offset()), (8, 11));
    search.replace(&mut start, &mut end, "two").expect("replace one");
    assert_eq!(buffer_text(buffer.upcast_ref()), "one ONE two");
    settings.set_case_sensitive(true);
    assert_eq!(crate::editor::replace_all(&search, "one+").expect("replace all"), 1);
    assert_eq!(buffer_text(buffer.upcast_ref()), "one+ ONE two");
    // No match left: the binding's own replace_all aborts here; the helper reports 0.
    settings.set_search_text(Some("absent"));
    assert_eq!(crate::editor::replace_all(&search, "x").expect("replace none"), 0);
    assert_eq!(buffer_text(buffer.upcast_ref()), "one+ ONE two");
}

