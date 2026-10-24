//! The board: a keyboard-first columns/list view over Relay's own tasks, in the spirit of
//! lific's issue board — status-icon columns, dense cards, a peek panel, quick create and
//! drag-to-reorder — built only on the engine's `task.*` ops.
//!
//! The page stays mounted while events re-render it, so its state (view, filters, hidden
//! columns, focus and the peeked task) lives in one `Board` per page, not in the widgets.
use super::*;
use gtk::cairo;
use gtk::gdk;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use task_pages::{COLUMNS, COLUMN_TITLES, OPEN_COLUMNS, PRIORITIES, SIZES, TYPES};
// The board's pure logic lives in relay-board, which builds without GTK so CI tests it.
use relay_board::{drop_index, group_of, group_title, open_blockers, task_matches, zone, Filters, Nudge, Zone, URGENT_FIRST};

const GROUPS: [(&str, &str); 7] = [
    ("", "None"),
    ("parent", "Parent"),
    ("type", "Type"),
    ("priority", "Priority"),
    ("size", "Size"),
    ("module", "Module"),
    ("session", "Agent"),
];

/// Rendered lanes (or list sections) in order, each with its visible cards in order.
type Lanes = Vec<(String, Vec<(i64, gtk::Widget)>)>;
/// How long the search waits for the typing to pause before it filters the board.
const SEARCH_PAUSE: std::time::Duration = std::time::Duration::from_millis(150);
type Action = Box<dyn Fn(&Rc<Board>)>;
/// What a View-menu pill does when toggled: on or off.
type Toggle = Box<dyn Fn(&Rc<Board>, bool)>;

/// Where the board's view (list or board, the grouping, the hidden columns) is kept between
/// runs, as Notes keeps its window in `native.notes.window`.
const VIEW_KEY: &str = "native.board";

thread_local! {
    static BOARD: RefCell<Option<Rc<Board>>> = const { RefCell::new(None) };
    /// The module the board is narrowed to, with its project: a module is a bundle of tasks,
    /// shown on the same board with the same create, timeline and done.
    static SCOPE: Cell<Option<(i64, i64)>> = const { Cell::new(None) };
    /// The view as last read from or written to [`VIEW_KEY`]; `None` until it has been read,
    /// so a board drawn before then never writes its defaults over a saved view.
    static SAVED_VIEW: RefCell<Option<Value>> = const { RefCell::new(None) };
    static VIEW_TIMER: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

/// Reads the saved view once a run, before the first board is mounted (`pages::refresh`).
pub async fn load_view(ui: &Rc<Ui>) {
    if SAVED_VIEW.with(|saved| saved.borrow().is_some()) {
        return;
    }
    // A failed read leaves the view unread: the defaults show and nothing is written.
    if let Ok(saved) = ui.call("settings.get", json!({"path": VIEW_KEY})).await {
        let saved = &saved["value"];
        let group = saved["group"].as_str().filter(|g| GROUPS.iter().any(|(key, _)| key == g)).unwrap_or("");
        let hidden: BTreeSet<String> = saved["hidden"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|column| COLUMNS.contains(column))
            .map(str::to_string)
            .collect();
        let view = view_of(saved["list"].as_bool().unwrap_or(false), group, &hidden);
        SAVED_VIEW.with(|slot| *slot.borrow_mut() = Some(view));
    }
}

/// The view in the shape [`VIEW_KEY`] keeps it.
fn view_of(list: bool, group: &str, hidden: &BTreeSet<String>) -> Value {
    json!({"list": list, "group": group, "hidden": hidden})
}
/// Opens the board narrowed to one module.
pub fn open_module(ui: &Rc<Ui>, module: i64) {
    SCOPE.with(|scope| scope.set(Some((ui.project.get(), module))));
    if ui.page.borrow().as_str() == "board" {
        ui.refresh_page();
    } else {
        ui.navigate("board");
    }
}
/// Back to every task: the Board and Modules tabs both leave a module's board.
pub fn leave_module() {
    SCOPE.with(|scope| scope.set(None));
}
fn scope() -> Option<i64> {
    SCOPE.with(|scope| scope.get()).map(|(_, module)| module)
}
fn current() -> Option<Rc<Board>> {
    BOARD.with(|board| board.borrow().clone())
}

/// Every task by id, built once per render: a card's links are looked up, not scanned for.
fn by_id(tasks: &[Value]) -> BTreeMap<i64, &Value> {
    tasks.iter().filter_map(|t| Some((t["id"].as_i64()?, t))).collect()
}

/// The line under a lane's name, as GitHub's project columns carry one.
fn lane_meaning(column: &str) -> &'static str {
    match column {
        "backlog" => "This hasn't been started",
        "ready" => "This is ready to be picked up",
        "active" => "This is actively being worked on",
        "in_review" => "This item is in review",
        _ => "This has been completed",
    }
}

pub(super) fn caption(column: &str) -> &'static str {
    COLUMN_TITLES
        .iter()
        .find(|(name, _)| *name == column)
        .map(|(_, title)| *title)
        .unwrap_or("Backlog")
}

/// `5m`, `3h`, `2d`: how long ago an RFC 3339 timestamp was, at a glance.
pub(super) fn ago(ts: &str) -> String {
    crate::relative::ago(ts, crate::relative::Form::Compact).unwrap_or_default()
}
/// The peek's longer reading of an [`ago`]: `just now`, `5m ago`, `on Mar 1`, or a dash.
pub(super) fn since(ts: &str) -> String {
    crate::relative::ago(ts, crate::relative::Form::CompactAgo).unwrap_or_else(|| "—".into())
}

pub(super) fn hue(name: &str) -> usize {
    name.bytes()
        .fold(2_166_136_261u32, |h, b| (h ^ b as u32).wrapping_mul(16_777_619)) as usize
        % 8
}

fn paint(widget: &gtk::Widget, cr: &cairo::Context, alpha: f64) {
    let c = widget.color();
    cr.set_source_rgba(
        c.red() as f64,
        c.green() as f64,
        c.blue() as f64,
        c.alpha() as f64 * alpha,
    );
}

/// lific's status glyphs in Relay's state colours: dashed backlog, open ready, half-full
/// active, three-quarter review and a checked done.
pub(super) fn status_icon(column: &str, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_valign(gtk::Align::Center);
    area.add_css_class("status-icon");
    area.add_css_class(&format!("status-{column}"));
    let column = column.to_string();
    area.set_draw_func(move |widget, cr, width, height| {
        let widget = widget.upcast_ref::<gtk::Widget>();
        paint(widget, cr, 1.);
        let (cx, cy) = (width as f64 / 2., height as f64 / 2.);
        let r = width.min(height) as f64 / 2. - 1.;
        cr.set_line_width(1.4);
        let pie = |cr: &cairo::Context, end: f64| {
            cr.move_to(cx, cy);
            cr.arc(cx, cy, r - 2.6, -std::f64::consts::FRAC_PI_2, end);
            cr.close_path();
            let _ = cr.fill();
        };
        match column.as_str() {
            "backlog" => {
                cr.set_dash(&[1.6, 1.9], 0.);
                cr.arc(cx, cy, r - 0.3, 0., std::f64::consts::TAU);
                let _ = cr.stroke();
            }
            "done" => {
                cr.arc(cx, cy, r, 0., std::f64::consts::TAU);
                let _ = cr.fill();
                cr.set_operator(cairo::Operator::Clear);
                cr.set_line_width(1.6);
                cr.move_to(cx - r * 0.45, cy + r * 0.02);
                cr.line_to(cx - r * 0.1, cy + r * 0.36);
                cr.line_to(cx + r * 0.48, cy - r * 0.32);
                let _ = cr.stroke();
            }
            other => {
                cr.arc(cx, cy, r - 0.3, 0., std::f64::consts::TAU);
                let _ = cr.stroke();
                match other {
                    "active" => pie(cr, std::f64::consts::FRAC_PI_2),
                    "in_review" => pie(cr, std::f64::consts::PI),
                    _ => {}
                }
            }
        }
    });
    area
}

/// Signal bars for low/medium/high, a red square with a cut-out `!` for urgent.
pub(super) fn priority_icon(priority: &str) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(13);
    area.set_content_height(13);
    area.set_valign(gtk::Align::Center);
    area.add_css_class("priority-icon");
    area.add_css_class(&format!("priority-{priority}"));
    area.set_tooltip_text(Some(&format!("Priority: {priority}")));
    let priority = priority.to_string();
    area.set_draw_func(move |widget, cr, _, _| {
        let widget = widget.upcast_ref::<gtk::Widget>();
        if priority == "urgent" {
            paint(widget, cr, 1.);
            cr.rectangle(0.5, 0.5, 12., 12.);
            let _ = cr.fill();
            cr.set_operator(cairo::Operator::Clear);
            cr.rectangle(5.6, 2.8, 1.8, 5.2);
            cr.rectangle(5.6, 9.2, 1.8, 1.8);
            let _ = cr.fill();
            return;
        }
        let lit = match priority.as_str() {
            "high" => 3,
            "medium" => 2,
            _ => 1,
        };
        for i in 0..3 {
            paint(widget, cr, if i < lit { 1. } else { 0.25 });
            let h = [4.5, 8., 11.5][i];
            cr.rectangle(1. + i as f64 * 4., 12.5 - h, 2.6, h);
            let _ = cr.fill();
        }
    });
    area
}

pub(super) fn lamp(class: &str) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(6);
    area.set_content_height(6);
    area.set_valign(gtk::Align::Center);
    area.add_css_class("board-lamp");
    if !class.is_empty() {
        area.add_css_class(class);
    }
    area.set_draw_func(|widget, cr, w, h| {
        paint(widget.upcast_ref(), cr, 1.);
        cr.arc(w as f64 / 2., h as f64 / 2., w.min(h) as f64 / 2., 0., std::f64::consts::TAU);
        let _ = cr.fill();
    });
    area
}

/// View-toggle glyphs: three lanes for Board, three rows for List.
fn view_glyph(list: bool) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(13);
    area.set_content_height(12);
    area.set_valign(gtk::Align::Center);
    area.set_draw_func(move |widget, cr, _, _| {
        paint(widget.upcast_ref(), cr, 1.);
        cr.set_line_width(1.4);
        for i in 0..3 {
            let at = 1.5 + i as f64 * 4.5;
            if list {
                cr.move_to(0.5, at);
                cr.line_to(12.5, at);
            } else {
                cr.rectangle(at - 1., 0.7, 2.6, 10.6);
            }
        }
        let _ = cr.stroke();
    });
    area
}

pub(super) fn state_badge(state: &str) -> Option<gtk::Box> {
    let caption = match state {
        "dispatched" => "Dispatched",
        "running" => "Running",
        "blocked" => "Blocked",
        "failed" => "Failed",
        "awaiting_review" => "Review",
        _ => return None,
    };
    let badge = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    badge.add_css_class("state-badge");
    badge.add_css_class(&format!("state-{state}"));
    badge.set_valign(gtk::Align::Center);
    badge.append(&lamp(""));
    badge.append(&label(&caption.to_uppercase(), "state-caption"));
    badge.set_tooltip_text(Some(&format!("Execution state: {}", state.replace('_', " "))));
    Some(badge)
}

pub(super) fn label_chip(name: &str) -> gtk::Box {
    let chip = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    chip.add_css_class("label-chip");
    chip.set_valign(gtk::Align::Center);
    let swatch = gtk::DrawingArea::new();
    swatch.set_content_width(6);
    swatch.set_content_height(6);
    swatch.set_valign(gtk::Align::Center);
    swatch.add_css_class(&format!("label-hue-{}", hue(name)));
    swatch.set_draw_func(|widget, cr, w, h| {
        paint(widget.upcast_ref(), cr, 1.);
        cr.rectangle(0., 0., w as f64, h as f64);
        let _ = cr.fill();
    });
    chip.append(&swatch);
    let caption = label(name, "label-chip-name");
    caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
    caption.set_max_width_chars(14);
    chip.append(&caption);
    chip
}

fn labels_row(task: &Value, max: usize) -> Option<gtk::Box> {
    let names: Vec<String> = rows(task, "labels")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    if names.is_empty() {
        return None;
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.add_css_class("card-labels");
    for name in names.iter().take(max) {
        row.append(&label_chip(name));
    }
    if names.len() > max {
        let more = label(&format!("+{}", names.len() - max), "label-more");
        more.set_tooltip_text(Some(&names[max..].join(", ")));
        row.append(&more);
    }
    Some(row)
}

/// Chips in rows that wrap at about `budget` characters, each kept its own width: a FlowBox
/// would line them up in equal columns.
pub(super) fn wrapped(items: Vec<(gtk::Widget, usize)>, budget: usize) -> gtk::Box {
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let mut row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let mut used = 0;
    for (item, wide) in items {
        if used > 0 && used + wide > budget {
            rows.append(&row);
            row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            used = 0;
        }
        item.set_valign(gtk::Align::Center);
        row.append(&item);
        used += wide;
    }
    if used > 0 {
        rows.append(&row);
    }
    rows
}

/// `high` → `High`: a field value as a pill or picker reads it.
pub(super) fn titled(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}


fn spacer() -> gtk::Box {
    let space = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    space.set_hexpand(true);
    space
}

/// The Modules tab: the numbers across every module, then a card per module, the ones still
/// in progress before the completed ones. A card opens its module's board.
pub fn modules(ui: &Rc<Ui>, body: &gtk::Box, modules: &[Value]) {
    let total_of = |m: &Value| -> i64 { m["counts"].as_object().map(|c| c.values().filter_map(Value::as_i64).sum()).unwrap_or(0) };
    let count_of = |m: &Value, column: &str| m["counts"][column].as_i64().unwrap_or(0);
    // The page scrolls itself (app.rs), so the content goes straight into it.
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("modules-content");
    content.set_vexpand(true);
    body.append(&content);
    if modules.is_empty() {
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 10);
        empty.add_css_class("modules-empty");
        empty.set_halign(gtk::Align::Center);
        empty.set_valign(gtk::Align::Center);
        empty.set_vexpand(true);
        let mark = crate::icons::image("modules", 28);
        mark.add_css_class("modules-empty-mark");
        empty.append(&mark);
        // Centred as the mark and the key are: the title sat at the left edge, and the hint's
        // centred lines inside a left-aligned label read as a stray indent.
        let title = label("No modules yet", "modules-empty-title");
        title.set_xalign(0.5);
        empty.append(&title);
        let hint = label(
            "A module bundles tasks into one release. It gets its own board with progress and patch notes, and its tasks still show on the main board.",
            "modules-empty-hint",
        );
        hint.set_wrap(true);
        hint.set_max_width_chars(52);
        hint.set_justify(gtk::Justification::Center);
        hint.set_xalign(0.5);
        empty.append(&hint);
        let add = button("", "primary");
        add.set_halign(gtk::Align::Center);
        add.set_child(Some(&icon_text(crate::icons::image("plus", 12).upcast_ref(), "New module")));
        let weak = Rc::downgrade(ui);
        add.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                task_pages::compose_module(&ui, ui.project.get());
            }
        });
        empty.append(&add);
        content.set_vexpand(true);
        content.append(&empty);
        return;
    }

    // The numbers across every module, as the board's stats band shows its own.
    let (done_modules, live): (Vec<&Value>, Vec<&Value>) = modules.iter().partition(|m| m["completed_at"].is_string());
    let tasks: i64 = modules.iter().map(total_of).sum();
    let finished: i64 = modules.iter().map(|m| count_of(m, "done")).sum();
    let flight: i64 = modules.iter().map(|m| count_of(m, "active")).sum();
    let review: i64 = modules.iter().map(|m| count_of(m, "in_review")).sum();
    let band = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    band.add_css_class("board-stats");
    let progress = gtk::Box::new(gtk::Orientation::Vertical, 6);
    progress.add_css_class("stat-tile");
    progress.add_css_class("stat-progress");
    progress.set_hexpand(true);
    let plural = |n: usize, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head.append(&label("Across every module", "stat-caption"));
    head.append(&spacer());
    head.append(&label(&format!("{finished} of {}", plural(tasks.max(0) as usize, "task done", "tasks done")), "stat-note"));
    progress.append(&head);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let pct = if tasks > 0 { finished as f64 * 100. / tasks as f64 } else { 0. };
    row.append(&label(&format!("{pct:.0}%"), "stat-value"));
    let counts: Vec<(&'static str, usize)> = COLUMNS
        .iter()
        .map(|c| (*c, modules.iter().map(|m| count_of(m, c)).sum::<i64>().max(0) as usize))
        .collect();
    row.append(&flow_bar(&counts, 8));
    progress.append(&row);
    let legend = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    legend.add_css_class("stat-legend");
    for (column, count) in &counts {
        let part = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        part.append(&status_icon(column, 10));
        part.append(&label(&format!("{} {count}", caption(column)), "stat-note"));
        if *count == 0 {
            part.add_css_class("stat-zero");
        }
        legend.append(&part);
    }
    progress.append(&legend);
    band.append(&progress);
    let tiles = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    tiles.set_homogeneous(true);
    tiles.set_hexpand(true);
    for tile in [
        stat_tile("Modules", &modules.len().to_string(), &format!("{} in progress", live.len()), ""),
        stat_tile("In flight", &flight.to_string(), if flight > 0 { "Tasks being worked on" } else { "Nothing in flight" }, if flight > 0 { "stat-warm" } else { "" }),
        stat_tile("In review", &review.to_string(), if review > 0 { "Waiting on you" } else { "Nothing waiting" }, if review > 0 { "stat-review" } else { "" }),
        stat_tile("Completed", &done_modules.len().to_string(), &plural(done_modules.len(), "module shipped", "modules shipped"), ""),
    ] {
        tiles.append(&tile);
    }
    band.append(&tiles);
    content.append(&band);

    for (title, group) in [("In progress", &live), ("Completed", &done_modules)] {
        if group.is_empty() {
            continue;
        }
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        heading.add_css_class("modules-section-head");
        heading.append(&label(title, "modules-section-title"));
        heading.append(&label(&group.len().to_string(), "lane-count"));
        content.append(&heading);
        let grid = gtk::FlowBox::new();
        grid.add_css_class("module-grid");
        grid.set_selection_mode(gtk::SelectionMode::None);
        grid.set_homogeneous(true);
        grid.set_min_children_per_line(1);
        grid.set_max_children_per_line(3);
        grid.set_column_spacing(12);
        grid.set_row_spacing(12);
        grid.set_valign(gtk::Align::Start);
        for module in group.iter() {
            grid.insert(&module_card(ui, module, total_of(module), &count_of), -1);
        }
        content.append(&grid);
    }
}

/// A module as a card: its name, priority and state; how far along it is, split by column;
/// then its open and done counts and when it last changed.
fn module_card(ui: &Rc<Ui>, module: &Value, total: i64, count_of: &dyn Fn(&Value, &str) -> i64) -> gtk::Button {
    let card = button("", "module-card");
    card.set_widget_name(&format!("module-card-{}", module["id"]));
    if module["completed_at"].is_string() {
        card.add_css_class("module-complete");
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let mark = crate::icons::image("modules", 14);
    mark.add_css_class("module-card-mark");
    head.append(&mark);
    let name = label(text(module, "name"), "module-card-name");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.set_hexpand(true);
    name.set_xalign(0.);
    head.append(&name);
    if module["completed_at"].is_string() {
        head.append(&label("Completed", "module-card-done"));
    }
    let priority = text(module, "priority");
    let pill = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    pill.add_css_class("field-pill");
    pill.add_css_class(&format!("pill-priority-{priority}"));
    pill.set_valign(gtk::Align::Center);
    pill.append(&priority_icon(priority));
    pill.append(&label(&titled(priority), "field-pill-name"));
    head.append(&pill);
    content.append(&head);

    let pct = module["progress_pct"].as_f64().unwrap_or(0.);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.append(&label(&format!("{pct:.0}%"), "module-card-pct"));
    let counts: Vec<(&'static str, usize)> = COLUMNS.iter().map(|c| (*c, count_of(module, c).max(0) as usize)).collect();
    row.append(&flow_bar(&counts, 6));
    content.append(&row);

    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    columns.add_css_class("module-card-columns");
    for (column, count) in &counts {
        let part = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        part.append(&status_icon(column, 11));
        part.append(&label(&count.to_string(), "module-card-meta"));
        part.set_tooltip_text(Some(caption(column)));
        if *count == 0 {
            part.add_css_class("stat-zero");
        }
        columns.append(&part);
    }
    content.append(&columns);

    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    foot.add_css_class("module-card-foot");
    let open = total - count_of(module, "done");
    foot.append(&label(
        &if total == 0 { "No tasks yet".to_string() } else { format!("{open} open · {total} task{}", if total == 1 { "" } else { "s" }) },
        "module-card-meta",
    ));
    foot.append(&spacer());
    if let Some(when) = crate::relative::ago(text(module, "updated_at"), crate::relative::Form::CompactAgo) {
        foot.append(&label(&format!("Updated {when}"), "module-card-ago"));
    }
    content.append(&foot);
    card.set_child(Some(&content));
    card.set_tooltip_text(Some("Open the module's board"));
    card.set_size_request(320, -1);
    let weak = Rc::downgrade(ui);
    let id = module["id"].as_i64().unwrap_or(0);
    card.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            open_module(&ui, id)
        }
    });
    card
}

/// Each column's ring colour (css/board.css `.status-*`), for drawings that paint several.
fn column_rgb(column: &str) -> (f64, f64, f64) {
    let hex = match column {
        "ready" => 0x4493f8,
        "active" => 0xf0a828,
        "in_review" => 0xab7df8,
        "done" => 0x2ec469,
        _ => 0x8c877f,
    };
    (((hex >> 16) & 0xff) as f64 / 255., ((hex >> 8) & 0xff) as f64 / 255., (hex & 0xff) as f64 / 255.)
}

/// One bar split by how many tasks sit in each column, in board order: the board's mix at a
/// glance. An empty board draws the bare track.
pub(super) fn flow_bar(counts: &[(&'static str, usize)], height: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.add_css_class("flow-bar");
    area.set_content_height(height);
    area.set_hexpand(true);
    area.set_valign(gtk::Align::Center);
    let counts = counts.to_vec();
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    area.set_tooltip_text(Some(
        &counts.iter().filter(|(_, n)| *n > 0).map(|(c, n)| format!("{} {n}", caption(c))).collect::<Vec<_>>().join(" · "),
    ));
    area.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (w as f64, h as f64);
        let r = (h / 2.).min(3.);
        let round = |cr: &cairo::Context, x: f64, width: f64| {
            cr.new_sub_path();
            cr.arc(x + width - r, r, r, -std::f64::consts::FRAC_PI_2, 0.);
            cr.arc(x + width - r, h - r, r, 0., std::f64::consts::FRAC_PI_2);
            cr.arc(x + r, h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
            cr.arc(x + r, r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
            cr.close_path();
        };
        // Glass, as the tile it sits on (theme.css @glass_selected): an ink veil, not a grey bar.
        cr.set_source_rgba(0.93, 0.914, 0.886, 0.09);
        round(cr, 0., w);
        let _ = cr.fill();
        if total == 0 {
            return;
        }
        // Clip to the rounded track, then lay the segments in with a hairline gap between.
        round(cr, 0., w);
        cr.clip();
        let mut x = 0.;
        for (column, n) in &counts {
            if *n == 0 {
                continue;
            }
            let width = w * *n as f64 / total as f64;
            let (red, green, blue) = column_rgb(column);
            cr.set_source_rgba(red, green, blue, if *column == "backlog" { 0.32 } else { 0.9 });
            cr.rectangle(x, 0., (width - 2.).max(1.), h);
            let _ = cr.fill();
            x += width;
        }
    });
    area
}

/// A number with its caption over it and a quiet line under it, as the stats band shows them.
/// Narrower than this, the board shows its progress alone, without the five stat tiles.
const STATS_NARROW: i32 = 980;

pub(super) fn stat_tile(caption: &str, value: &str, note: &str, class: &str) -> gtk::Box {
    let tile = gtk::Box::new(gtk::Orientation::Vertical, 3);
    tile.add_css_class("stat-tile");
    if !class.is_empty() {
        tile.add_css_class(class);
    }
    tile.set_hexpand(true);
    let head = label(caption, "stat-caption");
    head.set_xalign(0.);
    head.set_ellipsize(gtk::pango::EllipsizeMode::End);
    tile.append(&head);
    let number = label(value, "stat-value");
    number.set_xalign(0.);
    tile.append(&number);
    let line = label(note, "stat-note");
    line.set_xalign(0.);
    line.set_ellipsize(gtk::pango::EllipsizeMode::End);
    tile.append(&line);
    tile
}

/// Whether `ts` (RFC 3339) is within the last `days` days.
fn within_days(ts: &str, days: i64) -> bool {
    let (Ok(then), Ok(now)) = (glib::DateTime::from_iso8601(ts, Some(&glib::TimeZone::utc())), glib::DateTime::now_utc()) else {
        return false;
    };
    now.difference(&then).as_seconds() <= days * 86_400
}

fn icon_text(icon: &gtk::Widget, caption: &str) -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(icon);
    if !caption.is_empty() {
        content.append(&label(caption, ""));
    }
    content
}

/// A row that may be wider than a compact window: it clips instead of widening the page.
fn clipped(row: &gtk::Box) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(row)
        .build()
}

fn drop_id(value: &glib::Value) -> Option<i64> {
    value
        .get::<String>()
        .ok()
        .and_then(|s| s.strip_prefix("relay-task:").and_then(|id| id.parse().ok()))
}

struct Quick {
    bar: gtk::Box,
    column: gtk::DropDown,
    title: gtk::Entry,
    kind: gtk::DropDown,
    priority: gtk::DropDown,
    note: gtk::Label,
}

struct Board {
    ui: std::rc::Weak<Ui>,
    project: i64,
    page: gtk::Box,
    keys: gtk::EventControllerKey,
    /// Holds the workspace picker, rebuilt when the projects or workspaces it lists change.
    picker: gtk::Box,
    picker_key: RefCell<String>,
    count: gtk::Label,
    /// Board / Modules: a module's board lights Modules.
    tabs: gtk::Box,
    banner: gtk::Box,
    /// The module in scope, as `module.list` last described it.
    module: RefCell<Option<Value>>,
    shown_scope: Cell<Option<i64>>,
    query: gtk::Entry,
    filter: gtk::MenuButton,
    views: [gtk::ToggleButton; 2],
    /// The stats band: progress across the columns and the numbers that need a look.
    stats: gtk::Box,
    strip: gtk::Box,
    quick: Quick,
    rail: gtk::Box,
    undo: gtk::Button,
    content: gtk::Box,
    peek: gtk::Box,
    tasks: RefCell<Vec<Value>>,
    filters: RefCell<Filters>,
    group: RefCell<String>,
    list: Cell<bool>,
    hidden: RefCell<BTreeSet<String>>,
    focused: Cell<Option<i64>>,
    peeked: Cell<Option<i64>>,
    dragged: Cell<Option<i64>>,
    layout: RefCell<Lanes>,
    /// Each scroller by role (`board`, `lane:<column>`, `list`), to keep places across renders.
    scrolls: RefCell<Vec<(String, gtk::Adjustment)>>,
    /// What the peek last drew (`peek_key`): an unchanged peek keeps its scroll and focus.
    peek_key: RefCell<Option<String>>,
    peek_shown: Cell<Option<i64>>,
    /// A render is waiting for an open card menu or picker to close.
    deferred: Cell<bool>,
    /// Audit rows Ctrl+Z found stale or not undoable: the next press reaches past them.
    skipped: RefCell<BTreeSet<i64>>,
    /// The lanes as drawn, kept while the view, the hidden columns and the empty board stay.
    frame: RefCell<Option<Frame>>,
    /// Every card (or list row) drawn, by task id, with [`card_key`] of what it shows: a render
    /// builds only the cards whose key changed and moves the rest into place.
    cards: RefCell<HashMap<i64, (String, gtk::Box)>>,
    /// What the banner, stats, strip and View key last drew, by part: an unchanged part stays.
    drawn: RefCell<BTreeMap<&'static str, String>>,
    /// The search's pending filter while the typing has not paused.
    search: RefCell<Option<glib::SourceId>>,
    /// Task bodies by id with the `updated_at` they were read at: the board lists summaries,
    /// and reads a body only to search it, peek at it or copy it.
    bodies: RefCell<HashMap<i64, (String, String)>>,
    bodies_loading: Cell<bool>,
    /// What the last body read could not find: no read is started again for those alone.
    bodies_missed: RefCell<BTreeSet<i64>>,
    /// The peek waiting on its task's body.
    peek_loading: Cell<Option<i64>>,
}

/// The lanes (or list sections) as drawn, with what each holds besides cards.
struct Frame {
    /// The view, the hidden columns and whether the board is empty, as [`Board::frame_key`].
    key: String,
    lanes: Vec<Lane>,
}
struct Lane {
    column: &'static str,
    /// Where the cards go: the lane's scrolled box, or the list section itself (its head first).
    items: gtk::Box,
    /// The head, and what it last showed: its count, total and estimate.
    head: gtk::Box,
    head_key: RefCell<String>,
    /// The empty note, by whether a filter emptied the lane.
    empty: RefCell<Option<(bool, gtk::Widget)>>,
    /// Group headings by group.
    headings: RefCell<HashMap<String, gtk::Widget>>,
}

/// Everything a card (or list row) shows of `task`, as text: the card is rebuilt only when this
/// changes. Its board position and edit time are left out — a reorder moves the card and keeps it —
/// and the row's age (`ago`) is put back in, since a row shows it.
fn card_key(task: &Value, all: &BTreeMap<i64, &Value>, project: &str, list: bool, scoped: bool) -> String {
    let mut shown = task.clone();
    if let Some(fields) = shown.as_object_mut() {
        for field in ["position", "updated_at", "body", "changelog"] {
            fields.remove(field);
        }
    }
    let blockers: Vec<Value> = open_blockers(task, all).iter().map(|t| json!([t["id"], text(t, "title")])).collect();
    let duplicate = task["duplicate_of"].as_i64().and_then(|other| all.get(&other)).map(|t| text(t, "title"));
    let age = if list { ago(text(task, "updated_at")) } else { String::new() };
    json!([shown, blockers, duplicate, project, list, scoped, age]).to_string()
}

/// Puts exactly `wanted`, in order, into `items`: a widget already there is moved only when out
/// of place, one from another lane is taken out of it, and what is no longer wanted leaves.
fn place(items: &gtk::Box, wanted: &[gtk::Widget]) {
    let keep: HashSet<&gtk::Widget> = wanted.iter().collect();
    let mut child = items.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if !keep.contains(&widget) {
            items.remove(&widget);
        }
    }
    let mut previous: Option<gtk::Widget> = None;
    for widget in wanted {
        if widget.parent().as_ref() != Some(items.upcast_ref()) {
            detach(widget);
            items.insert_child_after(widget, previous.as_ref());
        } else if widget.prev_sibling() != previous {
            items.reorder_child_after(widget, previous.as_ref());
        }
        previous = Some(widget.clone());
    }
}

/// Removes what a card shows, leaving a menu open on it (a popover is the card's child too).
fn empty_out(card: &gtk::Box) {
    let mut child = card.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if !widget.is::<gtk::Popover>() {
            card.remove(&widget);
        }
    }
}

/// Takes a card out of whichever lane holds it.
fn detach(widget: &impl IsA<gtk::Widget>) {
    match widget.parent().and_downcast::<gtk::Box>() {
        Some(parent) => parent.remove(widget),
        None if widget.parent().is_some() => widget.unparent(),
        None => {}
    }
}

/// Copies a task as the card's copy key always has (`title`, body, `#id`), reading the body
/// first: the board holds summaries.
fn copy_task(ui: &std::rc::Weak<Ui>, id: i64, clipboard: gdk::Clipboard) {
    let Some(ui) = ui.upgrade() else { return };
    glib::spawn_future_local(async move {
        match ui.call("task.get", json!({"task_id":id})).await {
            Ok(task) => {
                clipboard.set_text(&format!("{}\n\n{}\n\n#{id}", text(&task, "title"), text(&task, "body")));
                ui.show_info(&format!("Copied #{id}"));
            }
            Err(error) => ui.show_error(&error.to_string()),
        }
    });
}

/// Mounts the board on `page` for `project` the first time, then re-renders it with `tasks`.
pub fn show(ui: &Rc<Ui>, page: &gtk::Box, project: i64, tasks: Vec<Value>) {
    // Another project's board starts on all of its tasks, not narrowed to a module it lacks.
    if SCOPE.with(|scope| scope.get()).is_some_and(|(owner, _)| owner != project) {
        leave_module();
    }
    let owner = ui.page_projects.borrow().get("board").copied();
    let mut fresh = false;
    let board = match current() {
        Some(board) if owner == Some(project) && board.project == project => board,
        old => {
            fresh = true;
            if let Some(old) = &old {
                page.remove_controller(&old.keys);
            }
            clear(page);
            ui.page_projects.borrow_mut().insert("board".into(), project);
            let board = Board::mount(ui, page, project);
            // The view, hidden columns and grouping follow the person across projects; the
            // filters name one project's modules, labels and agents, so they start empty.
            if let Some(old) = old {
                board.list.set(old.list.get());
                board.views[old.list.get() as usize].set_active(true);
                *board.hidden.borrow_mut() = old.hidden.borrow().clone();
                *board.group.borrow_mut() = old.group.borrow().clone();
            } else if let Some(saved) = SAVED_VIEW.with(|saved| saved.borrow().clone()) {
                // The first board of a run takes up the view the last run left (`load_view`).
                let list = saved["list"].as_bool().unwrap_or(false);
                board.list.set(list);
                board.views[list as usize].set_active(true);
                *board.hidden.borrow_mut() = rows(&saved, "hidden").iter().filter_map(|c| c.as_str().map(str::to_string)).collect();
                *board.group.borrow_mut() = text(&saved, "group").to_string();
            }
            BOARD.with(|slot| *slot.borrow_mut() = Some(board.clone()));
            board
        }
    };
    board.refresh_picker(ui);
    board.load_module(ui);
    let mut tasks = tasks;
    let titles: BTreeMap<i64, Value> = tasks
        .iter()
        .filter_map(|t| t["id"].as_i64().map(|id| (id, t["title"].clone())))
        .collect();
    // `is:blocked` finds what the Blocked tile counts, which takes the other tasks to know.
    let blocked: BTreeSet<i64> = {
        let all = by_id(&tasks);
        tasks.iter().filter(|t| relay_board::is_blocked(t, &all)).filter_map(|t| t["id"].as_i64()).collect()
    };
    for task in &mut tasks {
        if let Some(title) = task["parent_id"].as_i64().and_then(|id| titles.get(&id)) {
            task["_parent_title"] = title.clone();
        }
        if task["id"].as_i64().is_some_and(|id| blocked.contains(&id)) {
            task["_blocked"] = true.into();
        }
    }
    // Most events that refresh the page change no task: leave the board, and any open menu,
    // scroll position and selection on it, alone.
    if !fresh && *board.tasks.borrow() == tasks && board.shown_scope.get() == scope() {
        return;
    }
    *board.tasks.borrow_mut() = tasks;
    board.render_when_closed();
}

impl Board {
    fn mount(ui: &Rc<Ui>, page: &gtk::Box, project: i64) -> Rc<Self> {
        page.add_css_class("board-page");
        page.set_spacing(0);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.add_css_class("board-head");
        let tabs = board_switcher(ui, "board");
        header.append(&tabs);
        let picker = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.append(&picker);
        let count = label("", "board-count");
        count.set_widget_name("board-count");
        count.set_hexpand(true);
        count.set_ellipsize(gtk::pango::EllipsizeMode::End);
        header.append(&count);
        // GitHub's project layout: one wide filter bar under the header, its View menu holding
        // the filters, the grouping and which columns show.
        let filterbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        filterbar.add_css_class("board-filterbar");
        let search = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        search.add_css_class("board-searchbar");
        search.set_hexpand(true);
        search.append(&crate::icons::image("search", 14));
        let query = gtk::Entry::builder()
            .placeholder_text("Search, or filter by field: priority:high  size:M  module:gallery  label:ui  is:open")
            .hexpand(true)
            .has_frame(false)
            .build();
        query.set_widget_name("board-query");
        query.add_css_class("board-search");
        // The board's keys (`Board::key`), which nothing else names.
        query.set_tooltip_text(Some(
            "Search (/)\n\
             On a card: H J K L or the arrows move between cards, Enter opens, Space peeks, \
             . or Menu opens its menu, [ and ] move it a column, Shift+J/K move it up or down, \
             P changes its priority\n\
             G changes the grouping, V the view and F opens the filters; C quick-creates, N opens New task",
        ));
        search.append(&query);
        let wipe = crate::app::icon_button("close", "Clear the search (Esc)");
        wipe.add_css_class("board-search-clear");
        wipe.set_visible(false);
        let field = query.downgrade();
        wipe.connect_clicked(move |_| {
            if let Some(field) = field.upgrade() {
                field.set_text("");
                field.grab_focus();
            }
        });
        search.append(&wipe);
        let focus = gtk::GestureClick::new();
        let field = query.downgrade();
        focus.connect_pressed(move |_, _, _, _| {
            if let Some(field) = field.upgrade() {
                field.grab_focus();
            }
        });
        search.add_controller(focus);
        filterbar.append(&search);
        let filter = gtk::MenuButton::new();
        filter.add_css_class("board-tool");
        filter.set_tooltip_text(Some("Filter, group and choose columns (F)"));
        filter.set_widget_name("board-view");
        let views = [gtk::ToggleButton::new(), gtk::ToggleButton::new()];
        let toggle = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        toggle.add_css_class("board-view-toggle");
        for (index, key) in views.iter().enumerate() {
            let list = index == 1;
            key.set_child(Some(&view_glyph(list)));
            key.set_tooltip_text(Some(if list { "List view (V)" } else { "Board view (V)" }));
            key.update_property(&[gtk::accessible::Property::Label(if list { "List view" } else { "Board view" })]);
            key.set_widget_name(if list { "board-view-list" } else { "board-view-board" });
            if list {
                key.set_group(Some(&views[0]));
            }
            toggle.append(key);
        }
        views[0].set_active(true);
        filterbar.append(&filter);
        filterbar.append(&toggle);
        let undo = crate::app::icon_button("undo", "Undo the last board change (Ctrl+Z)");
        undo.add_css_class("board-tool");
        header.append(&undo);
        let add = button("", "primary");
        add.add_css_class("board-add");
        add.set_child(Some(&icon_text(
            crate::icons::image("plus", 12).upcast_ref(),
            "New task",
        )));
        add.set_tooltip_text(Some("New task (N) · quick-create in place (C)"));
        header.append(&add);
        for bar in [&header, &filterbar] {
            for child in widgets(bar) {
                if child.parent().as_ref() == Some(bar.upcast_ref()) {
                    child.set_valign(gtk::Align::Center);
                }
            }
        }
        page.append(&header);
        let banner = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        banner.add_css_class("module-banner");
        banner.set_visible(false);
        page.append(&banner);
        page.append(&filterbar);
        let stats = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        stats.add_css_class("board-stats");
        stats.set_visible(false);
        // As wide as the page and no taller: its resize says when the page crosses STATS_NARROW.
        let width_probe = gtk::DrawingArea::new();
        width_probe.set_hexpand(true);
        width_probe.set_content_height(0);
        page.append(&width_probe);
        page.append(&stats);

        // Active filters and the grouping, as removable chips; hidden while there are none.
        let strip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        strip.add_css_class("board-strip");
        let strip_row = clipped(&strip);
        strip_row.set_visible(false);
        page.append(&strip_row);

        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bar.add_css_class("board-quick");
        let column = gtk::DropDown::from_strings(&OPEN_COLUMNS.iter().map(|c| caption(c)).collect::<Vec<_>>());
        column.set_tooltip_text(Some("Column"));
        let title = gtk::Entry::builder()
            .placeholder_text("Task title — Enter creates, Esc closes")
            .hexpand(true)
            .build();
        title.set_widget_name("board-quick-title");
        // Titled and most urgent first, as the New task page offers them.
        let kind = gtk::DropDown::from_strings(&TYPES.map(titled).iter().map(String::as_str).collect::<Vec<_>>());
        kind.set_tooltip_text(Some("Type"));
        let priority = gtk::DropDown::from_strings(&URGENT_FIRST.map(titled).iter().map(String::as_str).collect::<Vec<_>>());
        priority.set_selected(URGENT_FIRST.iter().position(|p| *p == "medium").unwrap_or(0) as u32);
        priority.set_tooltip_text(Some("Priority"));
        let create = button("Create", "primary");
        let more = button("More fields…", "quiet");
        let close = crate::app::icon_button("close", "Close (Esc)");
        let note = label("", "board-quick-note");
        note.set_ellipsize(gtk::pango::EllipsizeMode::End);
        note.set_max_width_chars(40);
        for widget in [
            column.upcast_ref::<gtk::Widget>(),
            title.upcast_ref(),
            kind.upcast_ref(),
            priority.upcast_ref(),
            note.upcast_ref(),
            create.upcast_ref(),
            more.upcast_ref(),
            close.upcast_ref(),
        ] {
            widget.set_valign(gtk::Align::Center);
            bar.append(widget);
        }
        bar.set_visible(false);
        page.append(&bar);

        let rail = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        rail.set_widget_name("board-drag-rail");
        rail.add_css_class("board-drag-rail");
        rail.append(&label("DROP TO SET", "board-rail-caption"));
        rail.set_visible(false);
        // Floats over the lanes: shown at drag start, it must not push the card under the pointer.
        rail.set_valign(gtk::Align::Start);
        rail.set_halign(gtk::Align::Start);

        let main = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        main.set_vexpand(true);
        main.add_css_class("board-main");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_hexpand(true);
        content.set_vexpand(true);
        main.append(&content);
        let peek = gtk::Box::new(gtk::Orientation::Vertical, 0);
        peek.add_css_class("board-peek");
        peek.set_size_request(380, -1);
        peek.set_visible(false);
        main.append(&peek);
        let stack = gtk::Overlay::new();
        stack.set_child(Some(&main));
        stack.add_overlay(&rail);
        page.append(&stack);


        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let board = Rc::new(Self {
            ui: Rc::downgrade(ui),
            project,
            page: page.clone(),
            keys: keys.clone(),
            picker,
            picker_key: RefCell::new(String::new()),
            count,
            tabs,
            banner: banner.clone(),
            module: RefCell::new(None),
            shown_scope: Cell::new(None),
            query: query.clone(),
            filter: filter.clone(),
            views: views.clone(),
            stats,
            strip,
            quick: Quick {
                bar,
                column,
                title: title.clone(),
                kind,
                priority,
                note,
            },
            rail: rail.clone(),
            undo: undo.clone(),
            content,
            peek,
            tasks: RefCell::new(Vec::new()),
            filters: RefCell::new(Filters::new()),
            group: RefCell::new(String::new()),
            list: Cell::new(false),
            hidden: RefCell::new(BTreeSet::new()),
            focused: Cell::new(None),
            peeked: Cell::new(None),
            dragged: Cell::new(None),
            layout: RefCell::new(Vec::new()),
            scrolls: RefCell::new(Vec::new()),
            peek_key: RefCell::new(None),
            peek_shown: Cell::new(None),
            deferred: Cell::new(false),
            skipped: RefCell::new(BTreeSet::new()),
            frame: RefCell::new(None),
            cards: RefCell::new(HashMap::new()),
            drawn: RefCell::new(BTreeMap::new()),
            search: RefCell::new(None),
            bodies: RefCell::new(HashMap::new()),
            bodies_loading: Cell::new(false),
            bodies_missed: RefCell::new(BTreeSet::new()),
            peek_loading: Cell::new(None),
        });

        // The stat tiles are drawn for the page's width: the first render runs before the page
        // has one, and drew five tiles a 1024px window squeezed to "I…". Crossing the line
        // draws them again.
        let weak = Rc::downgrade(&board);
        let narrow = Cell::new(None);
        width_probe.connect_resize(move |_, w, _| {
            let now = w < STATS_NARROW;
            if narrow.replace(Some(now)) != Some(now) {
                let weak = weak.clone();
                glib::idle_add_local_once(move || {
                    if let Some(board) = weak.upgrade() {
                        board.render();
                    }
                });
            }
        });

        for (field, values) in [
            ("priority", &PRIORITIES[..]),
            ("size", &SIZES[1..]),
            ("type", &TYPES[..]),
        ] {
            for value in values {
                let key = button("", "quiet");
                let mark: gtk::Widget = match field {
                    "priority" => priority_icon(value).upcast(),
                    "type" => task_mark(value, 9).upcast(),
                    _ => label("", "").upcast(),
                };
                key.set_child(Some(&icon_text(&mark, &titled(value))));
                let drop = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
                let weak = Rc::downgrade(&board);
                drop.connect_drop(move |_, token, _, _| {
                    let (Some(board), Some(id)) = (weak.upgrade(), drop_id(token)) else {
                        return false;
                    };
                    board.act("task.update", json!({"task_id":id, field:value}));
                    true
                });
                key.add_controller(drop);
                rail.append(&key);
            }
        }

        let weak = Rc::downgrade(&board);
        let clear_key = wipe.downgrade();
        query.connect_changed(move |entry| {
            if let Some(key) = clear_key.upgrade() {
                key.set_visible(!entry.text().is_empty());
            }
            // Filter once the typing pauses, not on every key.
            let Some(board) = weak.upgrade() else { return };
            if let Some(pending) = board.search.borrow_mut().take() {
                pending.remove();
            }
            let weak = Rc::downgrade(&board);
            let pending = glib::timeout_add_local_once(SEARCH_PAUSE, move || {
                if let Some(board) = weak.upgrade() {
                    board.search.borrow_mut().take();
                    board.render();
                }
            });
            *board.search.borrow_mut() = Some(pending);
        });
        let escape = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&board);
        escape.connect_key_pressed(move |controller, key, _, _| {
            if key != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            if let Some(entry) = controller.widget().and_downcast::<gtk::Entry>() {
                entry.set_text("");
            }
            if let Some(board) = weak.upgrade() {
                board.focus_board();
            }
            glib::Propagation::Stop
        });
        query.add_controller(escape);
        let popover = gtk::Popover::new();
        popover.add_css_class("board-menu");
        popover.add_css_class("view-popover");
        popover.set_has_arrow(false);
        filter.set_popover(Some(&popover));
        let weak = Rc::downgrade(&board);
        filter.set_create_popup_func(move |_| {
            if let Some(board) = weak.upgrade() {
                board.filter_menu(&popover);
            }
        });
        let weak = Rc::downgrade(&board);
        views[1].connect_toggled(move |key| {
            if let Some(board) = weak.upgrade() {
                if board.list.replace(key.is_active()) != key.is_active() {
                    board.render();
                }
            }
        });
        let weak = Rc::downgrade(&board);
        undo.connect_clicked(move |key| {
            if let Some(board) = weak.upgrade() {
                board.undo_last(key);
            }
        });
        let weak = Rc::downgrade(&board);
        add.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                // The click took focus from the card, so start in the last focused card's column.
                let column = board.focused.get().and_then(|id| board.task(id)).map(|t| text(&t, "column").to_string());
                board.compose(column.as_deref().unwrap_or("backlog"));
            }
        });
        let weak = Rc::downgrade(&board);
        title.connect_activate(move |_| {
            if let Some(board) = weak.upgrade() {
                board.quick_create();
            }
        });
        let weak = Rc::downgrade(&board);
        create.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                board.quick_create();
            }
        });
        let weak = Rc::downgrade(&board);
        close.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                board.close_quick();
            }
        });
        let weak = Rc::downgrade(&board);
        more.connect_clicked(move |_| {
            if let (Some(board), Some(ui)) = (weak.upgrade(), weak.upgrade().and_then(|b| b.ui.upgrade())) {
                board.close_quick();
                let column = OPEN_COLUMNS[(board.quick.column.selected() as usize).min(OPEN_COLUMNS.len() - 1)];
                task_pages::compose(&ui, board.project, column, scope());
            }
        });
        let escape = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&board);
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                if let Some(board) = weak.upgrade() {
                    board.close_quick();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        board.quick.bar.add_controller(escape);

        let weak = Rc::downgrade(&board);
        keys.connect_key_pressed(move |_, key, keycode, mods| {
            let Some(board) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            // On a non-Latin layout a letter shortcut is the Latin key in the same place.
            board.key(crate::shortcuts::latin(key, keycode), mods)
        });
        page.add_controller(keys);
        board
    }

    fn ui(&self) -> Option<Rc<Ui>> {
        self.ui.upgrade()
    }
    /// Rebuilds the workspace picker once a project or workspace it lists is added, renamed or
    /// removed, unless its menu is open.
    fn refresh_picker(&self, ui: &Rc<Ui>) {
        let key = {
            let pick = |rows: &[Value], keys: &[&str]| -> Vec<Value> {
                rows.iter().map(|row| Value::Array(keys.iter().map(|k| row[*k].clone()).collect())).collect()
            };
            json!([pick(&ui.projects.borrow(), &["id", "name", "workspace_id"]), pick(&ui.workspaces.borrow(), &["id", "name"])])
                .to_string()
        };
        if *self.picker_key.borrow() == key || crate::app::open_popover(self.picker.upcast_ref()).is_some() {
            return;
        }
        *self.picker_key.borrow_mut() = key;
        clear(&self.picker);
        self.picker.append(&workspace_picker(ui, self.project, "board"));
    }
    fn task(&self, id: i64) -> Option<Value> {
        self.tasks
            .borrow()
            .iter()
            .find(|t| t["id"].as_i64() == Some(id))
            .cloned()
    }
    /// Every live task of `column` in board order, filters ignored: the order positions index.
    fn column_order(&self, column: &str) -> Vec<i64> {
        relay_board::column_order(&self.tasks.borrow(), column)
    }

    fn act(self: &Rc<Self>, op: &'static str, payload: Value) {
        let Some(ui) = self.ui() else { return };
        glib::spawn_future_local(async move {
            if let Err(error) = ui.call(op, payload).await {
                ui.show_error(&error.to_string());
            }
            ui.refresh_page();
        });
    }
    /// Moves a task to `column` (at a final index when given). Done is reached by approval,
    /// exactly as the task editor's Approve: the engine links the last agent's branch tip, or
    /// the project HEAD for a task that was never dispatched.
    fn move_to(self: &Rc<Self>, task: &Value, column: &str, position: Option<usize>) {
        let from = text(task, "column");
        if column == "done" {
            if from != "done" {
                let Some(ui) = self.ui() else { return };
                let id = task["id"].clone();
                // Said once it has happened, as news rather than an error.
                glib::spawn_future_local(async move {
                    match ui.call("task.approve", json!({"task_id":id})).await {
                        Ok(_) => ui.show_info(&format!("Approved #{id} into Done · Ctrl+Z moves it back; the linked commit stays")),
                        Err(error) => ui.show_error(&error.to_string()),
                    }
                    ui.refresh_page();
                });
            }
            return;
        }
        // Picking the column a task is already in would append it to the bottom.
        if column == from && position.is_none() {
            return;
        }
        // Moved here at once, so a key pressed again before the engine's refresh lands builds
        // on this move; that refresh confirms or corrects it.
        let id = task["id"].as_i64();
        let mut order = self.column_order(column);
        order.retain(|other| Some(*other) != id);
        if let Some(id) = id {
            order.insert(position.unwrap_or(order.len()).min(order.len()), id);
        }
        for t in self.tasks.borrow_mut().iter_mut() {
            if t["id"].as_i64() == id {
                t["column"] = column.into();
            }
            if let Some(at) = t["id"].as_i64().and_then(|i| order.iter().position(|o| *o == i)) {
                t["position"] = json!(at);
            }
        }
        self.render_when_closed();
        let mut payload = json!({"task_id":task["id"],"column":column});
        if let Some(at) = position {
            payload["position"] = json!(at);
        }
        self.act("task.move", payload);
    }
    /// Sets one field, applied locally and redrawn at once like [`Self::move_to`]. Picking the
    /// value a task already has writes nothing: a no-op update would only stale its undo rows.
    fn set_field(self: &Rc<Self>, id: i64, field: &'static str, value: Value) {
        {
            let mut tasks = self.tasks.borrow_mut();
            let Some(task) = tasks.iter_mut().find(|t| t["id"].as_i64() == Some(id)) else { return };
            if task[field] == value {
                return;
            }
            task[field] = value.clone();
        }
        self.render_when_closed();
        self.act("task.update", json!({"task_id":id, field:value}));
    }
    fn drop_on(self: &Rc<Self>, moving: i64, onto: i64, zone: Zone) -> bool {
        let (Some(task), Some(target)) = (self.task(moving), self.task(onto)) else {
            return false;
        };
        if moving == onto {
            return false;
        }
        if zone == Zone::Into {
            self.act("task.parent.set", json!({"task_id":moving,"parent_id":onto}));
            return true;
        }
        let column = text(&target, "column").to_string();
        if column == "done" && text(&task, "column") == "done" {
            return false;
        }
        let at = drop_index(&self.column_order(&column), moving, onto, zone == Zone::After);
        self.focused.set(Some(moving));
        self.move_to(&task, &column, Some(at));
        true
    }

    fn undo_last(self: &Rc<Self>, key: &gtk::Button) {
        let Some(ui) = self.ui() else { return };
        let project = self.project;
        key.set_sensitive(false);
        let key = key.clone();
        let board = self.clone();
        glib::spawn_future_local(async move {
            let result = async {
                let history = ui
                    .call(
                        "audit.list",
                        json!({"project_id":project,"actor":"user","op_prefix":"task.","limit":50}),
                    )
                    .await?;
                let skipped = board.skipped.borrow().clone();
                match rows(&history, "rows").iter().find(|r| {
                    !r["undo_op"].is_null()
                        && r["undone_by"].is_null()
                        && !r["id"].as_i64().is_some_and(|id| skipped.contains(&id))
                }) {
                    Some(row) => {
                        let what = format!("{} on #{}", text(row, "op"), row["payload"]["task_id"]);
                        match ui.call("audit.undo", json!({"audit_id":row["id"]})).await {
                            // What was undone, as the task's timeline says it.
                            Ok(_) => ui.show_info(&match (relay_board::activity_sentence(row), row["payload"]["task_id"].as_i64()) {
                                (Some(sentence), Some(id)) => format!("Undid #{id}: {sentence}"),
                                _ => format!("Undid {what}"),
                            }),
                            // A row that cannot be undone must not block every older one.
                            Err(crate::client::Error::Bus(error))
                                if matches!(error.code.as_str(), "audit.stale" | "audit.not_undoable") =>
                            {
                                if let Some(id) = row["id"].as_i64() {
                                    board.skipped.borrow_mut().insert(id);
                                }
                                ui.show_error(&format!(
                                    "Cannot undo {what}: {}. Press Ctrl+Z again for the change before it.",
                                    error.message
                                ));
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    None => ui.show_error("Nothing on the board to undo."),
                }
                Ok::<(), crate::client::Error>(())
            }
            .await;
            if let Err(error) = result {
                ui.show_error(&error.to_string());
            }
            key.set_sensitive(true);
            ui.refresh_page();
        });
    }

    fn open_quick(self: &Rc<Self>, column: &str) {
        let at = OPEN_COLUMNS.iter().position(|c| *c == column).unwrap_or(0);
        self.quick.column.set_selected(at as u32);
        self.quick.note.set_text("");
        self.quick.bar.set_visible(true);
        self.quick.title.grab_focus();
    }
    /// The full New task page, starting in `column` (Done is reached by approval, so Backlog).
    fn compose(self: &Rc<Self>, column: &str) {
        if let Some(ui) = self.ui() {
            let column = if OPEN_COLUMNS.contains(&column) { column } else { "backlog" };
            task_pages::compose(&ui, self.project, column, scope());
        }
    }
    fn close_quick(self: &Rc<Self>) {
        self.quick.bar.set_visible(false);
        self.quick.title.set_text("");
        self.focus_board();
    }
    fn quick_create(self: &Rc<Self>) {
        let Some(ui) = self.ui() else { return };
        let title = self.quick.title.text().trim().to_string();
        if title.is_empty() {
            self.quick.title.grab_focus();
            return;
        }
        let pick = |control: &gtk::DropDown, values: &[&'static str]| {
            values[(control.selected() as usize).min(values.len() - 1)]
        };
        let payload = json!({
            "project_id": self.project,
            "title": title,
            "column": pick(&self.quick.column, OPEN_COLUMNS),
            "type": pick(&self.quick.kind, &TYPES),
            "priority": pick(&self.quick.priority, &URGENT_FIRST),
            "module_id": scope(),
        });
        // The whole bar, Create included, waits for the reply: a second click would create twice.
        self.quick.bar.set_sensitive(false);
        let board = self.clone();
        glib::spawn_future_local(async move {
            match ui.call("task.create", payload).await {
                Ok(task) => {
                    if board.quick.title.text().trim() == title {
                        board.quick.title.set_text("");
                    }
                    board.quick.note.set_text(&format!("Created #{}", task["id"]));
                    board.focused.set(task["id"].as_i64());
                }
                Err(error) => board.quick.note.set_text(&error.to_string()),
            }
            board.quick.bar.set_sensitive(true);
            board.quick.title.grab_focus();
            ui.refresh_page();
        });
    }

    fn focus_board(self: &Rc<Self>) {
        let layout = self.layout.borrow();
        let cards = layout.iter().flat_map(|(_, cards)| cards.iter());
        let mut cards: Vec<_> = cards.collect();
        if let Some(id) = self.focused.get() {
            cards.sort_by_key(|(card, _)| *card != id);
        }
        if let Some((_, card)) = cards.first() {
            card.grab_focus();
        } else {
            self.content.grab_focus();
        }
    }
    /// The focused card as (column, index in its lane), if focus is on one. A button inside the
    /// card, or its menu, is not the card: Enter and Space press it.
    fn current(&self) -> Option<(String, usize)> {
        let focus = self.page.root().and_then(|root| root.focus())?;
        self.layout.borrow().iter().find_map(|(column, cards)| {
            cards
                .iter()
                .position(|(_, card)| focus == *card)
                .map(|at| (column.clone(), at))
        })
    }
    fn current_id(&self) -> Option<i64> {
        let (column, at) = self.current()?;
        self.layout
            .borrow()
            .iter()
            .find(|(c, _)| *c == column)
            .and_then(|(_, cards)| cards.get(at).map(|(id, _)| *id))
    }
    fn focus_at(&self, lane: usize, at: usize) {
        if let Some((_, cards)) = self.layout.borrow().get(lane) {
            if let Some((id, card)) = cards.get(at.min(cards.len().saturating_sub(1))) {
                self.focused.set(Some(*id));
                card.grab_focus();
            }
        }
    }

    fn key(self: &Rc<Self>, key: gdk::Key, mods: gdk::ModifierType) -> glib::Propagation {
        use gdk::Key;
        let focus = self.page.root().and_then(|root| root.focus());
        let mut ancestor = focus.clone();
        while let Some(widget) = ancestor {
            // Menus and pickers keep their own keys: arrows, Enter and Escape act inside them.
            if widget.is::<gtk::Editable>() || widget.is::<gtk::TextView>() || widget.is::<gtk::Popover>() {
                return glib::Propagation::Proceed;
            }
            ancestor = widget.parent();
        }
        // Caps Lock sends uppercase letters without Shift: the intent to reorder is Shift alone.
        let key = key.to_lower();
        let shift = mods.contains(gdk::ModifierType::SHIFT_MASK);
        if mods.contains(gdk::ModifierType::CONTROL_MASK) && !shift && key == Key::z {
            // emit_clicked runs even while an undo is in flight and the key is insensitive.
            if self.undo.is_sensitive() {
                self.undo.emit_clicked();
            }
            return glib::Propagation::Stop;
        }
        if mods.intersects(
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK,
        ) {
            return glib::Propagation::Proceed;
        }
        let lanes: Vec<(String, usize)> = self
            .layout
            .borrow()
            .iter()
            .map(|(c, cards)| (c.clone(), cards.len()))
            .collect();
        let here = self.current();
        let lane = here
            .as_ref()
            .and_then(|(c, _)| lanes.iter().position(|(name, _)| name == c));
        let id = self.current_id();
        let task = id.and_then(|id| self.task(id));
        // Card commands only claim their key while a card has focus, so Enter and Space
        // still press a focused button in the header, the strip or the peek panel.
        let card_command = matches!(
            key,
            Key::Return | Key::KP_Enter | Key::ISO_Enter | Key::space | Key::period | Key::Menu
                | Key::bracketleft | Key::bracketright | Key::p
        ) || (shift && matches!(key, Key::j | Key::k | Key::Up | Key::Down));
        if card_command && task.is_none() {
            return glib::Propagation::Proceed;
        }
        match key {
            Key::Escape => {
                if self.quick.bar.is_visible() {
                    self.close_quick();
                } else if self.peeked.get().is_some() {
                    self.peeked.set(None);
                    self.render_peek();
                    self.focus_board();
                } else {
                    return glib::Propagation::Proceed;
                }
            }
            Key::j | Key::k if shift => self.nudge(task.as_ref(), if key == Key::j { 1 } else { -1 }),
            Key::Down | Key::Up if shift => {
                self.nudge(task.as_ref(), if key == Key::Down { 1 } else { -1 })
            }
            Key::j | Key::k | Key::Down | Key::Up => {
                let delta: i64 = if matches!(key, Key::k | Key::Up) { -1 } else { 1 };
                match (lane, here.as_ref()) {
                    (Some(lane), Some((_, at))) => {
                        let next = *at as i64 + delta;
                        if self.list.get() && (next < 0 || next >= lanes[lane].1 as i64) {
                            // The list reads as one sequence: step into the next section.
                            let mut l = lane as i64 + delta;
                            while (0..lanes.len() as i64).contains(&l) {
                                let count = lanes[l as usize].1;
                                if count > 0 {
                                    self.focus_at(l as usize, if delta > 0 { 0 } else { count - 1 });
                                    break;
                                }
                                l += delta;
                            }
                        } else {
                            self.focus_at(lane, next.max(0) as usize);
                        }
                    }
                    _ => self.focus_board(),
                }
            }
            Key::h | Key::l | Key::Left | Key::Right => {
                let delta: i64 = if matches!(key, Key::h | Key::Left) { -1 } else { 1 };
                let at = here.as_ref().map(|(_, at)| *at).unwrap_or(0);
                let mut l = lane.map(|l| l as i64).unwrap_or(if delta > 0 { -1 } else { lanes.len() as i64 }) + delta;
                while (0..lanes.len() as i64).contains(&l) {
                    if lanes[l as usize].1 > 0 {
                        self.focus_at(l as usize, at);
                        break;
                    }
                    l += delta;
                }
            }
            Key::Return | Key::KP_Enter | Key::ISO_Enter => {
                if let (Some(id), Some(ui)) = (id, self.ui()) {
                    task_pages::open(&ui, id);
                }
            }
            Key::space => {
                if let Some(id) = id {
                    self.toggle_peek(id);
                }
            }
            Key::c => {
                let column = here.map(|(c, _)| c);
                self.open_quick(column.as_deref().unwrap_or("backlog"));
            }
            Key::n => {
                let column = here.map(|(c, _)| c);
                self.compose(column.as_deref().unwrap_or("backlog"));
            }
            Key::slash => {
                self.query.grab_focus();
            }
            Key::f => self.filter.popup(),
            Key::v => {
                let list = !self.list.get();
                self.views[list as usize].set_active(true);
            }
            Key::g => {
                let now = self.group.borrow().clone();
                let at = GROUPS.iter().position(|(g, _)| *g == now).unwrap_or(0);
                *self.group.borrow_mut() = GROUPS[(at + 1) % GROUPS.len()].0.to_string();
                self.render();
            }
            Key::bracketleft | Key::bracketright => {
                if let Some(task) = task {
                    let at = COLUMNS.iter().position(|c| *c == text(&task, "column")).unwrap_or(0) as i64;
                    let next = (at + if key == Key::bracketleft { -1 } else { 1 }).clamp(0, COLUMNS.len() as i64 - 1);
                    if next != at {
                        self.focused.set(task["id"].as_i64());
                        self.move_to(&task, COLUMNS[next as usize], None);
                    }
                }
            }
            // 1 to 5: straight to Backlog, Ready, Active, In review or Done, as lific's keys.
            Key::_1 | Key::_2 | Key::_3 | Key::_4 | Key::_5 if task.is_some() => {
                let task = task.unwrap();
                let at = match key {
                    Key::_1 => 0,
                    Key::_2 => 1,
                    Key::_3 => 2,
                    Key::_4 => 3,
                    _ => 4,
                };
                if let Some(column) = COLUMNS.get(at).filter(|c| **c != text(&task, "column")) {
                    self.focused.set(task["id"].as_i64());
                    self.move_to(&task, column, None);
                }
            }
            Key::p => {
                if let Some(task) = task {
                    let at = PRIORITIES.iter().position(|p| *p == text(&task, "priority")).unwrap_or(1);
                    if let Some(id) = task["id"].as_i64() {
                        self.focused.set(Some(id));
                        self.set_field(id, "priority", json!(PRIORITIES[(at + 1) % PRIORITIES.len()]));
                    }
                }
            }
            Key::period | Key::Menu => {
                if let (Some(id), Some(focus)) = (id, focus) {
                    let card = self.layout.borrow().iter().find_map(|(_, cards)| {
                        cards.iter().find(|(i, _)| *i == id).map(|(_, w)| w.clone())
                    });
                    self.menu(card.as_ref().unwrap_or(&focus), id, None);
                }
            }
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    }
    /// Shift+J/K: the focused card swaps places with its neighbour in the lane as shown, which
    /// leaves out filtered tasks and orders by group first.
    fn nudge(self: &Rc<Self>, task: Option<&Value>, delta: i64) {
        let Some(task) = task else { return };
        let Some(id) = task["id"].as_i64() else { return };
        let column = text(task, "column").to_string();
        if column == "done" {
            return;
        }
        let step = {
            let tasks = self.tasks.borrow();
            let all = by_id(&tasks);
            let layout = self.layout.borrow();
            let lane: Vec<&Value> = layout
                .iter()
                .find(|(lane, _)| *lane == column)
                .map(|(_, cards)| cards.iter().filter_map(|(card, _)| all.get(card).copied()).collect())
                .unwrap_or_default();
            relay_board::nudge(&lane, &relay_board::column_order(&tasks, &column), id, delta, &self.group.borrow())
        };
        match step {
            Nudge::Edge => {}
            Nudge::CrossesGroup => {
                if let Some(ui) = self.ui() {
                    ui.show_error("Cards keep to their group: change the task or the grouping to move it past one.");
                }
            }
            Nudge::To(at) => {
                self.focused.set(Some(id));
                self.move_to(task, &column, Some(at));
            }
        }
    }

    fn toggle_peek(self: &Rc<Self>, id: i64) {
        self.peeked.set(if self.peeked.get() == Some(id) { None } else { Some(id) });
        self.focused.set(Some(id));
        self.render_peek();
    }

    /// Render now, or once an open card menu or peek picker closes: a render destroys the
    /// widgets those popovers hang from.
    fn render_when_closed(self: &Rc<Self>) {
        let Some(popover) = crate::app::open_popover(self.page.upcast_ref()) else {
            self.deferred.set(false);
            self.render();
            return;
        };
        if !self.deferred.replace(true) {
            // One close, one render: the handler goes once it has run, or a long-lived menu
            // (View, a lane's, the workspace picker) would gather one per deferral.
            let handler: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::default();
            let (weak, slot) = (Rc::downgrade(self), handler.clone());
            let id = popover.connect_closed(move |popover| {
                if let Some(id) = slot.borrow_mut().take() {
                    popover.disconnect(id);
                }
                let Some(board) = weak.upgrade() else { return };
                if board.deferred.get() {
                    // The popover is still mapped while `closed` runs.
                    glib::idle_add_local_once(move || board.render_when_closed());
                }
            });
            *handler.borrow_mut() = Some(id);
        }
    }

    /// Every change to the view, the grouping or the hidden columns redraws the board, which
    /// writes it to [`VIEW_KEY`] a moment after the last one, once the saved view was read.
    fn keep_view(&self) {
        let view = view_of(self.list.get(), &self.group.borrow(), &self.hidden.borrow());
        let changed = SAVED_VIEW.with(|saved| {
            let mut saved = saved.borrow_mut();
            match saved.as_ref() {
                Some(old) if *old != view => {
                    *saved = Some(view);
                    true
                }
                _ => false,
            }
        });
        if !changed {
            return;
        }
        if let Some(timer) = VIEW_TIMER.with(|timer| timer.borrow_mut().take()) {
            timer.remove();
        }
        let ui = self.ui.clone();
        let timer = glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
            VIEW_TIMER.with(|timer| timer.borrow_mut().take());
            let (Some(ui), Some(value)) = (ui.upgrade(), SAVED_VIEW.with(|saved| saved.borrow().clone())) else { return };
            glib::spawn_future_local(async move {
                let _ = ui.call("settings.set", json!({"path": VIEW_KEY, "value": value})).await;
            });
        });
        VIEW_TIMER.with(|slot| *slot.borrow_mut() = Some(timer));
    }

    /// Redraws the board from `tasks`. Only what changed is touched: the lanes stay while the
    /// view, the hidden columns and the empty board do, each card is rebuilt only when what it
    /// shows changed ([`card_key`]) and is otherwise moved into place, and the banner, stats and
    /// chips are redrawn only when they read differently.
    fn render(self: &Rc<Self>) {
        self.keep_view();
        let focus_inside = self
            .page
            .root()
            .and_then(|root| root.focus())
            .is_some_and(|focus| focus.is_ancestor(&self.content));
        let everything = self.tasks.borrow().clone();
        let scope = scope();
        self.shown_scope.set(scope);
        let tasks: Vec<Value> = everything
            .iter()
            .filter(|t| scope.is_none() || t["module_id"].as_i64() == scope)
            .cloned()
            .collect();
        self.render_banner(&tasks);
        for (index, tab) in widgets(&self.tabs).into_iter().filter(|w| w.parent().as_ref() == Some(self.tabs.upcast_ref())).enumerate() {
            if (index == 1) == scope.is_some() {
                tab.add_css_class("selected");
            } else {
                tab.remove_css_class("selected");
            }
        }
        let filters = self.filters.borrow().clone();
        let query = self.query.text().trim().to_string();
        let visible: Vec<&Value> = {
            // The search reads bodies too; the board lists summaries, so a body read for the
            // search stands in for the empty one.
            let bodies = self.bodies.borrow();
            tasks
                .iter()
                .filter(|t| {
                    task_matches(t, &filters, "")
                        && match Self::fresh(&bodies, t).filter(|_| !query.is_empty()) {
                            Some(body) => {
                                let mut whole = (*t).clone();
                                whole["body"] = body.into();
                                relay_board::query_matches(&whole, &query)
                            }
                            None => relay_board::query_matches(t, &query),
                        }
                })
                .collect()
        };
        if !query.is_empty() {
            self.read_bodies();
        }
        let open = tasks.iter().filter(|t| text(t, "column") != "done").count();
        let mut summary = format!("{open} open · {} done", tasks.len() - open);
        if visible.len() != tasks.len() {
            summary.push_str(&format!(" · {} shown", visible.len()));
        }
        self.count.set_text(&summary);
        let active = filters.values().map(BTreeSet::len).sum::<usize>();
        if self.redraw("view", active.to_string()) {
            self.filter.set_child(Some(&icon_text(
                crate::icons::image("sliders", 12).upcast_ref(),
                &if active > 0 { format!("View · {active}") } else { "View".into() },
            )));
        }
        if active > 0 {
            self.filter.add_css_class("active");
        } else {
            self.filter.remove_css_class("active");
        }
        self.render_stats(&tasks, &everything);
        self.render_strip(&tasks);
        let key = self.frame_key(everything.is_empty());
        let rebuilt = self.frame.borrow().as_ref().is_none_or(|frame| frame.key != key);
        if rebuilt {
            let saved: BTreeMap<String, f64> = self.scrolls.borrow().iter().map(|(role, a)| (role.clone(), a.value())).collect();
            self.build_frame(key, everything.is_empty());
            // Keep each scroller where it was: restore once the new content has its size. A
            // scroller the last frame did not have (another view, a column shown again) starts
            // at the top.
            for (role, adjustment) in self.scrolls.borrow().iter() {
                let Some(value) = saved.get(role).copied().filter(|v| *v > 0.) else {
                    continue;
                };
                let done = Cell::new(false);
                adjustment.connect_changed(move |a| {
                    if !done.get() && a.upper() - a.page_size() >= value {
                        done.set(true);
                        a.set_value(value);
                    }
                });
            }
        }
        // The focused card and the lane it is in, to tell a card kept in place from one moved.
        let focused_card = || {
            let id = self.focused.get();
            self.layout.borrow().iter().find_map(|(_, cards)| {
                cards.iter().find(|(i, _)| Some(*i) == id).map(|(_, w)| w.clone())
            })
        };
        let held = focused_card().map(|card| (card.parent(), card));
        if !everything.is_empty() {
            self.render_lanes(&tasks, &everything, &visible);
        }
        if focus_inside {
            let focus = self.page.root().and_then(|root| root.focus());
            match focused_card() {
                Some(card) => {
                    let stayed = held.as_ref().is_some_and(|(parent, was)| *was == card && card.parent() == *parent);
                    // A card kept in place keeps its focus, and a key inside it keeps its own.
                    if !stayed || !focus.is_some_and(|f| f == card || f.is_ancestor(&card)) {
                        if !stayed {
                            // Taken out of its lane, the card still reads as the focus until
                            // GTK moves it on: drop that first, or the grab below is a no-op.
                            if let Some(root) = card.root() {
                                root.set_focus(None::<&gtk::Widget>);
                            }
                        }
                        card.grab_focus();
                    }
                }
                None if focus.is_some_and(|f| f.is_ancestor(&self.content)) => {}
                None => self.focus_board(),
            }
        }
        self.render_peek();
    }

    /// Whether the part `part` must be drawn again to read as `key`, noting it as drawn if so.
    fn redraw(&self, part: &'static str, key: String) -> bool {
        let mut drawn = self.drawn.borrow_mut();
        if drawn.get(part) == Some(&key) {
            return false;
        }
        drawn.insert(part, key);
        true
    }

    /// What the lanes themselves depend on: the view, the hidden columns, an empty board.
    fn frame_key(&self, empty: bool) -> String {
        format!("{}|{:?}|{empty}", self.list.get(), self.hidden.borrow())
    }

    /// The lanes (or list sections) with their heads empty and no cards, or the empty board.
    fn build_frame(self: &Rc<Self>, key: String, empty: bool) {
        // The cards outlive the frame: the next render moves them into the new lanes.
        for (_, card) in self.cards.borrow().values() {
            detach(card);
        }
        clear(&self.content);
        self.layout.borrow_mut().clear();
        self.scrolls.borrow_mut().clear();
        let mut lanes = Vec::new();
        if empty {
            self.render_empty();
            *self.frame.borrow_mut() = Some(Frame { key, lanes });
            return;
        }
        let list = self.list.get();
        let outer = if list {
            let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
            rows.add_css_class("board-list");
            let scroll = crate::app::scrolled(&rows);
            scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
            self.scrolls.borrow_mut().push(("list".into(), scroll.vadjustment()));
            self.content.append(&scroll);
            rows
        } else {
            let grid = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            grid.add_css_class("board-grid");
            grid.set_homogeneous(true);
            let scroller = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Automatic)
                .vscrollbar_policy(gtk::PolicyType::Never)
                .vexpand(true)
                .hexpand(true)
                .child(&grid)
                .build();
            // Lanes past the window's edge show a real scrollbar, not one that hides until hovered.
            scroller.set_overlay_scrolling(false);
            self.scrolls.borrow_mut().push(("board".into(), scroller.hadjustment()));
            self.content.append(&scroller);
            grid
        };
        let hidden = self.hidden.borrow().clone();
        if hidden.len() == COLUMNS.len() {
            let all = label("Every column is hidden. Turn one back on under View.", "board-empty");
            all.set_halign(gtk::Align::Center);
            outer.append(&all);
        }
        for &column in COLUMNS {
            if hidden.contains(column) {
                continue;
            }
            let head = gtk::Box::new(gtk::Orientation::Vertical, 2);
            head.add_css_class(if list { "list-head" } else { "lane-head" });
            // A list section holds its rows itself, its head first; a lane scrolls its cards
            // under its head.
            let (lane, items) = if list {
                let section = gtk::Box::new(gtk::Orientation::Vertical, 0);
                section.add_css_class("list-section");
                (section.clone(), section)
            } else {
                let lane = gtk::Box::new(gtk::Orientation::Vertical, 0);
                lane.add_css_class("board-lane");
                lane.add_css_class(&format!("lane-{column}"));
                lane.set_size_request(208, -1);
                lane.append(&head);
                let cards = gtk::Box::new(gtk::Orientation::Vertical, 6);
                cards.add_css_class("board-cards");
                let scroll = crate::app::scrolled(&cards);
                scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
                scroll.set_vscrollbar_policy(gtk::PolicyType::External);
                self.scrolls.borrow_mut().push((format!("lane:{column}"), scroll.vadjustment()));
                lane.append(&scroll);
                (lane, cards)
            };
            self.lane_drop(&lane, column);
            outer.append(&lane);
            lanes.push(Lane {
                column,
                items,
                head,
                head_key: RefCell::new(String::new()),
                empty: RefCell::new(None),
                headings: RefCell::new(HashMap::new()),
            });
        }
        *self.frame.borrow_mut() = Some(Frame { key, lanes });
    }

    /// A body read at or after the task's own edit, as the search and the peek may use it.
    fn fresh(bodies: &HashMap<i64, (String, String)>, task: &Value) -> Option<String> {
        let (read_at, body) = bodies.get(&task["id"].as_i64()?)?;
        (read_at.as_str() >= text(task, "updated_at")).then(|| body.clone())
    }

    /// Reads the bodies the search lacks, then filters again: a page of the list at a time
    /// for many, `task.get` for a few.
    fn read_bodies(self: &Rc<Self>) {
        if self.bodies_loading.get() {
            return;
        }
        let missing: BTreeSet<i64> = {
            let bodies = self.bodies.borrow();
            self.tasks
                .borrow()
                .iter()
                .filter(|t| Self::fresh(&bodies, t).is_none())
                .filter_map(|t| t["id"].as_i64())
                .collect()
        };
        if missing.is_empty() || missing.is_subset(&self.bodies_missed.borrow()) {
            return;
        }
        let Some(ui) = self.ui() else { return };
        self.bodies_loading.set(true);
        let (board, project) = (self.clone(), self.project);
        let last = self.tasks.borrow().len() as u64 + 100;
        glib::spawn_future_local(async move {
            let mut read: Vec<Value> = Vec::new();
            if missing.len() <= 20 {
                for id in &missing {
                    if let Ok(task) = ui.call("task.get", json!({"task_id":id})).await {
                        read.push(task);
                    }
                }
            } else {
                // Pages small enough to stay under the client's line limit; a page that still
                // is not is read again in halves.
                let (mut offset, mut limit) = (0u64, 50u64);
                loop {
                    match ui.call("task.list", json!({"project_id":project,"limit":limit,"offset":offset})).await {
                        Ok(page) => {
                            read.extend(rows(&page, "tasks"));
                            match page["next_offset"].as_u64() {
                                Some(next) => offset = next,
                                None => break,
                            }
                        }
                        Err(_) if limit > 1 => limit /= 2,
                        // One task too large to list: skip it (its body is read when peeked).
                        Err(_) if offset < last => offset += 1,
                        Err(_) => break,
                    }
                }
            }
            {
                let mut bodies = board.bodies.borrow_mut();
                for task in &read {
                    if let Some(id) = task["id"].as_i64() {
                        bodies.insert(id, (text(task, "updated_at").to_string(), text(task, "body").to_string()));
                    }
                }
                let still: BTreeSet<i64> = {
                    let tasks = board.tasks.borrow();
                    missing.into_iter().filter(|id| !tasks.iter().any(|t| t["id"].as_i64() == Some(*id) && Self::fresh(&bodies, t).is_some())).collect()
                };
                *board.bodies_missed.borrow_mut() = still;
            }
            board.bodies_loading.set(false);
            if !board.query.text().trim().is_empty() {
                board.render_when_closed();
            }
        });
    }

    /// Fetches the module in scope (its name, priority and whether it is done) for the banner.
    fn load_module(self: &Rc<Self>, ui: &Rc<Ui>) {
        let Some(id) = scope() else {
            *self.module.borrow_mut() = None;
            return;
        };
        let (ui, board, project) = (ui.clone(), self.clone(), self.project);
        glib::spawn_future_local(async move {
            let Ok(list) = ui.call("module.list", json!({"project_id":project,"include_archived":true})).await else { return };
            let found = rows(&list, "modules").into_iter().find(|m| m["id"].as_i64() == Some(id));
            if found.is_none() {
                // Deleted elsewhere, or another project's: back to every task, drawn now.
                if scope() == Some(id) {
                    leave_module();
                    *board.module.borrow_mut() = None;
                    board.render_when_closed();
                }
                return;
            }
            if *board.module.borrow() != found {
                *board.module.borrow_mut() = found;
                board.render_when_closed();
            }
        });
    }

    /// A module's head over its board: back to every task, its name and priority, how far
    /// along it is, and adding tasks, marking it done, renaming and deleting it.
    fn render_banner(self: &Rc<Self>, tasks: &[Value]) {
        // Only what the banner shows: the module row also counts its tasks, which every task
        // move changes, and a redraw would close a rename being typed.
        let key = {
            let module = self.module.borrow();
            let shown = module.as_ref().map(|m| json!([m["id"], m["name"], m["priority"], m["completed_at"]]));
            json!([scope(), shown, tasks.is_empty()]).to_string()
        };
        if !self.redraw("banner", key) {
            return;
        }
        clear(&self.banner);
        let (Some(id), Some(module)) = (scope(), self.module.borrow().clone()) else {
            self.banner.set_visible(false);
            return;
        };
        self.banner.set_visible(true);
        let back = button("", "module-back");
        back.set_child(Some(&icon_text(crate::icons::image("arrow-left", 12).upcast_ref(), "All modules")));
        let weak = self.ui.clone();
        back.connect_clicked(move |_| {
            leave_module();
            if let Some(ui) = weak.upgrade() {
                ui.navigate("modules");
            }
        });
        self.banner.append(&back);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.set_valign(gtk::Align::Center);
        head.append(&crate::icons::image("modules", 16));
        let name = label(text(&module, "name"), "module-banner-name");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        head.append(&name);
        let priority = text(&module, "priority");
        let pill = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        pill.add_css_class("field-pill");
        pill.add_css_class(&format!("pill-priority-{priority}"));
        pill.append(&priority_icon(priority));
        pill.append(&label(&titled(priority), "field-pill-name"));
        pill.set_valign(gtk::Align::Center);
        head.append(&pill);
        let done_module = !module["completed_at"].is_null();
        if done_module {
            head.append(&label("Completed", "module-card-done"));
        }
        self.banner.append(&head);
        // The editor: rename and reprioritise in place, behind the pencil.
        let editor = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        editor.add_css_class("module-editor");
        editor.set_valign(gtk::Align::Center);
        editor.set_visible(false);
        let rename = gtk::Entry::builder().text(text(&module, "name")).width_chars(22).build();
        rename.set_valign(gtk::Align::Center);
        editor.append(&rename);
        let chosen = Rc::new(RefCell::new(priority.to_string()));
        for p in URGENT_FIRST {
            let key = button("", "choice-pill");
            key.set_valign(gtk::Align::Center);
            key.set_child(Some(&icon_text(priority_icon(p).upcast_ref(), &titled(p))));
            if p == priority {
                key.add_css_class("selected");
            }
            let chosen = chosen.clone();
            key.connect_clicked(move |key| {
                *chosen.borrow_mut() = p.to_string();
                if let Some(row) = key.parent() {
                    for other in widgets(&row) {
                        other.remove_css_class("selected");
                    }
                }
                key.add_css_class("selected");
            });
            editor.append(&key);
        }
        let save = button("Save", "module-action");
        save.add_css_class("module-save");
        save.set_valign(gtk::Align::Center);
        let weak = Rc::downgrade(self);
        let field = rename.downgrade();
        save.connect_clicked(move |_| {
            let (Some(board), Some(field)) = (weak.upgrade(), field.upgrade()) else { return };
            let name = field.text().trim().to_string();
            if !name.is_empty() {
                board.act("module.update", json!({"module_id":id,"name":name,"priority":*chosen.borrow()}));
                *board.module.borrow_mut() = None;
                if let Some(ui) = board.ui() {
                    board.load_module(&ui);
                }
            }
        });
        editor.append(&save);
        let cancel = button("Cancel", "module-action");
        cancel.set_valign(gtk::Align::Center);
        editor.append(&cancel);
        self.banner.append(&editor);
        let edit = crate::app::icon_button("edit", "Rename or change priority");
        edit.set_valign(gtk::Align::Center);
        let (shown, pane) = (head.downgrade(), editor.downgrade());
        cancel.connect_clicked(move |_| {
            if let (Some(head), Some(editor)) = (shown.upgrade(), pane.upgrade()) {
                editor.set_visible(false);
                head.set_visible(true);
            }
        });
        // Enter saves the name, Escape leaves it as it was.
        let submit = save.downgrade();
        rename.connect_activate(move |_| {
            if let Some(save) = submit.upgrade() {
                save.emit_clicked();
            }
        });
        let escape = gtk::EventControllerKey::new();
        let close = cancel.downgrade();
        escape.connect_key_pressed(move |_, key, _, _| {
            if key != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            if let Some(cancel) = close.upgrade() {
                cancel.emit_clicked();
            }
            glib::Propagation::Stop
        });
        rename.add_controller(escape);
        let (shown, pane, field) = (head.downgrade(), editor.downgrade(), rename.downgrade());
        edit.connect_clicked(move |_| {
            if let (Some(head), Some(editor)) = (shown.upgrade(), pane.upgrade()) {
                let open = !editor.is_visible();
                editor.set_visible(open);
                head.set_visible(!open);
                if let Some(field) = field.upgrade().filter(|_| open) {
                    field.grab_focus();
                }
            }
        });
        self.banner.append(&edit);
        self.banner.append(&spacer());
        // How far along it is shows in the stats band under the filter bar; here, only when
        // the band has nothing to count.
        if tasks.is_empty() {
            self.banner.append(&label("No tasks yet", "module-card-meta"));
        }
        let add = button("", "module-action");
        add.set_child(Some(&icon_text(crate::icons::image("plus", 12).upcast_ref(), "Add tasks")));
        add.set_tooltip_text(Some("Bring existing tasks into this module"));
        let weak = Rc::downgrade(self);
        add.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                board.add_tasks(id);
            }
        });
        self.banner.append(&add);
        let finish = button("", "module-action");
        finish.set_child(Some(&icon_text(
            status_icon(if done_module { "in_review" } else { "done" }, 12).upcast_ref(),
            if done_module { "Reopen module" } else { "Mark module done" },
        )));
        let weak = Rc::downgrade(self);
        finish.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                board.act(if done_module { "module.reopen" } else { "module.complete" }, json!({"module_id":id}));
                *board.module.borrow_mut() = None;
                if let Some(ui) = board.ui() {
                    board.load_module(&ui);
                }
            }
        });
        self.banner.append(&finish);
        let notes = button("", "module-action");
        notes.set_child(Some(&icon_text(crate::icons::image("notes", 12).upcast_ref(), "Patch notes")));
        notes.set_tooltip_text(Some("Draft release notes from the done tasks' changelog sentences"));
        let weak = self.ui.clone();
        let title = text(&module, "name").to_string();
        notes.connect_clicked(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            let panel = crate::panel::Panel::new(&ui, &format!("Patch notes · {title}"), 520);
            let shown = label("Drafting…", "patch-notes");
            shown.set_wrap(true);
            shown.set_selectable(true);
            panel.body.append(&shown);
            let copy = button("Copy Markdown", "quiet");
            copy.set_halign(gtk::Align::End);
            copy.set_sensitive(false);
            panel.body.append(&copy);
            panel.present();
            let (shown, copy) = (shown.downgrade(), copy.downgrade());
            glib::spawn_future_local(async move {
                let draft = ui.call("module.changelog.draft", json!({"module_id":id})).await;
                let (Some(shown), Some(copy)) = (shown.upgrade(), copy.upgrade()) else { return };
                match draft {
                    Ok(draft) if !text(&draft, "markdown").trim().is_empty() => {
                        let markdown = text(&draft, "markdown").to_string();
                        shown.set_markup(&relay_board::markdown_markup(&markdown));
                        copy.set_sensitive(true);
                        copy.connect_clicked(move |key| key.clipboard().set_text(&markdown));
                    }
                    Ok(_) => shown.set_text("Nothing to note yet: done tasks with a changelog sentence appear here."),
                    Err(e) => shown.set_text(&e.to_string()),
                }
            });
        });
        self.banner.append(&notes);
        let delete = crate::app::icon_button("trash", "Delete this module; its tasks stay on the board");
        let weak = Rc::downgrade(self);
        crate::app::confirm_inline(&delete, "Delete module?", move |_| {
            if let Some(board) = weak.upgrade() {
                leave_module();
                board.act("module.delete", json!({"module_id":id}));
                if let Some(ui) = board.ui() {
                    ui.navigate("modules");
                }
            }
        });
        self.banner.append(&delete);
    }

    /// A side panel listing the tasks outside the module, to tick and bring in.
    fn add_tasks(self: &Rc<Self>, module: i64) {
        let Some(ui) = self.ui() else { return };
        let name = self.module.borrow().as_ref().map(|m| text(m, "name").to_string()).unwrap_or_default();
        let panel = crate::panel::Panel::new(&ui, &format!("Add tasks to {name}"), 440);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        list.add_css_class("module-pick-list");
        let picked: Rc<RefCell<BTreeSet<i64>>> = Rc::default();
        let tasks = self.tasks.borrow().clone();
        let outside: Vec<&Value> = tasks.iter().filter(|t| t["module_id"].as_i64() != Some(module) && text(t, "column") != "done").collect();
        if outside.is_empty() {
            list.append(&label("Every open task is already in this module.", "issue-side-empty"));
        }
        // Add waits for a ticked task.
        let add = button("Add to module", "primary");
        add.set_sensitive(false);
        for task in outside {
            let id = task["id"].as_i64().unwrap_or(0);
            let row = gtk::CheckButton::new();
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            content.append(&status_icon(text(task, "column"), 12));
            let title = label(text(task, "title"), "");
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.set_hexpand(true);
            content.append(&title);
            if let Some(other) = task["module_name"].as_str() {
                content.append(&label(other, "module-card-meta"));
            }
            row.set_child(Some(&content));
            let (picked, key) = (picked.clone(), add.downgrade());
            row.connect_toggled(move |row| {
                if row.is_active() {
                    picked.borrow_mut().insert(id);
                } else {
                    picked.borrow_mut().remove(&id);
                }
                if let Some(key) = key.upgrade() {
                    key.set_sensitive(!picked.borrow().is_empty());
                }
            });
            list.append(&row);
        }
        panel.body.append(&list);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        actions.append(&add);
        panel.body.append(&actions);
        let (weak, p) = (Rc::downgrade(self), Rc::downgrade(&panel));
        add.connect_clicked(move |key| {
            let (Some(board), Some(ui)) = (weak.upgrade(), weak.upgrade().and_then(|b| b.ui())) else { return };
            let chosen: Vec<i64> = picked.borrow().iter().copied().collect();
            if chosen.is_empty() {
                return;
            }
            key.set_sensitive(false);
            let p = p.clone();
            glib::spawn_future_local(async move {
                for id in chosen {
                    if let Err(e) = ui.call("task.update", json!({"task_id":id,"module_id":module})).await {
                        ui.show_error(&e.to_string());
                    }
                }
                if let Some(p) = p.upgrade() {
                    p.close();
                }
                ui.refresh_page();
                drop(board);
            });
        });
        panel.present();
    }

    /// The band over the lanes: how far the board (or the module) is along, split by column,
    /// then what is open, in flight, waiting on review, blocked and recently shipped. It counts
    /// every task in scope, not only what the search shows: it describes the work, not the view.
    /// One line of what the board holds, every figure honest: how many tasks, a bar and a share
    /// of what is done (only done counts as progress), then what is in review, active and with
    /// how many agents, blocked, shipped this week and the points left.
    fn render_stats(self: &Rc<Self>, tasks: &[Value], everything: &[Value]) {
        let in_column = |column: &str| tasks.iter().filter(|t| text(t, "column") == column).count();
        let done = in_column("done");
        let open: Vec<&Value> = tasks.iter().filter(|t| text(t, "column") != "done").collect();
        let left = relay_board::estimate(open.iter().copied());
        let urgent = open.iter().filter(|t| matches!(text(t, "priority"), "high" | "urgent")).count();
        let active = in_column("active");
        // The agents on open tasks, each once: what the cards show, not just a running state.
        let agents: BTreeSet<&str> = open.iter().flat_map(|t| t["sessions"].as_array().into_iter().flatten().filter_map(Value::as_str)).collect();
        let review = in_column("in_review");
        let all = by_id(everything);
        let blocked = open.iter().filter(|t| relay_board::is_blocked(t, &all)).count();
        let shipped = tasks
            .iter()
            .filter(|t| text(t, "column") == "done" && within_days(text(t, "updated_at"), 7))
            .count();
        let key = format!("{} {done} {left} {urgent} {active} {} {review} {blocked} {shipped} {:?}", tasks.len(), agents.len(), scope().is_some());
        if !self.redraw("stats", key) {
            return;
        }
        clear(&self.stats);
        self.stats.set_visible(!tasks.is_empty());
        if tasks.is_empty() {
            return;
        }
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.add_css_class("stats-line");
        let plural = |n: usize, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
        line.append(&label(&plural(tasks.len(), "task", "tasks"), "stats-figure"));
        let fraction = done as f64 / tasks.len() as f64;
        let bar = gtk::ProgressBar::new();
        bar.add_css_class("stats-bar");
        bar.set_fraction(fraction);
        bar.set_valign(gtk::Align::Center);
        bar.set_tooltip_text(Some(&format!("{done} of {} done", tasks.len())));
        line.append(&bar);
        line.append(&label(&format!("{done} done · {:.0}%", fraction * 100.), "stats-figure"));
        let mut part = |words: String, class: &str, tip: &str| {
            line.append(&label("·", "stats-dot"));
            let item = label(&words, "stats-part");
            if !class.is_empty() {
                item.add_css_class(class);
            }
            if !tip.is_empty() {
                item.set_tooltip_text(Some(tip));
            }
            line.append(&item);
        };
        part(format!("{review} in review"), if review > 0 { "stats-review" } else { "" }, "Waiting on you");
        part(
            if agents.is_empty() { format!("{active} active") } else { format!("{active} active, {} on it", plural(agents.len(), "agent", "agents")) },
            "",
            &agents.iter().copied().collect::<Vec<_>>().join(", "),
        );
        part(format!("{blocked} blocked"), if blocked > 0 { "stats-alert" } else { "" }, "Open tasks waiting on another open task");
        if urgent > 0 {
            part(format!("{urgent} high or urgent"), "stats-warm", "");
        }
        part(format!("{shipped} shipped this week"), "", "");
        if left > 0 {
            part(format!("{left} pt left"), "", "Estimate of open work (S = 1, M = 3, L = 5)");
        }
        self.stats.append(&line);
    }

    fn render_empty(self: &Rc<Self>) {
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.add_css_class("board-blank");
        empty.set_valign(gtk::Align::Center);
        empty.set_halign(gtk::Align::Center);
        empty.set_vexpand(true);
        let icon = status_icon("backlog", 28);
        icon.set_halign(gtk::Align::Center);
        empty.append(&icon);
        let title = label("No tasks yet", "board-blank-title");
        title.set_halign(gtk::Align::Center);
        empty.append(&title);
        let hint = label(
            "Tasks you add here are what agents are dispatched to. Press C to add one.",
            "board-blank-hint",
        );
        hint.set_halign(gtk::Align::Center);
        empty.append(&hint);
        let add = button("New task", "primary");
        add.set_halign(gtk::Align::Center);
        let weak = Rc::downgrade(self);
        add.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                board.open_quick("backlog");
            }
        });
        empty.append(&add);
        self.content.append(&empty);
    }

    fn render_strip(self: &Rc<Self>, tasks: &[Value]) {
        let filters = self.filters.borrow().clone();
        let group = self.group.borrow().clone();
        let hidden = self.hidden.borrow().clone();
        // The chips name modules and parents by the tasks that carry them.
        let named: Vec<String> = filters
            .iter()
            .filter(|(key, _)| matches!(key.as_str(), "module" | "parent"))
            .flat_map(|(_, values)| values.iter())
            .filter_map(|value| value.parse::<i64>().ok())
            .flat_map(|id| {
                tasks
                    .iter()
                    .filter(move |t| t["module_id"].as_i64() == Some(id) || t["id"].as_i64() == Some(id))
                    .take(2)
                    .map(|t| format!("{}{}", text(t, "module_name"), text(t, "title")))
            })
            .collect();
        if !self.redraw("strip", format!("{filters:?} {group} {hidden:?} {named:?}")) {
            return;
        }
        clear(&self.strip);
        if let Some(row) = self.strip.ancestor(gtk::ScrolledWindow::static_type()) {
            row.set_visible(filters.values().any(|v| !v.is_empty()) || !group.is_empty() || !hidden.is_empty());
        }
        for column in COLUMNS.iter().filter(|c| hidden.contains(**c)) {
            self.strip.append(&self.removable(&format!("hidden: {}", caption(column)), move |board| {
                board.hidden.borrow_mut().remove(*column);
            }));
        }
        for (key, values) in &filters {
            for value in values {
                let name = match key.as_str() {
                    "module" => tasks
                        .iter()
                        .find(|t| t["module_id"].as_i64() == value.parse().ok())
                        .and_then(|t| t["module_name"].as_str())
                        .unwrap_or(value)
                        .to_string(),
                    "parent" if value == "roots" => "top level".into(),
                    "parent" => tasks
                        .iter()
                        .find(|t| t["id"].as_i64() == value.parse().ok())
                        .map(|t| format!("#{value} {}", text(t, "title")))
                        .unwrap_or_else(|| format!("#{value}")),
                    _ => value.clone(),
                };
                let key_name = if key == "session" { "agent" } else { key.as_str() };
                self.strip.append(&self.removable(&format!("{key_name}: {name}"), {
                    let (key, value) = (key.clone(), value.clone());
                    move |board| {
                        if let Some(set) = board.filters.borrow_mut().get_mut(&key) {
                            set.remove(&value);
                        }
                    }
                }));
            }
        }
        if !group.is_empty() {
            let name = GROUPS.iter().find(|(g, _)| *g == group).map(|(_, n)| *n).unwrap_or("");
            self.strip.append(&self.removable(&format!("grouped by {}", name.to_lowercase()), |board| {
                board.group.borrow_mut().clear();
            }));
        }
    }
    fn removable(self: &Rc<Self>, caption: &str, remove: impl Fn(&Rc<Board>) + 'static) -> gtk::Button {
        let chip = button("", "filter-chip");
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        let name = label(caption, "");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(28);
        content.append(&name);
        content.append(&crate::icons::image("close", 10));
        chip.set_child(Some(&content));
        chip.set_tooltip_text(Some("Remove"));
        let weak = Rc::downgrade(self);
        chip.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                remove(&board);
                glib::idle_add_local_once(move || board.render());
            }
        });
        chip
    }

    /// The View menu: which columns show, how cards group and what filters them, each a row of
    /// small rounded pills that toggle in place.
    fn filter_menu(self: &Rc<Self>, popover: &gtk::Popover) {
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 4);
        menu.add_css_class("board-view-menu");
        menu.set_size_request(300, -1);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.append(&label("View", "view-menu-title"));
        head.append(&spacer());
        let reset = button("Reset", "view-reset");
        let weak = Rc::downgrade(self);
        let pop = popover.downgrade();
        reset.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                board.filters.borrow_mut().clear();
                board.group.borrow_mut().clear();
                board.hidden.borrow_mut().clear();
                board.render();
                if let Some(pop) = pop.upgrade() {
                    board.filter_menu(&pop);
                }
            }
        });
        head.append(&reset);
        menu.append(&head);
        // One toggle pill: its state read by `active`, its click handled by `toggled`.
        let pill = |caption: &str, mark: Option<gtk::Widget>, active: bool, toggled: Toggle| {
            let key = gtk::ToggleButton::new();
            key.add_css_class("view-pill");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 5);
            let wide = caption.chars().count().min(18) + if mark.is_some() { 5 } else { 3 };
            if let Some(mark) = mark {
                content.append(&mark);
            }
            let name = label(caption, "");
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            name.set_max_width_chars(18);
            content.append(&name);
            key.set_child(Some(&content));
            key.set_active(active);
            let weak = Rc::downgrade(self);
            key.connect_toggled(move |key| {
                if let Some(board) = weak.upgrade() {
                    toggled(&board, key.is_active());
                    board.render();
                }
            });
            (key.upcast::<gtk::Widget>(), wide)
        };
        let section = |title: &str, items: Vec<(gtk::Widget, usize)>| {
            if items.is_empty() {
                return;
            }
            let caption = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            caption.append(&label(title, "view-menu-caption"));
            // G steps through the groupings from the board, without opening this menu.
            if title == "Group by" {
                caption.append(&spacer());
                let cap = label("G", "board-kbd");
                cap.set_valign(gtk::Align::End);
                cap.set_margin_bottom(2);
                caption.append(&cap);
            }
            menu.append(&caption);
            menu.append(&wrapped(items, 38));
        };
        let hidden = self.hidden.borrow().clone();
        section("Columns", COLUMN_TITLES.iter().map(|(column, title)| {
            let column = *column;
            pill(title, Some(status_icon(column, 11).upcast()), !hidden.contains(column), Box::new(move |board, on| {
                if on {
                    board.hidden.borrow_mut().remove(column);
                } else {
                    board.hidden.borrow_mut().insert(column.to_string());
                }
            }))
        }).collect());
        let group = self.group.borrow().clone();
        let groups: Rc<RefCell<Vec<glib::WeakRef<gtk::ToggleButton>>>> = Rc::default();
        let mut group_pills = Vec::new();
        for (key, title) in GROUPS {
            let (item, wide) = pill(title, None, group == key, Box::new(move |board, on| {
                if on {
                    *board.group.borrow_mut() = key.to_string();
                } else if *board.group.borrow() == key {
                    board.group.borrow_mut().clear();
                }
            }));
            if let Some(toggle) = item.downcast_ref::<gtk::ToggleButton>() {
                groups.borrow_mut().push(toggle.downgrade());
                // One grouping at a time: lighting one puts the others out.
                let others = groups.clone();
                toggle.connect_toggled(move |toggle| {
                    if toggle.is_active() {
                        for other in others.borrow().iter().filter_map(|w| w.upgrade()) {
                            if other != *toggle && other.is_active() {
                                other.set_active(false);
                            }
                        }
                    }
                });
            }
            group_pills.push((item, wide));
        }
        section("Group by", group_pills);
        let tasks = self.tasks.borrow().clone();
        let mut modules = BTreeMap::new();
        let mut labels = BTreeSet::new();
        let mut agents = BTreeSet::new();
        for task in &tasks {
            if let Some(id) = task["module_id"].as_i64() {
                modules.insert(id.to_string(), task["module_name"].as_str().map(str::to_string).unwrap_or_else(|| format!("Module #{id}")));
            }
            labels.extend(rows(task, "labels").iter().filter_map(|v| v.as_str().map(str::to_string)));
            agents.extend(rows(task, "sessions").iter().filter_map(|v| v.as_str().map(str::to_string)));
        }
        let filters = self.filters.borrow().clone();
        let on = |key: &str, value: &str| filters.get(key).is_some_and(|set| set.contains(value));
        let toggle = |key: &'static str, value: String| -> Toggle {
            Box::new(move |board, active| {
                let mut filters = board.filters.borrow_mut();
                let set = filters.entry(key.to_string()).or_default();
                if active {
                    set.insert(value.clone());
                } else {
                    set.remove(&value);
                }
                if set.is_empty() {
                    filters.remove(key);
                }
            })
        };
        section("Priority", URGENT_FIRST.iter().map(|p| pill(&titled(p), Some(priority_icon(p).upcast()), on("priority", p), toggle("priority", p.to_string()))).collect());
        section("Size", SIZES[1..].iter().map(|s| pill(&format!("{s} · {} pt", relay_board::points(s)), None, on("size", s), toggle("size", s.to_string()))).collect());
        section("Type", TYPES.iter().map(|t| pill(&titled(t), None, on("type", t), toggle("type", t.to_string()))).collect());
        if scope().is_none() {
            section("Module", modules.into_iter().map(|(id, name)| pill(&name, None, on("module", &id), toggle("module", id))).collect());
        }
        section("Label", labels.into_iter().map(|l| pill(&l, None, on("label", &l), toggle("label", l.clone()))).collect());
        section("Agent", agents.into_iter().map(|a| pill(&a, None, on("session", &a), toggle("session", a.clone()))).collect());
        let scroll = crate::app::scrolled(&menu);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(520);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_vscrollbar_policy(gtk::PolicyType::External);
        popover.set_child(Some(&scroll));
    }

    fn sorted<'a>(&self, visible: &[&'a Value], column: &str) -> Vec<&'a Value> {
        relay_board::sorted(visible, column, &self.group.borrow())
    }

    /// GitHub's column head: status ring, name, count and the lane's estimate, then a menu and
    /// an add key; a lane (not a list section) adds the column's one-line meaning under it.
    /// Drawn into `outer`, the lane's head box, replacing what it showed: the box keeps its
    /// place in the lane.
    fn lane_head(self: &Rc<Self>, outer: &gtk::Box, column: &'static str, shown: &[&Value], total: usize) {
        empty_out(outer);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        head.add_css_class("lane-head-row");
        head.append(&status_icon(column, 14));
        let title = label(caption(column), "lane-title");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        head.append(&title);
        let count = shown.len();
        let count = label(&if count == total { total.to_string() } else { format!("{count} / {total}") }, "lane-count");
        count.set_valign(gtk::Align::Center);
        head.append(&count);
        // The estimate rides on the count: on the head row it cut the lane's name ("Back…"), and
        // "0 pt" on every lane said nothing.
        let points = relay_board::estimate(shown.iter().copied());
        if points > 0 {
            count.set_tooltip_text(Some(&format!("{points} points of work shown here (S = 1, M = 3, L = 5)")));
        }
        head.append(&spacer());
        let more = gtk::MenuButton::new();
        more.set_child(Some(&crate::icons::image("more", 14)));
        more.add_css_class("lane-key");
        more.set_tooltip_text(Some(&format!("{} options", caption(column))));
        let pop = gtk::Popover::new();
        pop.add_css_class("board-menu");
        pop.set_has_arrow(false);
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 1);
        let entry = |icon: &str, caption: &str| {
            let key = button("", "board-menu-item");
            key.set_child(Some(&icon_text(crate::icons::image(icon, 12).upcast_ref(), caption)));
            key
        };
        if column != "done" {
            let new = entry("plus", "New task here");
            let weak = Rc::downgrade(self);
            let close = pop.downgrade();
            new.connect_clicked(move |_| {
                if let Some(pop) = close.upgrade() {
                    pop.popdown();
                }
                if let Some(board) = weak.upgrade() {
                    board.compose(column);
                }
            });
            menu.append(&new);
            let quick = entry("edit", "Quick add in place");
            let weak = Rc::downgrade(self);
            let close = pop.downgrade();
            quick.connect_clicked(move |_| {
                if let Some(pop) = close.upgrade() {
                    pop.popdown();
                }
                if let Some(board) = weak.upgrade() {
                    board.open_quick(column);
                }
            });
            menu.append(&quick);
        }
        let hide = entry("minimize", "Hide column");
        let weak = Rc::downgrade(self);
        let close = pop.downgrade();
        hide.connect_clicked(move |_| {
            if let Some(pop) = close.upgrade() {
                pop.popdown();
            }
            if let Some(board) = weak.upgrade() {
                board.hidden.borrow_mut().insert(column.to_string());
                glib::idle_add_local_once(move || board.render());
            }
        });
        menu.append(&hide);
        pop.set_child(Some(&menu));
        more.set_popover(Some(&pop));
        head.append(&more);
        if column != "done" {
            let add = crate::app::icon_button("plus", &format!("New task in {}", caption(column)));
            add.add_css_class("lane-key");
            let weak = Rc::downgrade(self);
            add.connect_clicked(move |_| {
                if let Some(board) = weak.upgrade() {
                    board.compose(column);
                }
            });
            head.append(&add);
        }
        // What the lane means is its title's tooltip; a subtitle restating the name took a line.
        head.set_tooltip_text(Some(lane_meaning(column)));
        outer.append(&head);
    }

    fn lane_drop(self: &Rc<Self>, widget: &impl IsA<gtk::Widget>, column: &'static str) {
        let drop = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
        let weak = Rc::downgrade(self);
        drop.connect_enter(|target, _, _| {
            if let Some(w) = target.widget() {
                w.add_css_class("drop-lane");
            }
            gdk::DragAction::MOVE
        });
        drop.connect_leave(|target| {
            if let Some(w) = target.widget() {
                w.remove_css_class("drop-lane");
            }
        });
        drop.connect_drop(move |target, token, _, _| {
            if let Some(w) = target.widget() {
                w.remove_css_class("drop-lane");
            }
            let (Some(board), Some(id)) = (weak.upgrade(), drop_id(token)) else {
                return false;
            };
            let Some(task) = board.task(id) else { return false };
            board.focused.set(Some(id));
            if text(&task, "column") == column {
                // Below the last card of its own lane: to the bottom, unless it is already there.
                let order = board.column_order(column);
                if column == "done" || order.last() == Some(&id) {
                    return false;
                }
                board.move_to(&task, column, Some(order.len() - 1));
                return true;
            }
            board.move_to(&task, column, None);
            true
        });
        widget.add_controller(drop);
    }

    /// The columns view, or the list view when `self.list` is set: into each lane (or list
    /// section) of the frame, its head, empty note, group headings and cards (or rows), in
    /// order. A card whose [`card_key`] is unchanged is the same widget, moved if it must be; a
    /// filtered-out card leaves its lane but is kept for when it shows again.
    fn render_lanes(self: &Rc<Self>, tasks: &[Value], everything: &[Value], visible: &[&Value]) {
        let list = self.list.get();
        let grouping = self.group.borrow().clone();
        let all = by_id(everything);
        let project = self.project_name();
        let scoped = scope().is_some();
        let frame = self.frame.borrow();
        let Some(frame) = frame.as_ref() else { return };
        let mut cards = self.cards.borrow_mut();
        let mut layout = Vec::new();
        for lane in &frame.lanes {
            let column = lane.column;
            let total = tasks.iter().filter(|t| text(t, "column") == column).count();
            let shown = self.sorted(visible, column);
            let points = relay_board::estimate(shown.iter().copied());
            let key = format!("{} {total} {points}", shown.len());
            if *lane.head_key.borrow() != key {
                self.lane_head(&lane.head, column, &shown, total);
                *lane.head_key.borrow_mut() = key;
            }
            let mut wanted: Vec<gtk::Widget> = Vec::new();
            if list {
                wanted.push(lane.head.clone().upcast());
            }
            if shown.is_empty() {
                let filtered = total > 0;
                let mut slot = lane.empty.borrow_mut();
                if slot.as_ref().is_none_or(|(was, _)| *was != filtered) {
                    let note: gtk::Widget = if list {
                        label(if filtered { "No matches" } else { "All quiet" }, "list-empty").upcast()
                    } else {
                        self.empty_lane(column, filtered).upcast()
                    };
                    *slot = Some((filtered, note));
                }
                wanted.extend(slot.as_ref().map(|(_, note)| note.clone()));
            }
            let heading = if list { "list-group-heading" } else { "board-group-heading" };
            let mut placed = Vec::new();
            let mut previous: Option<String> = None;
            for task in shown {
                if !grouping.is_empty() {
                    let group = group_of(task, &grouping);
                    if previous.as_ref() != Some(&group) {
                        // A priority or type heading is a field value, titled as its pill is; a
                        // module's or an agent's name reads as it was written.
                        let words = match grouping.as_str() {
                            "priority" | "type" => titled(&group_title(&group)),
                            _ => group_title(&group),
                        };
                        let title = lane
                            .headings
                            .borrow_mut()
                            .entry(format!("{grouping} {group}"))
                            .or_insert_with(|| label(&words, heading).upcast())
                            .clone();
                        wanted.push(title);
                        previous = Some(group);
                    }
                }
                let id = task["id"].as_i64().unwrap_or(0);
                let key = card_key(task, &all, &project, list, scoped);
                let kind = if list { "list-row" } else { "task-card" };
                // A card that turns done (or a sub-task) is a new one, as it always was: its
                // colours change at once rather than through the card's hover transition.
                let looks = |item: &gtk::Box| {
                    item.has_css_class(kind)
                        && item.has_css_class("done") == (text(task, "column") == "done")
                        && (list || item.has_css_class("task-child") == !task["parent_id"].is_null())
                };
                let item = match cards.get_mut(&id) {
                    Some((drawn, item)) if *drawn == key => item.clone(),
                    // Redrawn inside the same card: it keeps its place among the others.
                    Some((drawn, item)) if looks(item) => {
                        if list {
                            self.fill_row(item, task, &all);
                        } else {
                            self.fill_card(item, task, &all, &project);
                        }
                        *drawn = key;
                        item.clone()
                    }
                    _ => {
                        let item = if list { self.row(id) } else { self.card(id) };
                        if list {
                            self.fill_row(&item, task, &all);
                        } else {
                            self.fill_card(&item, task, &all, &project);
                        }
                        if let Some((_, old)) = cards.insert(id, (key, item.clone())) {
                            detach(&old);
                        }
                        item
                    }
                };
                wanted.push(item.clone().upcast());
                placed.push((id, item.upcast()));
            }
            place(&lane.items, &wanted);
            layout.push((column.to_string(), placed));
        }
        // A task gone from the board takes its card with it.
        cards.retain(|id, (_, card)| {
            let kept = all.contains_key(id);
            if !kept {
                detach(card);
            }
            kept
        });
        *self.layout.borrow_mut() = layout;
    }

    fn empty_lane(&self, column: &str, filtered: bool) -> gtk::Box {
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 4);
        empty.add_css_class("board-empty");
        let (title, hint) = if filtered {
            ("No matches", "Nothing here fits the search or filters.")
        } else {
            match column {
                "backlog" => ("All quiet", "Press C to capture a task."),
                "ready" => ("All quiet", "Drop a card here when it is ready to dispatch."),
                "active" => ("No work in flight", "Dispatched tasks land here."),
                "in_review" => ("Nothing to review", "Agents move finished work here."),
                _ => ("Nothing approved yet", "Drop a card here to approve it."),
            }
        };
        let t = label(title, "board-empty-title");
        t.set_halign(gtk::Align::Center);
        let h = label(hint, "board-empty-hint");
        h.set_halign(gtk::Align::Center);
        h.set_wrap(true);
        h.set_justify(gtk::Justification::Center);
        empty.append(&t);
        empty.append(&h);
        empty
    }

    /// The project's name, as a card's `Project #id` line reads.
    fn project_name(&self) -> String {
        self.ui()
            .and_then(|ui| {
                ui.projects.borrow().iter().find(|p| p["id"].as_i64() == Some(self.project)).map(|p| text(p, "name").to_string())
            })
            .unwrap_or_default()
    }

    /// A card for task `id`, empty: what it answers (focus, clicks, its menu, drag and drop,
    /// files dropped on it). [`Self::fill_card`] draws the task into it.
    fn card(self: &Rc<Self>, id: i64) -> gtk::Box {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
        card.add_css_class("task-card");
        card.set_widget_name("task-card");
        card.set_focusable(true);
        self.wire(&card, id);
        if let Some(ui) = self.ui() {
            task_pages::attach_on_drop(&ui, &card, id);
        }
        card
    }

    /// A dense card, as lific draws one: the task's key (`REL-12`) and any state badge, the
    /// title, then one wrapped row of chips — priority, size, type, module, labels, sub-task
    /// progress, relations, attachments and commits, and the agents on it. Drawn into the card
    /// from [`Self::card`], replacing what it showed: the card itself (its place, focus and
    /// handlers) stays.
    fn fill_card(self: &Rc<Self>, card: &gtk::Box, task: &Value, all: &BTreeMap<i64, &Value>, project: &str) {
        empty_out(card);
        let id = task["id"].as_i64().unwrap_or(0);
        let key = relay_board::task_key(project, id);
        let column = text(task, "column");
        let done = column == "done";
        if done {
            card.add_css_class("done");
        } else {
            card.remove_css_class("done");
        }
        if !task["parent_id"].is_null() {
            card.add_css_class("task-child");
        } else {
            card.remove_css_class("task-child");
        }
        card.update_property(&[gtk::accessible::Property::Label(&format!(
            "{key} {} · {} · {}",
            text(task, "title"),
            text(task, "type"),
            text(task, "priority")
        ))]);

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        top.add_css_class("card-top");
        let number = label(&key, "card-id");
        number.set_tooltip_text(Some(&format!("{project} #{id}")));
        top.append(&number);
        if let Some(badge) = state_badge(text(task, "state")) {
            top.append(&badge);
        }
        top.append(&spacer());
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tools.add_css_class("card-tools");
        let copy = crate::app::icon_button("copy", "Copy title, body and id");
        let weak = self.ui.clone();
        copy.connect_clicked(move |key| copy_task(&weak, id, key.clipboard()));
        let peek = crate::app::icon_button("sidebar", "Peek (Space)");
        let weak = Rc::downgrade(self);
        peek.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                board.toggle_peek(id);
            }
        });
        let open = crate::app::icon_button("external", "Open (Enter)");
        open.set_widget_name("board-open");
        let weak = self.ui.clone();
        open.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                task_pages::open(&ui, id);
            }
        });
        for key in [&copy, &peek, &open] {
            key.add_css_class("card-key");
            tools.append(key);
        }
        top.append(&tools);
        card.append(&top);

        let title = label(text(task, "title"), "card-title");
        title.set_wrap(true);
        title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        title.set_lines(2);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_xalign(0.);
        title.set_tooltip_text(Some(text(task, "title")));
        card.append(&title);

        if let Some(parent) = task["_parent_title"].as_str() {
            let lineage = gtk::Box::new(gtk::Orientation::Horizontal, 3);
            lineage.add_css_class("card-lineage");
            lineage.append(&crate::icons::image("chevron-right", 10));
            let caption = label(parent, "task-lineage");
            caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
            lineage.append(&caption);
            card.append(&lineage);
        }

        // One row of chips, wrapped to the card: what a glance at the board should tell.
        let mut chips: Vec<(gtk::Widget, usize)> = Vec::new();
        let chip = |icon: Option<gtk::Widget>, caption: &str, class: &str, tip: &str| {
            let chip = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            chip.add_css_class("field-pill");
            if !class.is_empty() {
                chip.add_css_class(class);
            }
            if !tip.is_empty() {
                chip.set_tooltip_text(Some(tip));
            }
            let wide = caption.chars().count().min(18) + if icon.is_some() { 5 } else { 3 };
            if let Some(icon) = icon {
                chip.append(&icon);
            }
            if !caption.is_empty() {
                let name = label(caption, "field-pill-name");
                name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                name.set_max_width_chars(18);
                chip.append(&name);
            }
            (chip.upcast::<gtk::Widget>(), wide)
        };
        let priority = text(task, "priority");
        let urgency = priority_icon(priority);
        urgency.set_tooltip_text(Some(&format!("{} priority", titled(priority))));
        urgency.add_css_class(&format!("pill-priority-{priority}"));
        chips.push((urgency.upcast(), 2));
        let size = text(task, "size");
        if !size.is_empty() {
            chips.push(chip(None, size, "pill-size", &format!("Size {size} · {} pt", relay_board::points(size))));
        }
        let kind = text(task, "type");
        if kind != "task" && !kind.is_empty() {
            chips.push(chip(Some(task_mark(kind, 9).upcast()), &titled(kind), "", ""));
        }
        // On a module's own board every card is in it: the chip would only repeat the banner.
        if let Some(module) = task["module_name"].as_str().filter(|_| scope().is_none()) {
            chips.push(chip(Some(crate::icons::image("modules", 10).upcast()), module, "pill-module", "Module"));
        }
        let tags: Vec<String> = rows(task, "labels").iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        for tag in tags.iter().take(2) {
            chips.push((label_chip(tag).upcast(), tag.chars().count().min(14) + 4));
        }
        if tags.len() > 2 {
            let more = label(&format!("+{}", tags.len() - 2), "label-more");
            more.set_tooltip_text(Some(&tags[2..].join(", ")));
            chips.push((more.upcast(), 3));
        }
        let total = task["rollup"]["total"].as_i64().unwrap_or(0);
        if total > 0 {
            let finished = task["rollup"]["done"].as_i64().unwrap_or(0);
            chips.push(chip(Some(crate::icons::image("check", 10).upcast()), &format!("{finished}/{total}"), "pill-rollup", &format!("{finished} of {total} sub-tasks done")));
        }
        let blockers = open_blockers(task, all);
        if !blockers.is_empty() && !done {
            let caption = if blockers.len() == 1 {
                format!("blocked by {}", relay_board::task_key(project, blockers[0]["id"].as_i64().unwrap_or(0)))
            } else {
                format!("blocked by {}", blockers.len())
            };
            let tip = blockers.iter().map(|t| format!("{} {}", relay_board::task_key(project, t["id"].as_i64().unwrap_or(0)), text(t, "title"))).collect::<Vec<_>>().join("\n");
            chips.push(chip(Some(lamp("blocked-lamp").upcast()), &caption, "pill-blocked", &tip));
        }
        if let Some(duplicate) = task["duplicate_of"].as_i64().and_then(|other| all.get(&other)) {
            let other = relay_board::task_key(project, duplicate["id"].as_i64().unwrap_or(0));
            chips.push(chip(None, &format!("duplicate of {other}"), "", text(duplicate, "title")));
        }
        for (field, icon, one, many) in [("attachments", "file", "attachment", "attachments"), ("commits", "commit", "commit", "commits")] {
            let n = rows(task, field).len();
            if n > 0 {
                chips.push(chip(Some(crate::icons::image(icon, 10).upcast()), &n.to_string(), "pill-count", &format!("{n} {}", if n == 1 { one } else { many })));
            }
        }
        // The agents on it, as initials: the lamp says whether one is running now.
        let agents: Vec<String> = rows(task, "sessions").iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        if !agents.is_empty() {
            let people = gtk::Box::new(gtk::Orientation::Horizontal, 3);
            people.add_css_class("card-agents");
            people.set_tooltip_text(Some(&agents.join(", ")));
            if text(task, "state") == "running" {
                people.append(&lamp("live-lamp"));
            }
            for agent in agents.iter().take(3) {
                let initials: String = agent.split(['-', '_', ' ']).filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase();
                let avatar = label(&initials, "card-avatar");
                avatar.set_xalign(0.5);
                people.append(&avatar);
            }
            if agents.len() > 3 {
                people.append(&label(&format!("+{}", agents.len() - 3), "label-more"));
            }
            chips.push((people.upcast(), 4 + 3 * agents.len().min(3)));
        }
        let fields = wrapped(chips, 30);
        fields.add_css_class("card-fields");
        card.append(&fields);
    }

    fn row(self: &Rc<Self>, id: i64) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
        row.add_css_class("list-row");
        row.set_widget_name("task-card");
        row.set_focusable(true);
        self.wire(&row, id);
        row
    }

    /// Draws `task` into its list row from [`Self::row`], as [`Self::fill_card`] does a card.
    fn fill_row(self: &Rc<Self>, row: &gtk::Box, task: &Value, all: &BTreeMap<i64, &Value>) {
        empty_out(row);
        let id = task["id"].as_i64().unwrap_or(0);
        if text(task, "column") == "done" {
            row.add_css_class("done");
        } else {
            row.remove_css_class("done");
        }
        row.append(&priority_icon(text(task, "priority")));
        let number = label(&relay_board::task_key(&self.project_name(), id), "card-id");
        number.set_width_chars(7);
        row.append(&number);
        let mark = task_mark(text(task, "type"), 9);
        mark.set_tooltip_text(Some(text(task, "type")));
        row.append(&mark);
        let title = label(text(task, "title"), "list-title");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_hexpand(true);
        title.set_tooltip_text(Some(text(task, "title")));
        if task["parent_id"].is_null() {
            row.append(&title);
        } else {
            let both = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            both.set_hexpand(true);
            title.set_hexpand(false);
            both.append(&title);
            if let Some(parent) = task["_parent_title"].as_str() {
                let lineage = label(&format!("in {parent}"), "list-lineage");
                lineage.set_ellipsize(gtk::pango::EllipsizeMode::End);
                lineage.set_hexpand(true);
                both.append(&lineage);
            }
            row.append(&both);
        }
        let total = task["rollup"]["total"].as_i64().unwrap_or(0);
        if total > 0 {
            row.append(&label(
                &format!("{}/{total}", task["rollup"]["done"].as_i64().unwrap_or(0)),
                "list-meta",
            ));
        }
        if !open_blockers(task, all).is_empty() && text(task, "column") != "done" {
            let lamp = lamp("blocked-lamp");
            lamp.set_tooltip_text(Some("Blocked by an open task"));
            row.append(&lamp);
        }
        if let Some(tags) = labels_row(task, 2) {
            row.append(&tags);
        }
        if let Some(badge) = state_badge(text(task, "state")) {
            row.append(&badge);
        }
        if !text(task, "size").is_empty() {
            let size = label(text(task, "size"), "task-chip");
            size.set_tooltip_text(Some(&format!("Size {} · {} pt", text(task, "size"), relay_board::points(text(task, "size")))));
            row.append(&size);
        }
        if let Some(module) = task["module_name"].as_str() {
            let caption = label(module, "list-meta");
            caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
            caption.set_max_width_chars(14);
            row.append(&caption);
        }
        if let Some(agent) = rows(task, "sessions").last().and_then(|v| v.as_str().map(str::to_string)) {
            let caption = label(&agent, "list-agent");
            caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
            caption.set_max_width_chars(12);
            row.append(&caption);
        }
        let when = label(&ago(text(task, "updated_at")), "card-ago");
        when.set_width_chars(6);
        when.set_xalign(1.);
        row.append(&when);
    }

    /// Click, keyboard focus, context menu, drag and drop — shared by cards and list rows.
    fn wire(self: &Rc<Self>, widget: &gtk::Box, id: i64) {
        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        focus.connect_enter(move |_| {
            if let Some(board) = weak.upgrade() {
                board.focused.set(Some(id));
            }
        });
        widget.add_controller(focus);

        let click = gtk::GestureClick::new();
        click.set_button(0);
        let weak = Rc::downgrade(self);
        click.connect_pressed(|gesture, _, _, _| {
            if let Some(widget) = gesture.widget() {
                widget.grab_focus();
            }
        });
        click.connect_released(move |gesture, presses, x, y| {
            let (Some(board), Some(widget)) = (weak.upgrade(), gesture.widget()) else {
                return;
            };
            let state = gesture.current_event_state();
            match gesture.current_button() {
                3 => board.menu(&widget, id, Some((x, y))),
                2 => board.toggle_peek(id),
                1 if state.contains(gdk::ModifierType::CONTROL_MASK) => board.toggle_peek(id),
                // While the peek panel is open a click retargets it and a double click opens;
                // otherwise one click opens, and the second press of a double click is ignored.
                1 if presses == 1 && board.peeked.get().is_some() => {
                    board.peeked.set(Some(id));
                    board.render_peek();
                }
                1 if presses == 1 || board.peeked.get().is_some() => {
                    if let Some(ui) = board.ui() {
                        task_pages::open(&ui, id);
                    }
                }
                _ => {}
            }
        });
        widget.add_controller(click);

        let drag = gtk::DragSource::new();
        drag.set_actions(gdk::DragAction::MOVE);
        let grab = Rc::new(Cell::new((0., 0.)));
        let at = grab.clone();
        drag.connect_prepare(move |_, x, y| {
            at.set((x, y));
            Some(gdk::ContentProvider::for_value(&format!("relay-task:{id}").to_value()))
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_begin(move |source, _| {
            let Some(board) = weak.upgrade() else { return };
            board.dragged.set(Some(id));
            board.rail.set_visible(true);
            if let Some(widget) = source.widget() {
                let (x, y) = grab.get();
                // A still image of the card: the live one fades with the `dragging` class.
                let image = gtk::WidgetPaintable::new(Some(&widget)).current_image();
                source.set_icon(Some(&image), x as i32, y as i32);
                widget.add_css_class("dragging");
            }
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_end(move |source, _, _| {
            if let Some(widget) = source.widget() {
                widget.remove_css_class("dragging");
            }
            if let Some(board) = weak.upgrade() {
                board.dragged.set(None);
                board.rail.set_visible(false);
            }
        });
        widget.add_controller(drag);

        let drop = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
        let classes = ["drop-before", "drop-into", "drop-after"];
        let weak = Rc::downgrade(self);
        drop.connect_motion(move |target, _, y| {
            let Some(widget) = target.widget() else {
                return gdk::DragAction::empty();
            };
            let dragging_self = weak.upgrade().and_then(|b| b.dragged.get()) == Some(id);
            for class in classes {
                widget.remove_css_class(class);
            }
            if dragging_self {
                return gdk::DragAction::empty();
            }
            widget.add_css_class(match zone(y, widget.height() as f64) {
                Zone::Before => "drop-before",
                Zone::Into => "drop-into",
                Zone::After => "drop-after",
            });
            gdk::DragAction::MOVE
        });
        drop.connect_leave(move |target| {
            if let Some(widget) = target.widget() {
                for class in classes {
                    widget.remove_css_class(class);
                }
            }
        });
        let weak = Rc::downgrade(self);
        drop.connect_drop(move |target, token, _, y| {
            let Some(widget) = target.widget() else { return false };
            for class in classes {
                widget.remove_css_class(class);
            }
            let (Some(board), Some(moving)) = (weak.upgrade(), drop_id(token)) else {
                return false;
            };
            board.drop_on(moving, id, zone(y, widget.height() as f64))
        });
        widget.add_controller(drop);
    }

    fn menu(self: &Rc<Self>, anchor: &gtk::Widget, id: i64, at: Option<(f64, f64)>) {
        let Some(task) = self.task(id) else { return };
        let pop = gtk::Popover::new();
        pop.add_css_class("board-menu");
        pop.set_has_arrow(false);
        pop.set_parent(anchor);
        if let Some((x, y)) = at {
            pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        }
        pop.connect_closed(|pop| {
            let pop = pop.clone();
            glib::idle_add_local_once(move || {
                if pop.parent().is_some() {
                    pop.unparent();
                }
            });
        });
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        menu.set_size_request(260, -1);
        let heading = label(&format!("#{id} {}", text(&task, "title")), "board-menu-title");
        heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
        heading.set_max_width_chars(34);
        menu.append(&heading);
        let item = |icon: &str, caption: &str, key: &str| {
            let b = button("", "board-menu-item");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            content.append(&crate::icons::image(icon, 12));
            content.append(&label(caption, ""));
            content.append(&spacer());
            if !key.is_empty() {
                content.append(&label(key, "board-kbd"));
            }
            b.set_child(Some(&content));
            b
        };
        let weak = Rc::downgrade(self);
        let close = pop.downgrade();
        let run = move |f: Action| {
            let weak = weak.clone();
            let close = close.clone();
            move |_: &gtk::Button| {
                if let Some(pop) = close.upgrade() {
                    pop.popdown();
                }
                if let Some(board) = weak.upgrade() {
                    f(&board);
                }
            }
        };
        let open = item("external", "Open", "↵");
        open.connect_clicked(run(Box::new(move |board| {
            if let Some(ui) = board.ui() {
                task_pages::open(&ui, id);
            }
        })));
        menu.append(&open);
        let peek = item("sidebar", "Peek", "Space");
        peek.connect_clicked(run(Box::new(move |board| board.toggle_peek(id))));
        menu.append(&peek);

        // A caption with the key that does the same from a focused card, as Open and Peek show theirs.
        let section = |title: &str, key: &str| {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&label(title, "board-menu-caption"));
            row.append(&spacer());
            if !key.is_empty() {
                let cap = label(key, "board-kbd");
                cap.set_valign(gtk::Align::End);
                cap.set_margin_end(6);
                row.append(&cap);
            }
            row
        };
        menu.append(&section("MOVE TO", "[ ]"));
        let columns = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        columns.set_homogeneous(true);
        for (column, title) in COLUMN_TITLES {
            let key = button("", "board-menu-choice");
            key.set_child(Some(&status_icon(column, 13)));
            key.set_tooltip_text(Some(if column == "done" {
                "Done · approves the task and links the last agent's branch tip (the project HEAD if never dispatched)"
            } else {
                title
            }));
            if column == text(&task, "column") {
                key.add_css_class("selected");
                key.set_sensitive(false);
            }
            let moving = task.clone();
            key.connect_clicked(run(Box::new(move |board| {
                board.focused.set(Some(id));
                board.move_to(&moving, column, None);
            })));
            columns.append(&key);
        }
        menu.append(&columns);
        menu.append(&section("PRIORITY", "P"));
        let priorities = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        priorities.set_homogeneous(true);
        for priority in URGENT_FIRST {
            let key = button("", "board-menu-choice");
            key.set_child(Some(&priority_icon(priority)));
            if priority == text(&task, "priority") {
                key.add_css_class("selected");
            }
            key.connect_clicked(run(Box::new(move |board| {
                board.set_field(id, "priority", json!(priority));
            })));
            priorities.append(&key);
        }
        menu.append(&priorities);
        menu.append(&section("TYPE", ""));
        let kinds = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        kinds.set_homogeneous(true);
        for kind in TYPES {
            let key = button("", "board-menu-choice");
            key.set_child(Some(&task_mark(kind, 9)));
            key.set_tooltip_text(Some(kind));
            if kind == text(&task, "type") {
                key.add_css_class("selected");
            }
            key.connect_clicked(run(Box::new(move |board| {
                board.set_field(id, "type", json!(kind));
            })));
            kinds.append(&key);
        }
        menu.append(&kinds);
        menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let copy = item("copy", "Copy title, body and id", "");
        let weak = self.ui.clone();
        copy.connect_clicked(move |key| {
            copy_task(&weak, id, key.clipboard());
            if let Some(pop) = key.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>() {
                pop.popdown();
            }
        });
        menu.append(&copy);
        if !task["parent_id"].is_null() {
            let detach = item("arrow-left", "Detach from parent", "");
            detach.connect_clicked(run(Box::new(move |board| {
                board.act("task.parent.set", json!({"task_id":id,"parent_id":null}));
            })));
            menu.append(&detach);
        }
        let delete = item("trash", "Delete task", "");
        delete.add_css_class("destructive");
        delete.set_tooltip_text(Some("Ctrl+Z on the board restores it"));
        crate::app::confirm_inline(&delete, "Delete this task?", run(Box::new(move |board| {
            if board.peeked.get() == Some(id) {
                board.peeked.set(None);
            }
            board.act("task.delete", json!({"task_id":id}));
        })));
        menu.append(&delete);
        pop.set_child(Some(&menu));
        pop.popup();
    }

    /// A property row in the peek panel whose value opens a picker of `values`.
    fn picker(
        self: &Rc<Self>,
        values: &[&'static str],
        current: &str,
        icon: fn(&str) -> Option<gtk::Widget>,
        pick: impl Fn(&Rc<Board>, &'static str) + Clone + 'static,
    ) -> gtk::MenuButton {
        let name = |value: &str| -> String {
            if let Some((_, title)) = COLUMN_TITLES.iter().find(|(c, _)| *c == value) {
                title.to_string()
            } else if value.is_empty() {
                "None".into()
            } else {
                let mut chars = value.chars();
                chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
            }
        };
        let content = |value: &str| {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 7);
            if let Some(icon) = icon(value) {
                row.append(&icon);
            }
            row.append(&label(&name(value), ""));
            row
        };
        let menu = gtk::MenuButton::new();
        menu.add_css_class("peek-value");
        menu.set_child(Some(&content(current)));
        let pop = gtk::Popover::new();
        pop.add_css_class("board-menu");
        let list = gtk::Box::new(gtk::Orientation::Vertical, 1);
        for &value in values {
            let key = button("", "board-menu-item");
            key.set_child(Some(&content(value)));
            if value == current {
                key.add_css_class("selected");
            }
            let weak = Rc::downgrade(self);
            let close = pop.downgrade();
            let pick = pick.clone();
            key.connect_clicked(move |_| {
                if let Some(pop) = close.upgrade() {
                    pop.popdown();
                }
                if let Some(board) = weak.upgrade() {
                    pick(&board, value);
                }
            });
            list.append(&key);
        }
        pop.set_child(Some(&list));
        menu.set_popover(Some(&pop));
        menu
    }

    /// The peeked task and the tasks it links to, which is everything the peek draws.
    fn peek_key(&self) -> Option<String> {
        let id = self.peeked.get()?;
        let tasks = self.tasks.borrow();
        let task = tasks.iter().find(|t| t["id"].as_i64() == Some(id))?;
        let listed = |key: &str, other: &Value| task[key].as_array().is_some_and(|ids| ids.contains(&other["id"]));
        let linked: Vec<&Value> = tasks
            .iter()
            .filter(|t| t["id"] == task["parent_id"] || t["parent_id"] == task["id"] || listed("blocked_by", t) || listed("blocks", t))
            .collect();
        Some(json!([task, linked]).to_string())
    }

    fn render_peek(self: &Rc<Self>) {
        let key = self.peek_key();
        if key.is_some() && *self.peek_key.borrow() == key && self.peek.first_child().is_some() {
            return;
        }
        // The board holds summaries: the peek reads its task's body first, and until it has
        // it the panel stays as it was.
        let written = match self.peeked.get().and_then(|id| self.task(id)) {
            Some(task) => match Self::fresh(&self.bodies.borrow(), &task) {
                Some(body) => body,
                None => {
                    self.read_peeked(&task);
                    return;
                }
            },
            None => String::new(),
        };
        // The same task redrawn keeps its place in a long description.
        let same = key.is_some() && self.peek_shown.replace(self.peeked.get()) == self.peeked.get();
        let kept = same
            .then(|| self.peek.last_child().and_downcast::<gtk::ScrolledWindow>())
            .flatten()
            .map(|scroll| scroll.vadjustment().value());
        *self.peek_key.borrow_mut() = key;
        clear(&self.peek);
        let Some(id) = self.peeked.get() else {
            self.peek.set_visible(false);
            return;
        };
        let Some(task) = self.task(id) else {
            self.peeked.set(None);
            self.peek.set_visible(false);
            return;
        };
        let tasks = self.tasks.borrow().clone();
        self.peek.set_visible(true);
        let column = text(&task, "column").to_string();
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        head.add_css_class("peek-head");
        head.append(&status_icon(&column, 13));
        head.append(&label(&caption(&column).to_uppercase(), "lane-title"));
        head.append(&label(&format!("#{id}"), "card-id"));
        head.append(&spacer());
        let open = crate::app::icon_button("external", "Open the full editor (Enter)");
        let weak = self.ui.clone();
        open.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                task_pages::open(&ui, id);
            }
        });
        head.append(&open);
        let close = crate::app::icon_button("close", "Close (Esc)");
        let weak = Rc::downgrade(self);
        close.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                board.peeked.set(None);
                board.render_peek();
                // The focused key went with the peek.
                board.focus_board();
            }
        });
        head.append(&close);
        self.peek.append(&head);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.add_css_class("peek-body");
        let title = label(text(&task, "title"), "peek-title");
        title.set_wrap(true);
        title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        title.set_selectable(true);
        body.append(&title);

        let grid = gtk::Grid::new();
        grid.add_css_class("peek-props");
        grid.set_row_spacing(2);
        grid.set_column_spacing(12);
        let mut row = 0;
        let mut prop = |name: &str, value: &gtk::Widget| {
            let key = label(name, "peek-key");
            key.set_valign(gtk::Align::Center);
            grid.attach(&key, 0, row, 1, 1);
            value.set_halign(gtk::Align::Start);
            value.set_hexpand(true);
            grid.attach(value, 1, row, 1, 1);
            row += 1;
        };
        let moving = task.clone();
        prop(
            "Status",
            self.picker(
                COLUMNS,
                &column,
                |c| Some(status_icon(c, 13).upcast()),
                move |board, c| board.move_to(&moving, c, None),
            )
            .upcast_ref(),
        );
        prop(
            "Priority",
            self.picker(&URGENT_FIRST, text(&task, "priority"), |p| Some(priority_icon(p).upcast()), move |board, p| {
                board.set_field(id, "priority", json!(p));
            })
            .upcast_ref(),
        );
        prop(
            "Type",
            self.picker(&TYPES, text(&task, "type"), |t| Some(task_mark(t, 9).upcast()), move |board, t| {
                board.set_field(id, "type", json!(t));
            })
            .upcast_ref(),
        );
        prop(
            "Size",
            self.picker(&SIZES, text(&task, "size"), |_| None, move |board, s| {
                let size = if s.is_empty() { Value::Null } else { json!(s) };
                board.set_field(id, "size", size);
            })
            .upcast_ref(),
        );
        let value = |caption: &str| label(caption, "peek-text").upcast::<gtk::Widget>();
        prop(
            "State",
            &state_badge(text(&task, "state"))
                .map(|b| b.upcast::<gtk::Widget>())
                .unwrap_or_else(|| value("Idle")),
        );
        if let Some(module) = task["module_name"].as_str() {
            prop("Module", &value(module));
        }
        let agents: Vec<String> = rows(&task, "sessions").iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        prop("Agents", &value(&if agents.is_empty() { "Unassigned".into() } else { agents.join(" · ") }));
        if let Some(parent) = task["parent_id"].as_i64().and_then(|p| tasks.iter().find(|t| t["id"].as_i64() == Some(p))) {
            prop("Parent", self.task_link(parent).upcast_ref());
        }
        prop("Updated", &value(&since(text(&task, "updated_at"))));
        prop("Created", &value(&since(text(&task, "created_at"))));
        body.append(&grid);

        if let Some(tags) = labels_row(&task, 12) {
            tags.add_css_class("peek-labels");
            body.append(&tags);
        }
        body.append(&label("DESCRIPTION", "board-menu-caption"));
        let description = written.trim();
        let about = paragraph(if description.is_empty() { "No description." } else { description });
        about.add_css_class(if description.is_empty() { "peek-muted" } else { "peek-description" });
        about.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        body.append(&about);

        let children: Vec<&Value> = tasks.iter().filter(|t| t["parent_id"] == task["id"]).collect();
        if !children.is_empty() {
            let finished = children.iter().filter(|t| text(t, "column") == "done").count();
            body.append(&label(&format!("SUB-TASKS  {finished}/{}", children.len()), "board-menu-caption"));
            for child in children {
                body.append(&self.task_link(child));
            }
        }
        for (key, title) in [("blocked_by", "BLOCKED BY"), ("blocks", "BLOCKS")] {
            let linked: Vec<&Value> = rows(&task, key)
                .iter()
                .filter_map(|other| tasks.iter().find(|t| t["id"] == *other))
                .collect();
            if !linked.is_empty() {
                body.append(&label(title, "board-menu-caption"));
                for other in linked {
                    body.append(&self.task_link(other));
                }
            }
        }
        let commits = rows(&task, "commits");
        if !commits.is_empty() {
            body.append(&label("COMMITS", "board-menu-caption"));
            for commit in commits {
                let line = label(
                    &format!(
                        "{}  {}",
                        text(&commit, "sha").chars().take(10).collect::<String>(),
                        text(&commit, "branch")
                    ),
                    "peek-commit",
                );
                line.set_selectable(true);
                body.append(&line);
            }
        }
        let edit = button("Open full editor", "quiet");
        edit.add_css_class("peek-edit");
        let weak = self.ui.clone();
        edit.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                task_pages::open(&ui, id);
            }
        });
        body.append(&edit);
        let scroll = crate::app::scrolled(&body);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        if let Some(value) = kept.filter(|v| *v > 0.) {
            let done = Cell::new(false);
            scroll.vadjustment().connect_changed(move |a| {
                if !done.get() && a.upper() - a.page_size() >= value {
                    done.set(true);
                    a.set_value(value);
                }
            });
        }
        self.peek.append(&scroll);
    }

    /// Reads the peeked task's body, then draws the peek.
    fn read_peeked(self: &Rc<Self>, task: &Value) {
        let Some(id) = task["id"].as_i64() else { return };
        if self.peek_loading.replace(Some(id)) == Some(id) {
            return;
        }
        let Some(ui) = self.ui() else { return };
        let (board, listed) = (self.clone(), text(task, "updated_at").to_string());
        glib::spawn_future_local(async move {
            let read = ui.call("task.get", json!({"task_id":id})).await;
            // Unreadable (deleted meanwhile): the peek reads as having no description.
            let (at, body) = match &read {
                Ok(task) => (text(task, "updated_at").to_string(), text(task, "body").to_string()),
                Err(_) => (listed, String::new()),
            };
            board.bodies.borrow_mut().insert(id, (at, body));
            if board.peek_loading.get() == Some(id) {
                board.peek_loading.set(None);
            }
            if board.peeked.get() == Some(id) {
                board.render_peek();
            }
        });
    }

    fn task_link(self: &Rc<Self>, task: &Value) -> gtk::Button {
        let id = task["id"].as_i64().unwrap_or(0);
        let key = button("", "peek-link");
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        content.append(&status_icon(text(task, "column"), 12));
        content.append(&label(&format!("#{id}"), "card-id"));
        let title = label(text(task, "title"), "");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_hexpand(true);
        content.append(&title);
        key.set_child(Some(&content));
        if text(task, "column") == "done" {
            key.add_css_class("done");
        }
        let weak = Rc::downgrade(self);
        key.connect_clicked(move |_| {
            if let Some(board) = weak.upgrade() {
                board.peeked.set(Some(id));
                board.focused.set(Some(id));
                board.render_peek();
                board.focus_board();
            }
        });
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ages_read_at_a_glance() {
        // The wording itself is relative.rs's; the board owns what an unreadable time shows.
        let minutes = |m: i32| glib::DateTime::now_utc().unwrap().add_minutes(-m).unwrap().format_iso8601().unwrap().to_string();
        assert_eq!(ago(&minutes(45)), "45m");
        assert_eq!(ago(""), "");
        assert_eq!(since(&minutes(0)), "just now");
        assert_eq!(since(&minutes(45)), "45m ago");
        assert_eq!(since(""), "—");
    }
    #[test]
    fn derived_orders_follow_the_one_vocabulary() {
        assert_eq!(COLUMNS, COLUMN_TITLES.map(|(c, _)| c));
        assert_eq!(OPEN_COLUMNS, &COLUMNS[..COLUMNS.len() - 1]);
        assert!(!OPEN_COLUMNS.contains(&"done"));
    }
}
