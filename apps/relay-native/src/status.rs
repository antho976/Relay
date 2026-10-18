use super::*;
use std::collections::VecDeque;

#[path = "status_usage.rs"]
mod usage;
pub(super) use usage::UsageState;

thread_local! {
    /// A device count is being read, and whether another event asked for one meanwhile: a
    /// burst of device and run events reads it at most twice.
    static DEVICES_BUSY: Cell<bool> = const { Cell::new(false) };
    static DEVICES_QUEUED: Cell<bool> = const { Cell::new(false) };
}

impl Ui {
    pub(super) fn render_status_counts(&self) {
        clear(&self.resource_status);
        let sessions = self.sessions.borrow();
        let blocked = sessions
            .iter()
            .filter(|s| text(s, "state") == "blocked")
            .count();
        let live = sessions
            .iter()
            .filter(|s| matches!(text(s, "state"), "spawning" | "running" | "idle"))
            .count();
        let parked = sessions
            .iter()
            .filter(|s| text(s, "state") == "parked")
            .count();
        for (count, caption, color) in [
            (blocked, if blocked == 1 { "needs you" } else { "need you" }, (0.898, 0.22, 0.18)),
            (
                live,
                "live",
                if live > 0 {
                    (0.18, 0.77, 0.41)
                } else {
                    (0.35, 0.35, 0.37)
                },
            ),
            (parked, "parked", (0.47, 0.47, 0.48)),
        ] {
            if count == 0 && caption != "live" {
                continue;
            }
            if self.resource_status.first_child().is_some() {
                self.resource_status.append(&label("·", "faint"));
            }
            if caption != "parked" {
                let lamp = gtk::DrawingArea::new();
                lamp.set_content_width(6);
                lamp.set_content_height(6);
                lamp.set_valign(gtk::Align::Center);
                lamp.set_draw_func(move |_, cr, _, _| {
                    cr.set_source_rgb(color.0, color.1, color.2);
                    cr.arc(3., 3., 3., 0., std::f64::consts::TAU);
                    let _ = cr.fill();
                });
                self.resource_status.append(&lamp);
            }
            self.resource_status.append(&label(
                &format!("{count} {caption}"),
                if caption == "parked" {
                    "faint"
                } else {
                    "status-count"
                },
            ));
        }
    }

    pub(super) fn refresh_status(self: &Rc<Self>) {
        self.render_status_counts();
        self.refresh_usage(false);
        self.refresh_devices();
        self.refresh_gaming();
    }

    pub(super) fn refresh_devices(self: &Rc<Self>) {
        if self.client.borrow().is_none() {
            return;
        }
        if DEVICES_BUSY.with(|b| b.replace(true)) {
            DEVICES_QUEUED.with(|q| q.set(true));
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let generation = ui.generation.get();
            // A connection of its own: `adb devices` may take its full 2 s while it starts the
            // adb server, and on the control connection that wait would sit in front of the
            // first project and session reads after connect.
            let list = || Client::lifecycle_request(&ui.rt, ui.path.clone(), "device.list", json!({}));
            let mut devices = list().await;
            if matches!(&devices, Err(Error::Bus(e)) if e.code == "device.adb_timeout") {
                // The server was starting; it answers once it is up, rather than leaving
                // "No device" until the next device event.
                glib::timeout_future_seconds(1).await;
                devices = list().await;
            }
            DEVICES_BUSY.with(|b| b.set(false));
            if DEVICES_QUEUED.with(|q| q.replace(false)) {
                ui.refresh_devices();
            }
            if generation != ui.generation.get() {
                return;
            }
            if let Ok(v) = devices {
                let n = rows(&v, "devices").len();
                ui.device_status.set_text(&match n {
                    0 => "No device".into(),
                    1 => "1 device".into(),
                    _ => format!("{n} devices"),
                });
            }
        });
    }


    pub fn resources(self: &Rc<Self>) {
        let Some(panel) = crate::panel::Panel::toggle(self, "Resources", 400) else {
            return;
        };
        panel.bottom(220);
        panel.add_css_class("resources-panel");
        panel.add_css_class("floating-panel");
        let body = panel.body.clone();
        body.add_css_class("resources-body");
        body.set_spacing(0);
        body.append(&label("Connecting…", "resource-empty"));
        let ui = self.clone();
        let task = glib::spawn_future_local(async move {
            let result = Client::connect(&ui.rt, ui.path.clone()).await;
            let (client, notices) = match result {
                Ok(v) => v,
                Err(e) => {
                    clear(&body);
                    body.append(&label(&e.to_string(), "resource-empty"));
                    return;
                }
            };
            let history = Rc::new(RefCell::new(VecDeque::new()));
            let mut view = None;
            let result = async {
                client
                    .request(
                        &ui.rt,
                        "bus.subscribe",
                        json!({"events":["resource.sample"]}),
                    )
                    .await?;
                client
                    .request(&ui.rt, "app.resources.watch", json!({"on":true}))
                    .await?;
                client.request(&ui.rt, "app.resources.get", json!({})).await
            }
            .await;
            match result {
                Ok(v) => render_resources(&body, &v, &history, &mut view),
                Err(e) => {
                    clear(&body);
                    body.append(&label(&e.to_string(), "resource-empty"));
                    return;
                }
            }
            while let Ok(notice) = notices.recv().await {
                match notice {
                    Notice::Event(e) if e.ev == "resource.sample" => {
                        render_resources(&body, &e.payload, &history, &mut view)
                    }
                    Notice::Disconnected(e) => {
                        body.append(&label(&e.to_string(), "resource-empty"));
                        break;
                    }
                    _ => {}
                }
            }
        });
        // Closing the socket releases its sampling lease, including app shutdown.
        panel.on_closed(move || task.abort());
        panel.present();
    }
}

/// D66: the panel keeps the last 30 samples.
const HISTORY: usize = 30;

/// The open panel's widgets, built on the first sample. A sample updates them in place; a
/// list is rebuilt only when its set of sessions or worktrees changes, so a hovered row keeps
/// its tooltip and a sample every 2 s does not recreate every row.
struct ResourceView {
    tiles: Vec<(gtk::Label, gtk::DrawingArea)>,
    engine: gtk::Label,
    tabs: [gtk::ToggleButton; 2],
    sessions: ResourceList,
    worktrees: ResourceList,
}

struct ResourceList {
    root: gtk::Box,
    /// Sorted, so a reordering alone does not rebuild the list.
    keys: Vec<String>,
    rows: Vec<(String, ResourceRow)>,
}

struct ResourceRow {
    root: gtk::Box,
    bar: gtk::ProgressBar,
    value: gtk::Label,
    size: gtk::Label,
}

struct RowData {
    key: String,
    name: String,
    /// Faint text after the name: a worktree's project.
    note: String,
    tip: String,
    fraction: f64,
    value: String,
    size: f64,
}

/// Megabytes as people read them: MB below a gigabyte, one decimal below ten.
fn amount(mb: f64) -> String {
    match mb.max(0.) {
        mb if mb < 1024. => format!("{mb:.0} MB"),
        mb if mb < 10240. => format!("{:.1} GB", mb / 1024.),
        mb => format!("{:.0} GB", mb / 1024.),
    }
}

/// The project a worktree belongs to: the directory holding `.relay/worktrees`, or the one
/// above a checkout of its own.
fn project_of(path: &str) -> &str {
    let root = path.split("/.relay/worktrees/").next().filter(|_| path.contains("/.relay/worktrees/"));
    let root = root.unwrap_or_else(|| path.rsplit_once('/').map_or("", |(dir, _)| dir));
    root.rsplit('/').next().unwrap_or(root)
}

/// Unchanged text is left alone: resetting a label's tooltip hides one being read.
fn set_tip(widget: &impl IsA<gtk::Widget>, s: &str) {
    if widget.tooltip_text().as_deref() != Some(s) {
        widget.set_tooltip_text(Some(s));
    }
}

/// A meter's width in a resource row.
const METER_W: i32 = 56;

fn resource_row(item: &RowData) -> ResourceRow {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    root.add_css_class("resource-row");
    let title = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    title.set_hexpand(true);
    let name = label(&item.name, "resource-name");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.append(&name);
    if !item.note.is_empty() {
        let note = label(&item.note, "resource-note");
        note.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.append(&note);
    }
    root.append(&title);
    let value = label("", "resource-value");
    value.set_size_request(58, -1);
    value.set_xalign(1.);
    root.append(&value);
    // Beside the size it measures: next to the CPU figure, a session's memory bar read as its CPU.
    let bar = gtk::ProgressBar::new();
    bar.set_valign(gtk::Align::Center);
    bar.add_css_class("resource-meter");
    bar.set_size_request(METER_W, -1);
    root.append(&bar);
    let size = label("", "resource-size");
    size.set_size_request(60, -1);
    size.set_xalign(1.);
    root.append(&size);
    ResourceRow { root, bar, value, size }
}

/// The two column captions over a list, aligned with its value and size columns.
fn list_head(value: &str, size: &str) -> gtk::Box {
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    head.add_css_class("resource-head");
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    head.append(&spacer);
    let value = label(value, "resource-head-label");
    value.set_size_request(58, -1);
    value.set_xalign(1.);
    head.append(&value);
    // Over the meters, which have no caption of their own.
    let meters = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    meters.set_size_request(METER_W, -1);
    head.append(&meters);
    let size = label(size, "resource-head-label");
    size.set_size_request(60, -1);
    size.set_xalign(1.);
    head.append(&size);
    head
}

impl ResourceList {
    fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("resource-list");
        ResourceList { root, keys: Vec::new(), rows: Vec::new() }
    }

    /// Rebuilds when the set of rows (or the footnote) changed, largest first; then writes
    /// each row's numbers in place.
    fn sync(&mut self, mut items: Vec<RowData>, head: (&str, &str), footnote: Option<String>, empty: &str) {
        let mut keys: Vec<String> = items.iter().map(|i| i.key.clone()).collect();
        keys.extend(footnote.clone());
        keys.sort();
        if keys != self.keys {
            items.sort_by(|a, b| b.size.total_cmp(&a.size));
            clear(&self.root);
            self.rows.clear();
            if items.is_empty() {
                self.root.append(&label(empty, "resource-empty"));
            } else {
                self.root.append(&list_head(head.0, head.1));
            }
            for item in &items {
                let row = resource_row(item);
                self.root.append(&row.root);
                self.rows.push((item.key.clone(), row));
            }
            if let Some(footnote) = footnote {
                self.root.append(&label(&footnote, "resource-footnote"));
            }
            self.keys = keys;
        }
        for item in &items {
            let Some((_, row)) = self.rows.iter().find(|(key, _)| *key == item.key) else {
                continue;
            };
            set_tip(&row.root, &item.tip);
            row.bar.set_fraction(item.fraction.clamp(0., 1.));
            row.value.set_text(&item.value);
            row.size.set_text(&amount(item.size));
        }
    }
}

fn build_resources(body: &gtk::Box, history: &Rc<RefCell<VecDeque<[f64; 3]>>>) -> ResourceView {
    clear(body);
    let tiles_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    tiles_box.set_homogeneous(true);
    tiles_box.add_css_class("resource-tiles");
    let mut tiles = Vec::new();
    for (i, caption) in ["CPU", "Memory", "Disk"].into_iter().enumerate() {
        let tile = gtk::Box::new(gtk::Orientation::Vertical, 2);
        tile.add_css_class("resource-tile");
        tile.append(&label(caption, "resource-tile-caption"));
        let value = label("", "resource-tile-value");
        tile.append(&value);
        let graph = gtk::DrawingArea::new();
        graph.add_css_class("resource-spark");
        graph.set_content_height(28);
        let history = history.clone();
        graph.set_draw_func(move |area, cr, w, h| {
            let history = history.borrow();
            if history.len() < 2 {
                return;
            }
            let max = history.iter().map(|p| p[i]).fold(0., f64::max);
            // A flat series sits at mid-height rather than on the floor.
            let (lo, span) = {
                let min = history.iter().map(|p| p[i]).fold(f64::MAX, f64::min);
                if max - min < max * 0.02 + 1e-6 {
                    (min - 1., 2.)
                } else {
                    (min, max - min)
                }
            };
            let points: Vec<(f64, f64)> = history
                .iter()
                .enumerate()
                .map(|(n, p)| {
                    let x = w as f64 * n as f64 / (history.len() - 1) as f64;
                    let y = h as f64 - 2. - (p[i] - lo) / span * (h - 4) as f64;
                    (x, y)
                })
                .collect();
            let c = area.color();
            let (r, g, b) = (c.red() as f64, c.green() as f64, c.blue() as f64);
            for (n, (x, y)) in points.iter().enumerate() {
                if n == 0 {
                    cr.move_to(*x, *y);
                } else {
                    cr.line_to(*x, *y);
                }
            }
            cr.line_to(w as f64, h as f64);
            cr.line_to(0., h as f64);
            cr.close_path();
            cr.set_source_rgba(r, g, b, 0.10);
            let _ = cr.fill();
            for (n, (x, y)) in points.iter().enumerate() {
                if n == 0 {
                    cr.move_to(*x, *y);
                } else {
                    cr.line_to(*x, *y);
                }
            }
            cr.set_source_rgba(r, g, b, 0.85);
            cr.set_line_width(1.25);
            let _ = cr.stroke();
        });
        tile.append(&graph);
        tiles_box.append(&tile);
        tiles.push((value, graph));
    }
    body.append(&tiles_box);
    let engine = label("", "resource-engine");
    engine.set_ellipsize(gtk::pango::EllipsizeMode::End);
    body.append(&engine);

    let strip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    strip.add_css_class("resource-tabs");
    strip.set_halign(gtk::Align::Start);
    let sessions_tab = gtk::ToggleButton::with_label("Sessions");
    let worktrees_tab = gtk::ToggleButton::with_label("Worktrees");
    worktrees_tab.set_group(Some(&sessions_tab));
    sessions_tab.set_active(true);
    strip.append(&sessions_tab);
    strip.append(&worktrees_tab);
    body.append(&strip);

    let sessions = ResourceList::new();
    let worktrees = ResourceList::new();
    let stack = gtk::Stack::new();
    stack.set_vhomogeneous(false);
    stack.set_interpolate_size(false);
    stack.add_named(&sessions.root, Some("sessions"));
    stack.add_named(&worktrees.root, Some("worktrees"));
    for (tab, name) in [(&sessions_tab, "sessions"), (&worktrees_tab, "worktrees")] {
        let stack = stack.clone();
        tab.connect_toggled(move |tab| {
            if tab.is_active() {
                stack.set_visible_child_name(name);
            }
        });
    }
    body.append(&stack);
    ResourceView { tiles, engine, tabs: [sessions_tab, worktrees_tab], sessions, worktrees }
}

fn render_resources(
    body: &gtk::Box,
    v: &Value,
    history: &Rc<RefCell<VecDeque<[f64; 3]>>>,
    view: &mut Option<ResourceView>,
) {
    let panes = rows(v, "panes");
    let trees = rows(v, "worktrees");
    let num = |v: &Value, key: &str| v[key].as_f64().unwrap_or(0.);
    let relay = &v["relay"];
    let cpu = panes.iter().map(|p| num(p, "cpu_pct")).sum::<f64>() + num(relay, "cpu_pct");
    let memory = num(v, "total_rss_mb");
    let disk = trees.iter().map(|t| num(t, "disk_mb")).sum::<f64>();
    {
        let mut history = history.borrow_mut();
        history.push_back([cpu, memory, disk]);
        while history.len() > HISTORY {
            history.pop_front();
        }
    }
    let view = view.get_or_insert_with(|| build_resources(body, history));

    for ((value, graph), text) in view.tiles.iter().zip([format!("{cpu:.1}%"), amount(memory), amount(disk)]) {
        value.set_text(&text);
        graph.queue_draw();
    }
    view.engine.set_text(&format!(
        "Relay engine {} · {:.1}% CPU · store {}",
        amount(num(relay, "rss_mb")),
        num(relay, "cpu_pct"),
        amount(num(v, "store_mb")),
    ));

    // A parked session has no process: it is counted, not listed.
    let (live, parked): (Vec<_>, Vec<_>) = panes.iter().partition(|p| p["pid"].as_i64().is_some());
    // Each session's share of what they use together: six alike are six sixths, not six full bars.
    let total_rss = live.iter().map(|p| num(p, "rss_mb")).sum::<f64>().max(1.);
    let sessions = live
        .iter()
        .map(|p| {
            let name = text(p, "session");
            RowData {
                key: name.to_owned(),
                name: name.to_owned(),
                note: String::new(),
                tip: format!("{name} · pid {}", p["pid"]),
                fraction: num(p, "rss_mb") / total_rss,
                value: format!("{:.1}%", num(p, "cpu_pct")),
                size: num(p, "rss_mb"),
            }
        })
        .collect();
    let footnote = match parked.len() {
        0 => None,
        1 => Some("1 parked session, using no memory".to_owned()),
        n => Some(format!("{n} parked sessions, using no memory")),
    };
    view.tabs[0].set_label(&format!("Sessions  {}", live.len()));
    view.sessions.sync(sessions, ("CPU", "Memory"), footnote, "No agent is running.");

    let total_disk = trees.iter().map(|t| num(t, "disk_mb")).sum::<f64>().max(1.);
    let worktrees = trees
        .iter()
        .map(|t| {
            let path = text(t, "path");
            let build = t["build_mb"].as_f64();
            RowData {
                key: path.to_owned(),
                name: path.rsplit('/').next().unwrap_or(path).to_owned(),
                note: project_of(path).to_owned(),
                tip: match build {
                    Some(mb) => format!("{path}\n{} of it is build output", amount(mb)),
                    None => path.to_owned(),
                },
                fraction: num(t, "disk_mb") / total_disk,
                value: build.map(amount).unwrap_or_else(|| "–".into()),
                size: num(t, "disk_mb"),
            }
        })
        .collect();
    view.tabs[1].set_label(&format!("Worktrees  {}", trees.len()));
    view.worktrees.sync(worktrees, ("Build", "Size"), None, "No worktrees.");
}

#[cfg(test)]
mod tests {
    use super::{amount, project_of};

    #[test]
    fn amounts_read_in_the_unit_people_use() {
        assert_eq!(amount(75.4), "75 MB");
        assert_eq!(amount(2432.7), "2.4 GB");
        assert_eq!(amount(64887.8), "63 GB");
    }

    #[test]
    fn a_worktree_names_its_project() {
        assert_eq!(project_of("/home/a/dev3/RelayV4/.relay/worktrees/witty-ibis"), "RelayV4");
        assert_eq!(project_of("/home/a/dev3/ShooterGame2/UE5-shooter-game"), "ShooterGame2");
    }
}
