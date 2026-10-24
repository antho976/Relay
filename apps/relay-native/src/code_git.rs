use super::*;
use crate::client::Error;
use relay_client::git_view::{
    change_summary, commit_graph, diff_marks, has_conflict_markers, inline_diff, is_conflict, is_staged, is_unstaged,
    split_path, GraphRow, Mark,
};

struct GitRefreshGuard(Rc<Editor>, Rc<Ui>);
impl Drop for GitRefreshGuard {
    fn drop(&mut self) {
        self.0.git_refresh_pending.set(false);
        if self.0.git_refresh_dirty.replace(false) {
            self.0.refresh_git(&self.1);
        }
    }
}

thread_local! {
    /// The branch-name entry's Enter handler, replaced with each picker render.
    static BRANCH_ENTER: RefCell<Option<glib::SignalHandlerId>> = const { RefCell::new(None) };
    /// Source Control sections the person folded; kept across refreshes, as VS Code does.
    static FOLDED: RefCell<std::collections::HashSet<&'static str>> =
        RefCell::new(["merge"].into_iter().collect());
    /// The panel's view as last read from or written to [`VIEW_KEY`]; `None` until it has been
    /// read, so nothing is written over a saved view before it is known.
    static SAVED_VIEW: RefCell<Option<Value>> = const { RefCell::new(None) };
    static VIEW_TIMER: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}
/// Where the Git panel's view (the Graph key, where the history split sits and which sections
/// are folded) is kept between runs, as Notes keeps its window in `native.notes.window`.
const VIEW_KEY: &str = "native.code";
/// Every Source Control section, so a fold read back names one of them.
const SECTIONS: [&str; 6] = ["conflicts", "staged", "changes", "remote", "history", "merge"];
fn folded(id: &'static str) -> bool {
    FOLDED.with(|folded| folded.borrow().contains(id))
}
fn set_folded(id: &'static str, fold: bool) {
    FOLDED.with(|folded| {
        if fold {
            folded.borrow_mut().insert(id);
        } else {
            folded.borrow_mut().remove(id);
        }
    });
}

/// A collapsible Source Control group: chevron, title, count and its header actions.
struct Section {
    root: gtk::Box,
    actions: gtk::Box,
    body: gtk::Box,
    toggle: gtk::Button,
}
fn section(id: &'static str, title: &str, count: Option<usize>) -> Section {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("scm-section");
    root.set_widget_name(&format!("git-section-{id}"));
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    head.add_css_class("scm-section-head");
    let toggle = gtk::Button::new();
    toggle.add_css_class("scm-toggle");
    toggle.set_hexpand(true);
    let face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let chevron = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    chevron.add_css_class("scm-chevron");
    face.append(&chevron);
    face.append(&label(title, "scm-title"));
    if let Some(count) = count {
        let badge = label(&count.to_string(), "scm-count");
        badge.set_valign(gtk::Align::Center);
        face.append(&badge);
    }
    toggle.set_child(Some(&face));
    head.append(&toggle);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    actions.add_css_class("scm-head-actions");
    actions.set_valign(gtk::Align::Center);
    head.append(&actions);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.add_css_class("scm-section-body");
    root.append(&head);
    root.append(&body);
    let paint = move |open: bool| {
        clear(&chevron);
        chevron.append(&crate::icons::image(
            if open { "chevron-down" } else { "chevron-right" },
            12,
        ));
    };
    let open = !folded(id);
    body.set_visible(open);
    actions.set_visible(open);
    paint(open);
    toggle.update_state(&[gtk::accessible::State::Expanded(Some(open))]);
    let fold_body = body.clone();
    let fold_actions = actions.clone();
    toggle.connect_clicked(move |toggle| {
        let open = !fold_body.is_visible();
        fold_body.set_visible(open);
        fold_actions.set_visible(open);
        set_folded(id, !open);
        paint(open);
        toggle.update_state(&[gtk::accessible::State::Expanded(Some(open))]);
    });
    Section {
        root,
        actions,
        body,
        toggle,
    }
}

/// Ahead/behind as small arrows and counts, for the branch header and the picker.
fn sync_counts(ahead: Option<i64>, behind: Option<i64>) -> Option<gtk::Box> {
    let ahead = ahead.filter(|n| *n > 0);
    let behind = behind.filter(|n| *n > 0);
    if ahead.is_none() && behind.is_none() {
        return None;
    }
    let counts = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    counts.add_css_class("git-sync-counts");
    for (icon, count) in [("arrow-down", behind), ("arrow-up", ahead)] {
        if let Some(count) = count {
            counts.append(&crate::icons::image(icon, 11));
            counts.append(&label(&count.to_string(), "git-sync-count"));
        }
    }
    Some(counts)
}

/// Rows drawn per change group or commit; the rest is one "more files" line.
const CHANGE_ROWS: usize = 500;
/// Commits History lists at first, and each "Show more" adds; `git.log` gives at most 500.
pub(super) const HISTORY_PAGE: u32 = 30;
const HISTORY_MOST: u32 = 500;
const UNRESOLVED: &str = "Resolve the merge conflicts and mark them resolved before committing";

fn more_files(total: usize) -> Option<gtk::Label> {
    let hidden = total.checked_sub(CHANGE_ROWS).filter(|n| *n > 0)?;
    let more = label(
        &format!("{hidden} more file{} not listed", if hidden == 1 { "" } else { "s" }),
        "dim",
    );
    more.set_xalign(0.);
    more.set_margin_start(12);
    more.set_margin_top(4);
    more.set_margin_bottom(4);
    Some(more)
}

/// What the Git panel draws, hashed: a refresh that fetched the same thing leaves it alone.
fn git_signature(ui: &Ui, e: &Editor, status: &Value, results: &GitResults) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    status.to_string().hash(&mut hash);
    // How many commits were asked for: "Show more" that brings nothing new still redraws, so
    // its key goes instead of staying greyed.
    e.git_history.get().hash(&mut hash);
    for result in [&results.0, &results.1, &results.2] {
        match result {
            Ok(value) => value.to_string().hash(&mut hash),
            Err(error) => error.to_string().hash(&mut hash),
        }
    }
    for session in ui.sessions.borrow().iter().filter(|s| text(s, "state") != "closed") {
        (text(session, "name"), text(session, "branch")).hash(&mut hash);
    }
    for worktree in e.worktrees.borrow().iter() {
        (text(worktree, "path"), text(worktree, "branch")).hash(&mut hash);
    }
    hash.finish()
}

type GitResults = (Result<Value, Error>, Result<Value, Error>, Result<Value, Error>);

/// The pull request lookup, apart from the rest of the panel: `gh` answers in a fraction of a
/// second signed out and only at its timeout offline, and the panel does not wait for it.
#[derive(Default)]
pub(super) struct PrState {
    /// The project the last answer is for, and the answer: the listing, or that it failed.
    answer: Option<(i64, Result<Value, ()>)>,
    /// A lookup is in flight for this project.
    pending: Option<i64>,
    /// Refresh was pressed while one was in flight: ask GitHub afresh when it answers.
    again: bool,
    /// The row drawn last, filled in place when the answer arrives.
    row: Option<Rc<PrRow>>,
}

/// The Remote section's pull request row and what describes the branch it is for.
pub(super) struct PrRow {
    project: i64,
    row: gtk::Box,
    copy: gtk::Box,
    title: gtk::Label,
    hint: gtk::Label,
    key: gtk::Button,
    branch: String,
    upstream: bool,
    ahead: Option<i64>,
    behind: Option<i64>,
    /// Whether the branch is merged, as `git.branches` reports it; None when it is not listed.
    merged: Option<bool>,
    /// The pull request the key opens; none opens a new one.
    url: Rc<RefCell<Option<String>>>,
}

impl PrRow {
    /// Describe the branch's pull request from a `git.pr.list` answer.
    fn fill(&self, answer: &Result<Value, ()>) {
        let pull_request = answer.as_ref().ok().and_then(|value| {
            let matches: Vec<_> = rows(value, "pull_requests")
                .into_iter()
                .filter(|pr| pr["same_repository"] == true && text(pr, "branch") == self.branch)
                .collect();
            matches
                .iter()
                .find(|pr| text(pr, "state") == "open")
                .or_else(|| matches.first())
                .cloned()
        });
        let complete = answer.as_ref().ok().is_some_and(|value| value["complete"] == true);
        let ready = complete
            && self.upstream
            && self.ahead == Some(0)
            && self.behind == Some(0)
            && self.merged == Some(false);
        if let Some(old) = self.copy.next_sibling().filter(|w| w.has_css_class("scm-tag")) {
            self.row.remove(&old);
        }
        let hint = if let Some(pr) = &pull_request {
            let state = if text(pr, "state") == "open" && pr["draft"] == true {
                "draft"
            } else {
                text(pr, "state")
            };
            let badge = label(state, "scm-tag");
            badge.add_css_class(&format!("pr-{state}"));
            badge.set_valign(gtk::Align::Center);
            self.row.insert_child_after(&badge, Some(&self.copy));
            let title = text(pr, "title");
            self.title.set_text(&if title.is_empty() {
                format!("Pull request #{}", pr["number"])
            } else {
                format!("#{} {title}", pr["number"])
            });
            format!("Pull request · {state}")
        } else {
            self.title.set_text("Pull request");
            if answer.is_err() {
                String::from("GitHub unavailable · status unknown")
            } else if !complete {
                String::from("Lookup incomplete")
            } else if ready {
                String::from("Pushed and ready for review")
            } else if self.merged == Some(true) {
                String::from("Nothing to review: every commit is already on the base branch")
            } else {
                String::from("Push the branch first")
            }
        };
        self.hint.set_text(&hint);
        self.hint.set_tooltip_text(Some(&hint));
        self.key.set_sensitive(pull_request.is_some() || ready);
        self.key.set_label(if pull_request.is_some() { "Open" } else { "Open PR" });
        if ready && pull_request.is_none() {
            self.key.add_css_class("primary");
        } else {
            self.key.remove_css_class("primary");
        }
        *self.url.borrow_mut() = pull_request.as_ref().and_then(|pr| pr["url"].as_str()).map(str::to_owned);
    }
}

/// Where the keyboard is among the change rows under `git`: the row's group ("staged" or
/// "worktree"), its path, its place in the group, and whether the focus is on its stage key
/// rather than the row. A rebuild puts it back with `refocus_change`.
type ChangeFocus = (String, String, usize, bool);
fn focused_change(git: &gtk::Box) -> Option<ChangeFocus> {
    let focus = git.root()?.focus()?;
    if !focus.is_ancestor(git) {
        return None;
    }
    // A row is the open key, named git-change:{group}:{path}, then its stage key (`change_row`).
    let row = std::iter::successors(Some(focus.clone()), |w| w.parent()).find(|w| w.has_css_class("scm-row"))?;
    let open = row.first_child()?;
    let name = open.widget_name();
    let (group, path) = name.strip_prefix("git-change:")?.split_once(':')?;
    let index = change_keys(git.upcast_ref(), group).iter().position(|key| *key == open)?;
    Some((group.to_owned(), path.to_owned(), index, focus != open))
}
/// The open keys of a group's rows under `root`, in the order they read.
fn change_keys(root: &gtk::Widget, group: &str) -> Vec<gtk::Widget> {
    fn walk(widget: &gtk::Widget, prefix: &str, found: &mut Vec<gtk::Widget>) {
        if widget.widget_name().starts_with(prefix) {
            found.push(widget.clone());
            return;
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            walk(&next, prefix, found);
            child = next.next_sibling();
        }
    }
    let mut found = Vec::new();
    walk(root, &format!("git-change:{group}:"), &mut found);
    found
}
/// After a rebuild, the keyboard goes back to the same row, or, when staging or unstaging took
/// it out of its group, the one now in its place (else the group's last, else the same file
/// in the other group), so a person can stage one file after another from the keys.
fn refocus_change(git: &gtk::Box, (group, path, index, on_key): ChangeFocus) {
    let keys = change_keys(git.upcast_ref(), &group);
    let same = |key: &&gtk::Widget| {
        key.widget_name().strip_prefix("git-change:").and_then(|rest| rest.split_once(':')).is_some_and(|(_, p)| p == path)
    };
    let open = keys
        .iter()
        .find(same)
        .or_else(|| keys.get(index))
        .or_else(|| keys.last())
        .cloned()
        .or_else(|| {
            let other = if group == "staged" { "worktree" } else { "staged" };
            change_keys(git.upcast_ref(), other).iter().find(same).cloned()
        });
    let target = open.and_then(|open| if on_key { open.next_sibling() } else { Some(open) });
    if let Some(target) = target.filter(|target| target.is_mapped()) {
        target.grab_focus();
    }
}

/// Every scroller's position under `root`, in tree order.
fn scroll_positions(root: &gtk::Widget) -> Vec<f64> {
    let mut positions = Vec::new();
    fn walk(widget: &gtk::Widget, positions: &mut Vec<f64>) {
        if let Some(scroller) = widget.downcast_ref::<gtk::ScrolledWindow>() {
            positions.push(scroller.vadjustment().value());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            walk(&next, positions);
            child = next.next_sibling();
        }
    }
    walk(root, &mut positions);
    positions
}

/// Put the scrollers a rebuild replaced back where they were, once each has its new size.
fn restore_scrolls(root: &gtk::Widget, saved: Vec<f64>) {
    let mut scrollers = Vec::new();
    fn walk(widget: &gtk::Widget, scrollers: &mut Vec<gtk::Adjustment>) {
        if let Some(scroller) = widget.downcast_ref::<gtk::ScrolledWindow>() {
            scrollers.push(scroller.vadjustment());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            walk(&next, scrollers);
            child = next.next_sibling();
        }
    }
    walk(root, &mut scrollers);
    if scrollers.len() != saved.len() {
        return;
    }
    for (adjustment, value) in scrollers.into_iter().zip(saved) {
        if value <= 0. {
            continue;
        }
        let done = Cell::new(false);
        adjustment.connect_changed(move |a| {
            if !done.get() && a.upper() - a.page_size() >= value {
                done.set(true);
                a.set_value(value);
            }
        });
    }
}

impl Editor {
    /// The panel's view in the shape [`VIEW_KEY`] keeps it.
    fn git_view(&self) -> Value {
        let mut folded: Vec<&str> = FOLDED.with(|folded| folded.borrow().iter().copied().collect());
        folded.sort_unstable();
        json!({"graph": self.git_graph.get(), "history_split": self.git_split_position.get(), "folded": folded})
    }
    /// Takes up the view the last run left, once a run, before the panel is first drawn.
    async fn load_git_view(&self, ui: &Rc<Ui>) {
        if SAVED_VIEW.with(|saved| saved.borrow().is_some()) {
            return;
        }
        // A failed read leaves the view unread: the defaults show and nothing is written.
        let Ok(saved) = ui.call("settings.get", json!({"path": VIEW_KEY})).await else { return };
        let saved = &saved["value"];
        if let Some(graph) = saved["graph"].as_bool() {
            self.git_graph.set(graph);
        }
        if let Some(split) = saved["history_split"].as_i64().and_then(|at| i32::try_from(at).ok()).filter(|at| *at > 0) {
            self.git_split_position.set(split);
        }
        if let Some(folded) = saved["folded"].as_array() {
            let ids = SECTIONS.into_iter().filter(|id| folded.iter().any(|f| f.as_str() == Some(*id))).collect();
            FOLDED.with(|slot| *slot.borrow_mut() = ids);
        }
        SAVED_VIEW.with(|slot| *slot.borrow_mut() = Some(self.git_view()));
    }
    /// Writes the view a moment after it last changed (a dragged split changes it on every
    /// frame), once the saved one has been read.
    fn keep_git_view(&self, ui: &Rc<Ui>) {
        let view = self.git_view();
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
        let weak = Rc::downgrade(ui);
        let timer = glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
            VIEW_TIMER.with(|timer| timer.borrow_mut().take());
            let (Some(ui), Some(value)) = (weak.upgrade(), SAVED_VIEW.with(|saved| saved.borrow().clone())) else { return };
            glib::spawn_future_local(async move {
                let _ = ui.call("settings.set", json!({"path": VIEW_KEY, "value": value})).await;
            });
        });
        VIEW_TIMER.with(|slot| *slot.borrow_mut() = Some(timer));
    }
    /// Folding `section` is part of the view [`VIEW_KEY`] keeps.
    fn keeps_fold(self: &Rc<Self>, ui: &Rc<Ui>, section: &Section) {
        let (ed, weak) = (Rc::downgrade(self), Rc::downgrade(ui));
        section.toggle.connect_clicked(move |_| {
            if let (Some(ed), Some(ui)) = (ed.upgrade(), weak.upgrade()) {
                ed.keep_git_view(&ui);
            }
        });
    }

    pub(super) fn refresh_git(self: &Rc<Self>, ui: &Rc<Ui>) {
        if ui.project.get() == 0 {
            return;
        }
        // Another checkout's rows must not stay clickable while its status loads: their keys
        // act on whatever checkout is selected when they are pressed (RA-452).
        let scope = (ui.project.get(), self.worktree.borrow().clone());
        if *self.git_scope.borrow() != scope {
            clear(&self.git);
            self.git_revision.set(self.git_revision.get() + 1);
            // The message and branch name being typed belong to the checkout they were typed
            // for: kept for the session, and that checkout's come back.
            let fields = [commit_text(&self.commit_message), self.branch_name.text().into(), self.branch_start.text().into()];
            let old = std::mem::replace(&mut *self.git_scope.borrow_mut(), scope.clone());
            let [message, name, start] = {
                let mut drafts = self.git_drafts.borrow_mut();
                if fields.iter().any(|field| !field.is_empty()) {
                    drafts.insert(old, fields);
                } else {
                    drafts.remove(&old);
                }
                drafts.remove(&scope).unwrap_or_default()
            };
            self.commit_message.buffer().set_text(&message);
            self.branch_name.set_text(&name);
            self.branch_start.set_text(&start);
            self.git_history.set(HISTORY_PAGE);
            *self.tree_badges.borrow_mut() = Default::default();
        }
        // A status is on its way either way: this one, or the rerun queued behind one.
        self.status_requests.set(self.status_requests.get() + 1);
        if self.git_refresh_pending.replace(true) {
            self.git_refresh_dirty.set(true);
            return;
        }
        self.git_revision.set(self.git_revision.get() + 1);
        let revision = self.git_revision.get();
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let payload = self.payload(ui, json!({}));
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let _pending = GitRefreshGuard(e.clone(), ui.clone());
            let result = ui.call("git.status", payload.clone()).await;
            if !e.matches(&ui, project, &worktree) || revision != e.git_revision.get() {
                return;
            }
            let status = match result {
                Ok(v) => v,
                Err(err) => {
                    ui.show_error(&readable(&err));
                    return;
                }
            };
            let files = rows(&status, "files");
            e.note_changes(&files);
            e.note_badges(&files);
            // Hidden, the panel needs only the status that tints the explorer; it is rebuilt
            // when it is next shown.
            if !e.git.is_mapped() {
                e.git_stale.set(true);
                return;
            }
            // Pull requests come on their own: the panel draws without waiting for GitHub.
            e.request_prs(&ui, project);
            e.load_git_view(&ui).await;
            let results = tokio::join!(
                ui.call("git.branches", payload.clone()),
                ui.call("git.log", e.payload(&ui, json!({"limit":e.git_history.get()}))),
                ui.call("integration.list", json!({"project_id":project}))
            );
            if !e.matches(&ui, project, &worktree) || revision != e.git_revision.get() {
                return;
            }
            // Nothing the panel draws has changed: keep it, with its focus, popovers and scroll.
            let signature = git_signature(&ui, &e, &status, &results);
            if e.git.first_child().is_some() && e.git_signature.get() == signature {
                return;
            }
            // A rebuild would close an open branch picker or commit menu under the pointer:
            // wait for it to close.
            if let Some(popover) = crate::app::open_popover(e.git.upcast_ref()) {
                if !e.git_deferred.replace(true) {
                    let (ed, weak) = (Rc::downgrade(&e), Rc::downgrade(&ui));
                    popover.connect_closed(move |_| {
                        if let (Some(ed), Some(ui)) = (ed.upgrade(), weak.upgrade()) {
                            if ed.git_deferred.replace(false) {
                                ed.refresh_git(&ui);
                            }
                        }
                    });
                }
                return;
            }
            e.git_signature.set(signature);
            let typing = e.commit_message.has_focus();
            let focused = focused_change(&e.git);
            let scrolls = scroll_positions(e.git.upcast_ref());
            clear(&e.git);
            // git.status names a detached head "HEAD"; it has no branch to publish or open a PR for.
            let branch_name = Some(text(&status, "branch")).filter(|b| *b != "HEAD").unwrap_or("").to_string();
            let detached = branch_name.is_empty();
            let ahead = status["ahead"].as_i64();
            let behind = status["behind"].as_i64();
            let upstream = status["upstream"].as_str().map(str::to_owned);

            // Header: the branch picker leads, as VS Code's status-bar branch does.
            let header = gtk::Box::new(gtk::Orientation::Horizontal, 2);
            header.add_css_class("code-bar");
            header.add_css_class("git-head");
            let picker = gtk::MenuButton::new();
            picker.set_widget_name("git-branch-picker");
            picker.add_css_class("git-branch-picker");
            picker.set_hexpand(true);
            picker.set_valign(gtk::Align::Center);
            picker.set_tooltip_text(Some("Switch, create or fetch branches"));
            let face = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            face.append(&crate::icons::image("branch", 14));
            let shown = if branch_name.is_empty() { "Detached HEAD" } else { branch_name.as_str() };
            let name = label(shown, "git-branch-name");
            name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            name.set_hexpand(true);
            face.append(&name);
            if let Some(counts) = sync_counts(ahead, behind) {
                face.append(&counts);
            }
            if upstream.is_none() && !branch_name.is_empty() {
                let local = crate::icons::image("cloud", 13);
                local.add_css_class("git-unpublished");
                local.set_tooltip_text(Some("Not published to a remote"));
                face.append(&local);
            }
            face.append(&crate::icons::image("chevron-down", 12));
            picker.set_child(Some(&face));
            picker.update_property(&[gtk::accessible::Property::Label(&format!(
                "Branch {shown}. Switch branch"
            ))]);
            header.append(&picker);
            let fetch = crate::app::icon_button("download", "Fetch from the remote");
            fetch.add_css_class("git-head-key");
            fetch.set_valign(gtk::Align::Center);
            let ed = e.clone();
            let weak = Rc::downgrade(&ui);
            fetch.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ed.git_action(&ui, "git.fetch", json!({}), None);
                }
            });
            header.append(&fetch);
            let refresh = crate::app::icon_button("refresh", "Refresh Git");
            refresh.add_css_class("git-head-key");
            refresh.set_valign(gtk::Align::Center);
            let ed = e.clone();
            let weak = Rc::downgrade(&ui);
            refresh.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ed.pr_refresh.set(true);
                    ed.refresh_git(&ui);
                    ed.refresh_scopes(&ui);
                }
            });
            header.append(&refresh);
            e.git.append(&header);
            let branch_popup = gtk::Popover::new();
            branch_popup.add_css_class("branch-popover");
            branch_popup.set_has_arrow(false);
            let branches_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
            branches_box.set_size_request(340, -1);
            branches_box.append(&label("Loading branches…", "branch-note"));
            branch_popup.set_child(Some(&branches_box));
            picker.set_popover(Some(&branch_popup));

            let panes = gtk::Paned::new(gtk::Orientation::Vertical);
            panes.add_css_class("code-git-split");
            panes.set_vexpand(true);
            panes.set_resize_start_child(true);
            panes.set_resize_end_child(false);
            panes.set_shrink_end_child(false);
            let free = ui.window.height().max(ui.window.default_height()) - 42 - 24 - 36;
            let kept = e.git_split_position.get();
            panes.set_position(if kept > 0 {
                kept
            } else if folded("history") {
                free - 34
            } else {
                (free - 280).max(120)
            });
            let (split_position, weak) = (Rc::downgrade(&e), Rc::downgrade(&ui));
            panes.connect_position_notify(move |panes| {
                if let (Some(ed), Some(ui)) = (split_position.upgrade(), weak.upgrade()) {
                    ed.git_split_position.set(panes.position());
                    ed.keep_git_view(&ui);
                }
            });
            let upper = gtk::Box::new(gtk::Orientation::Vertical, 0);
            upper.add_css_class("code-git-upper");
            let upper_scroll = scrolled(&upper);
            // Rows cut their names to the panel's width; nothing scrolls sideways.
            upper_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
            panes.set_start_child(Some(&upper_scroll));
            e.git.append(&panes);

            // The engine lists at most 5 000 changes: past that, the counts and the "all" keys
            // speak for the whole tree, not the list.
            let truncated = status["truncated"] == true;
            let total = status["total"].as_u64().map_or(files.len(), |n| n as usize).max(files.len());
            if truncated {
                let cut = label(&format!("Showing the first {} of {total} changes.", files.len()), "dim");
                cut.set_xalign(0.);
                cut.set_wrap(true);
                cut.set_margin_start(12);
                cut.set_margin_top(6);
                upper.append(&cut);
            }
            let conflicts: Vec<Value> = files.iter().filter(|f| is_conflict(f)).cloned().collect();
            let staged: Vec<Value> =
                files.iter().filter(|f| !is_conflict(f) && is_staged(f)).cloned().collect();
            let unstaged: Vec<Value> =
                files.iter().filter(|f| !is_conflict(f) && is_unstaged(f)).cloned().collect();

            // Commit box: message, then a split Commit key and Suggest.
            let message = e.commit_message.clone();
            if message.widget_name() != "commit-message" {
                message.set_widget_name("commit-message");
                let keys = gtk::EventControllerKey::new();
                let weak = Rc::downgrade(&ui);
                let editor = Rc::downgrade(&e);
                keys.connect_key_pressed(move |_, key, _, modifiers| {
                    if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter)
                        && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                    {
                        if let (Some(ui), Some(editor)) = (weak.upgrade(), editor.upgrade()) {
                            let message = commit_text(&editor.commit_message).trim().to_string();
                            if editor.git.has_css_class("has-conflicts") {
                                ui.show_error(UNRESOLVED);
                            } else if !editor.git.has_css_class("has-changes") {
                                // As the Commit key says it.
                                ui.show_error("No changes to commit.");
                            } else if !message.is_empty() {
                                // Like VS Code's smart commit: staged changes if any, else all.
                                let all = !editor.git.has_css_class("has-staged");
                                editor.git_action(
                                    &ui,
                                    "git.commit",
                                    json!({"message":message,"all":all}),
                                    None,
                                );
                            } else {
                                // As the Commit key says it.
                                ui.show_error("Enter a commit message first.");
                                editor.commit_message.grab_focus();
                            }
                        }
                        return glib::Propagation::Stop;
                    }
                    glib::Propagation::Proceed
                });
                message.add_controller(keys);
            }
            if staged.is_empty() {
                e.git.remove_css_class("has-staged");
            } else {
                e.git.add_css_class("has-staged");
            }
            if files.is_empty() {
                e.git.remove_css_class("has-changes");
            } else {
                e.git.add_css_class("has-changes");
            }
            // `all` stages with `git add -A`, which would record the conflict markers as resolved;
            // git itself refuses a commit while the index is unmerged. Resolve first.
            let resolving = !conflicts.is_empty();
            if resolving {
                e.git.add_css_class("has-conflicts");
            } else {
                e.git.remove_css_class("has-conflicts");
            }
            detach(&message);
            message.set_wrap_mode(gtk::WrapMode::WordChar);
            message.set_size_request(-1, 58);
            message.add_css_class("code-commit-message");
            let hint = if branch_name.is_empty() {
                String::from("Message (Ctrl+Enter to commit)")
            } else {
                format!("Message (Ctrl+Enter to commit on {branch_name})")
            };
            message.update_property(&[gtk::accessible::Property::Label(&hint)]);
            let commit_form = gtk::Box::new(gtk::Orientation::Vertical, 6);
            commit_form.add_css_class("code-commit-form");
            commit_form.add_css_class("scm-commit");
            let input = gtk::Overlay::new();
            input.set_child(Some(&message));
            let placeholder = label(&hint, "code-commit-placeholder");
            placeholder.set_halign(gtk::Align::Start);
            placeholder.set_valign(gtk::Align::Start);
            placeholder.set_wrap(true);
            placeholder.set_can_target(false);
            input.add_overlay(&placeholder);
            message
                .buffer()
                .bind_property("text", &placeholder, "visible")
                .transform_to(|_, value: String| Some(value.is_empty()))
                .sync_create()
                .build();
            commit_form.append(&input);
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let split = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            split.add_css_class("linked");
            split.add_css_class("scm-commit-split");
            split.set_hexpand(true);
            let smart_all = staged.is_empty();
            let commit = button(
                &if files.is_empty() {
                    String::from("Commit")
                } else if smart_all {
                    format!("Commit all {total}")
                } else {
                    format!("Commit {} staged", staged.len())
                },
                "primary",
            );
            commit.set_widget_name("git-commit");
            commit.add_css_class("scm-commit-key");
            commit.set_hexpand(true);
            commit.set_sensitive(!files.is_empty() && !resolving);
            commit.set_tooltip_text(Some(if resolving {
                UNRESOLVED
            } else if files.is_empty() {
                "No changes to commit"
            } else if smart_all {
                "Nothing is staged, so every change is committed (Ctrl+Enter)"
            } else {
                "Commit the staged changes (Ctrl+Enter)"
            }));
            let commit_more = gtk::MenuButton::new();
            commit_more.add_css_class("scm-commit-more");
            commit_more.set_tooltip_text(Some("More commit options"));
            commit_more.set_child(Some(&crate::icons::image("chevron-down", 12)));
            commit_more.set_sensitive(!files.is_empty() && !resolving);
            let more = gtk::Popover::new();
            more.add_css_class("scm-menu");
            more.set_has_arrow(false);
            let more_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
            more.set_child(Some(&more_box));
            commit_more.set_popover(Some(&more));
            let mut commit_keys = vec![(commit.clone(), smart_all)];
            for (title, all, enabled) in [
                ("Commit staged", false, !staged.is_empty()),
                ("Commit all changes", true, !files.is_empty()),
            ] {
                let key = button(title, "quiet");
                key.add_css_class("scm-menu-item");
                key.set_sensitive(enabled);
                let pop = more.downgrade();
                key.connect_clicked(move |_| {
                    if let Some(pop) = pop.upgrade() {
                        pop.popdown();
                    }
                });
                more_box.append(&key);
                commit_keys.push((key, all));
            }
            for (key, all) in commit_keys {
                let ed = e.clone();
                let weak = Rc::downgrade(&ui);
                let message = message.clone();
                key.connect_clicked(move |_| {
                    if let Some(ui) = weak.upgrade() {
                        let content = commit_text(&message).trim().to_string();
                        if content.is_empty() {
                            ui.show_error("Enter a commit message first.");
                            message.grab_focus();
                            return;
                        }
                        ed.git_action(
                            &ui,
                            "git.commit",
                            json!({"message":content,"all":all}),
                            None,
                        );
                    }
                });
            }
            split.append(&commit);
            split.append(&commit_more);
            actions.append(&split);
            let suggest = button("Suggest", "quiet");
            suggest.add_css_class("scm-suggest");
            suggest.set_tooltip_text(Some("Draft a message from the changes"));
            suggest.set_sensitive(!files.is_empty());
            let ed = e.clone();
            let weak = Rc::downgrade(&ui);
            let field = message.clone();
            suggest.connect_clicked(move |_| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let ed = ed.clone();
                let field = field.clone();
                let original = commit_text(&field);
                let project = ui.project.get();
                let worktree = ed.worktree.borrow().clone();
                glib::spawn_future_local(async move {
                    match ui
                        .call("git.suggest_message", ed.payload(&ui, json!({})))
                        .await
                    {
                        Ok(v)
                            if ed.matches(&ui, project, &worktree)
                                && commit_text(&field) == original =>
                        {
                            // One undo step, so Ctrl+Z brings back what was typed; set_text
                            // cannot be undone.
                            let buffer = field.buffer();
                            let (mut start, mut end) = buffer.bounds();
                            buffer.begin_user_action();
                            buffer.delete(&mut start, &mut end);
                            buffer.insert(&mut start, text(&v, "message"));
                            buffer.end_user_action();
                        }
                        Ok(_) => {}
                        Err(err) => ui.show_error(&readable(&err)),
                    }
                });
            });
            actions.append(&suggest);
            commit_form.append(&actions);
            upper.append(&commit_form);

            if files.is_empty() {
                let clean = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                clean.add_css_class("scm-clean");
                let mark = crate::icons::image("check", 14);
                mark.set_valign(gtk::Align::Start);
                clean.append(&mark);
                let copy = gtk::Box::new(gtk::Orientation::Vertical, 2);
                copy.append(&label("No changes", "scm-clean-title"));
                let detail = label(
                    &format!("The working tree matches the last commit on {shown}."),
                    "scm-clean-hint",
                );
                detail.set_wrap(true);
                copy.append(&detail);
                clean.append(&copy);
                upper.append(&clean);
            }
            for (id, title, group, staged_group) in [
                ("conflicts", "Merge changes", &conflicts, false),
                ("staged", "Staged changes", &staged, true),
                ("changes", "Changes", &unstaged, false),
            ] {
                if group.is_empty() {
                    continue;
                }
                let group_section = section(id, title, Some(group.len()));
                e.keeps_fold(&ui, &group_section);
                if id == "changes" {
                    group_section.toggle.set_tooltip_text(Some(&change_summary(group)));
                }
                // A cut list's "all" is the whole tree (`.`), unless a conflict is in it: staging
                // that would mark it resolved unseen.
                let paths: Vec<String> = if truncated && id != "conflicts" && conflicts.is_empty() {
                    vec![".".to_string()]
                } else {
                    group.iter().map(|file| text(file, "path").to_owned()).collect()
                };
                let (icon, caption, op) = if staged_group {
                    ("minus", "Unstage all changes", "git.unstage")
                } else if id == "conflicts" {
                    ("check", "Mark all as resolved", "git.stage")
                } else {
                    ("plus", "Stage all changes", "git.stage")
                };
                let all = crate::app::icon_button(icon, caption);
                all.add_css_class("scm-head-key");
                all.set_widget_name(&format!("git-{id}-all"));
                let ed = e.clone();
                let weak = Rc::downgrade(&ui);
                let resolve = id == "conflicts";
                all.connect_clicked(move |_| {
                    if let Some(ui) = weak.upgrade() {
                        if resolve {
                            ed.mark_resolved(&ui, paths.clone());
                        } else {
                            ed.git_action(&ui, op, json!({"paths":paths}), None);
                        }
                    }
                });
                group_section.actions.append(&all);
                // Each row is several widgets; an un-ignored build directory would be thousands.
                for file in group.iter().take(CHANGE_ROWS) {
                    group_section.body.append(&e.change_row(&ui, file, staged_group));
                }
                if let Some(more) = more_files(group.len()) {
                    group_section.body.append(&more);
                }
                upper.append(&group_section.root);
            }

            // Remote: push and the pull request, one compact row each.
            let remote = section("remote", "Remote", None);
            e.keeps_fold(&ui, &remote);
            let can_push = !detached && (upstream.is_none() || (ahead.is_some_and(|n| n > 0) && behind == Some(0)));
            let push_state = match (&upstream, ahead, behind) {
                _ if detached => String::from("Check out a branch to push or open a pull request"),
                (None, _, _) => String::from("This branch is not on a remote yet"),
                (Some(up), Some(0), Some(0)) => format!("Up to date with {up}"),
                (Some(up), Some(a), Some(b)) => format!(
                    "{a} to push · {b} to pull · {up}"
                ),
                (Some(up), _, _) => format!("{up} · fetch to compare"),
            };
            let push_row = remote_row(
                &remote.body,
                "cloud",
                upstream.as_deref().unwrap_or(if detached { "Detached HEAD" } else { "Not published" }),
                &push_state,
            );
            let push = button(
                &if detached {
                    String::from("No branch")
                } else if can_push {
                    if let Some(ahead) = ahead.filter(|n| *n > 0) {
                        format!("Push {ahead}")
                    } else {
                        String::from("Publish")
                    }
                } else if behind.is_some_and(|n| n > 0) {
                    String::from("Pull first")
                } else if ahead.is_none() || behind.is_none() {
                    String::from("Unknown")
                } else {
                    String::from("Synced")
                },
                if can_push { "primary" } else { "quiet" },
            );
            push.add_css_class("small-key");
            push.set_valign(gtk::Align::Center);
            push.set_sensitive(can_push);
            // Each state says what it means, and what to do when Relay cannot do it.
            push.set_tooltip_text(Some(&if detached {
                String::from("Check out a branch to push it")
            } else if can_push {
                match (&upstream, ahead.filter(|n| *n > 0)) {
                    (Some(up), Some(ahead)) => format!("Push {ahead} commit{} to {up}", if ahead == 1 { "" } else { "s" }),
                    _ => String::from("Publish this branch to the remote"),
                }
            } else if behind.is_some_and(|n| n > 0) {
                String::from("The remote has commits this branch lacks. Pull them in a terminal (git pull), then push.")
            } else if ahead.is_none() || behind.is_none() {
                String::from("Fetch to compare with the remote")
            } else {
                String::from("Nothing to push")
            }));
            let ed = e.clone();
            let weak = Rc::downgrade(&ui);
            // Asked on the key itself, as Discard is: a second click pushes.
            crate::app::confirm_inline(&push, "Push?", move |_| {
                if let Some(ui) = weak.upgrade() {
                    ed.git_action(&ui, "git.push", json!({}), None);
                }
            });
            push_row.0.append(&push);
            let pr_row = remote_row(&remote.body, "merge", "Pull request", "Checking GitHub…");
            let pr = button("Open PR", "quiet");
            pr.add_css_class("small-key");
            pr.set_valign(gtk::Align::Center);
            pr.set_sensitive(false);
            pr_row.0.append(&pr);
            pr_row.0.set_visible(!detached);
            upper.append(&remote.root);

            // History sits in its own pane below, resizable and foldable.
            let history = gtk::Box::new(gtk::Orientation::Vertical, 0);
            history.add_css_class("code-history-pane");
            let history_section = section("history", "History", None);
            e.keeps_fold(&ui, &history_section);
            history_section.root.add_css_class("scm-history");
            let graph_key = gtk::ToggleButton::with_label("Graph");
            graph_key.add_css_class("code-graph-toggle");
            graph_key.set_active(e.git_graph.get());
            let (graph_state, weak) = (Rc::downgrade(&e), Rc::downgrade(&ui));
            graph_key.connect_toggled(move |key| {
                if let (Some(ed), Some(ui)) = (graph_state.upgrade(), weak.upgrade()) {
                    ed.git_graph.set(key.is_active());
                    ed.keep_git_view(&ui);
                }
            });
            history_section.actions.append(&graph_key);
            let history_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
            history_box.add_css_class("code-history");
            let history_scroll = scrolled(&history_box);
            history_scroll.set_min_content_height(120);
            history_section.body.append(&history_scroll);
            history_section.body.set_vexpand(true);
            history_section.root.set_vexpand(true);
            history.append(&history_section.root);
            panes.set_end_child(Some(&history));
            let fold_panes = panes.downgrade();
            let reopen = Rc::new(Cell::new(0));
            history_section.toggle.connect_clicked(move |_| {
                let Some(panes) = fold_panes.upgrade() else {
                    return;
                };
                if folded("history") {
                    reopen.set(panes.position());
                    panes.set_position(panes.height() - 34);
                } else {
                    let saved = reopen.get();
                    panes.set_position(if saved > 0 { saved } else { (panes.height() - 280).max(120) });
                }
            });

            let merged = results.0.as_ref().ok().and_then(|value| {
                rows(value, "branches")
                    .into_iter()
                    .find(|branch| text(branch, "name") == branch_name)
                    .map(|branch| branch["merged"] == true)
            });
            let url: Rc<RefCell<Option<String>>> = Rc::default();
            let target = Rc::new(PrRow {
                project,
                row: pr_row.0.clone(),
                copy: pr_row.1.clone(),
                title: pr_row.2.clone(),
                hint: pr_row.3.clone(),
                key: pr.clone(),
                branch: branch_name.clone(),
                upstream: upstream.is_some(),
                ahead,
                behind,
                merged,
                url: url.clone(),
            });
            // The last answer for this project fills the row now; a lookup in flight
            // refills it when GitHub answers.
            if let Some((_, answer)) = e.prs.borrow().answer.as_ref().filter(|(p, _)| *p == project) {
                target.fill(answer);
            }
            e.prs.borrow_mut().row = Some(target);
            let ed = e.clone();
            let weak = Rc::downgrade(&ui);
            // Opening one that exists needs no question; publishing one is asked on the key.
            let unpublished = url.clone();
            crate::app::confirm_inline_if(&pr, "Open PR?", move || unpublished.borrow().is_none(), move |_| {
                if let Some(ui) = weak.upgrade() {
                    if let Some(url) = url.borrow().as_deref() {
                        let uri = gtk::UriLauncher::new(url);
                        let shown = ui.clone();
                        uri.launch(Some(&ui.window), gtk::gio::Cancellable::NONE, move |result| {
                            match result {
                                Err(error) if !error.matches(gtk::DialogError::Dismissed) => {
                                    shown.show_error(&format!("Could not open the pull request: {}", error.message()))
                                }
                                _ => {}
                            }
                        });
                    } else {
                        ed.git_action(&ui, "git.pr.open", json!({}), None);
                    }
                }
            });
            let live: Vec<Value> = ui
                .sessions
                .borrow()
                .iter()
                .filter(|s| text(s, "state") != "closed")
                .cloned()
                .collect();
            let merge_section = section("merge", "Merge test", Some(live.len()));
            e.keeps_fold(&ui, &merge_section);
            let merge = merge_section.body.clone();
            upper.append(&merge_section.root);
            let hint = label(
                "Safely test agent branches together in a disposable worktree. This does not merge them into your current branch.",
                "code-git-hint",
            );
            hint.set_wrap(true);
            merge.append(&hint);
            let mut picks = Vec::new();
            for session in &live {
                let name = text(session, "name").to_string();
                let pick = gtk::CheckButton::new();
                pick.add_css_class("code-merge-pick");
                let copy = gtk::Box::new(gtk::Orientation::Vertical, 2);
                // A long agent or branch name is cut in the middle, not pushed past the panel.
                for (words, class) in [(name.as_str(), "body"), (text(session, "branch"), "mono")] {
                    let line = label(words, class);
                    line.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                    line.set_xalign(0.);
                    copy.append(&line);
                }
                pick.set_child(Some(&copy));
                merge.append(&pick);
                picks.push((name, pick));
            }
            let test = button("Select at least 2 branches", "quiet");
            test.add_css_class("code-merge-action");
            test.set_sensitive(false);
            let selected_count = Rc::new(Cell::new(0_usize));
            for (pick_name, pick) in &picks {
                let count = selected_count.clone();
                // Weak: Test's own handler owns every pick, so a strong Test here is a cycle.
                let test = test.downgrade();
                let (remember, name) = (Rc::downgrade(&e), pick_name.clone());
                pick.connect_toggled(move |pick| {
                    if let Some(ed) = remember.upgrade() {
                        let mut picked = ed.git_merge_picks.borrow_mut();
                        if pick.is_active() {
                            picked.insert(name.clone());
                        } else {
                            picked.remove(&name);
                        }
                    }
                    let Some(test) = test.upgrade() else { return };
                    let total = if pick.is_active() {
                        count.get() + 1
                    } else {
                        count.get().saturating_sub(1)
                    };
                    count.set(total);
                    test.set_sensitive(total >= 2);
                    test.set_label(&if total >= 2 {
                        format!("Test merge + build · {total}")
                    } else {
                        String::from("Select at least 2 branches")
                    });
                });
            }
            merge.append(&test);
            // Ticks survive a refresh; a session that has gone drops out of the set.
            let picked = e.git_merge_picks.borrow().clone();
            e.git_merge_picks.borrow_mut().retain(|name| picks.iter().any(|(n, _)| n == name));
            for (name, pick) in &picks {
                if picked.contains(name) {
                    pick.set_active(true);
                }
            }
            let weak = Rc::downgrade(&ui);
            let ed = e.clone();
            test.connect_clicked(move |key| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let selected: Vec<_> = picks
                    .iter()
                    .filter(|(_, p)| p.is_active())
                    .map(|(n, _)| n.clone())
                    .collect();
                if selected.len() < 2 {
                    ui.show_error("Select at least 2 branches to test together.");
                    return;
                }
                key.set_sensitive(false);
                let key = key.clone();
                let ed = ed.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = ui
                        .call(
                            "integration.request",
                            json!({"project_id":project,"sessions":selected,"build":true}),
                        )
                        .await
                    {
                        ui.show_error(&readable(&e));
                    }
                    key.set_sensitive(true);
                    if ui.project.get() == project {
                        ed.refresh_git(&ui);
                    }
                });
            });
            match results.2 {
                Ok(v) => {
                    for run in rows(&v, "integrations").iter().filter(|run| run["state"] != "discarded").take(3) {
                        let line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                        let l = label(
                            &format!(
                                "#{} · {}{}",
                                run["id"],
                                text(run, "state"),
                                run["conflict"]
                                    .as_array()
                                    .map(|v| format!(
                                        " · {}",
                                        v.iter()
                                            .filter_map(Value::as_str)
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    ))
                                    .unwrap_or_default()
                            ),
                            "code-git-hint",
                        );
                        l.set_wrap(true);
                        l.set_hexpand(true);
                        line.append(&l);
                        // A finished integration keeps its checkout and build output until it
                        // is discarded (D38); this is where a person does that.
                        if matches!(text(run, "state"), "passed" | "failed" | "conflict") && !run["worktree"].is_null() {
                            let id = run["id"].as_i64().unwrap_or(0);
                            let discard = crate::app::icon_button("trash", &format!("Discard integration #{id}: its checkout, build output and branch"));
                            discard.set_valign(gtk::Align::Center);
                            let weak = Rc::downgrade(&ui);
                            let ed = e.clone();
                            // It throws away a checkout and a branch: asked once in place.
                            crate::app::confirm_inline(&discard, "Discard it?", move |key| {
                                let Some(ui) = weak.upgrade() else { return };
                                key.set_sensitive(false);
                                let key = key.clone();
                                let ed = ed.clone();
                                glib::spawn_future_local(async move {
                                    if let Err(err) = ui.call("integration.discard", json!({"integration_id":id})).await {
                                        ui.show_error(&readable(&err));
                                        key.set_sensitive(true);
                                    }
                                    if ui.project.get() == project {
                                        ed.refresh_git(&ui);
                                    }
                                });
                            });
                            line.append(&discard);
                        }
                        merge.append(&line);
                    }
                }
                Err(err) => merge.append(&label(&readable(&err), "code-git-hint")),
            }
            match results.0 {
                Ok(v) => e.render_branch_picker(&ui, &branches_box, &branch_popup, &v),
                Err(err) => {
                    clear(&branches_box);
                    branches_box.append(&label(&readable(&err), "branch-note"));
                }
            }
            match results.1 {
                Ok(v) => {
                    let commits = rows(&v, "commits");
                    // A full page is the limit asked for, not the whole history.
                    let more = commits.len() as u32 >= e.git_history.get();
                    if let Some(face) = history_section.toggle.child().and_downcast::<gtk::Box>() {
                        let count = label(&format!("{}{}", commits.len(), if more { "+" } else { "" }), "scm-count");
                        count.set_valign(gtk::Align::Center);
                        face.append(&count);
                    }
                    let graph = commit_graph(&commits);
                    let graph_width = graph
                        .iter()
                        .flat_map(|row| {
                            row.links
                                .iter()
                                .map(|(_, lane)| *lane)
                                .chain(std::iter::once(row.lane))
                        })
                        .max()
                        .unwrap_or(0)
                        + 1;
                    let mut graphs = Vec::new();
                    for (commit, graph_row) in commits.into_iter().zip(graph) {
                        let sha = text(&commit, "sha").to_string();
                        let b = button("", "file");
                        b.add_css_class("code-commit");
                        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                        let graph = graph_widget(graph_row, graph_width);
                        graphs.push(graph.clone());
                        row.append(&graph);
                        let copy = gtk::Box::new(gtk::Orientation::Vertical, 2);
                        copy.set_hexpand(true);
                        copy.set_valign(gtk::Align::Center);
                        let subject = label(text(&commit, "subject"), "code-commit-subject");
                        subject.set_ellipsize(gtk::pango::EllipsizeMode::End);
                        copy.append(&subject);
                        let meta = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                        meta.append(&label(&sha[..sha.len().min(7)], "code-commit-sha"));
                        let author = label(text(&commit, "author"), "code-commit-author");
                        author.set_ellipsize(gtk::pango::EllipsizeMode::End);
                        meta.append(&author);
                        copy.append(&meta);
                        row.append(&copy);
                        b.set_child(Some(&row));
                        // "3 h ago", as the commit's own panel says it.
                        let at = text(&commit, "at");
                        b.set_tooltip_text(Some(&format!(
                            "{}\n{}\n{}",
                            text(&commit, "author"),
                            crate::relative::ago(at, crate::relative::Form::Long).unwrap_or_else(|| at.to_string()),
                            sha
                        )));
                        let ed = e.clone();
                        let weak = Rc::downgrade(&ui);
                        b.connect_clicked(move |_| {
                            if let Some(ui) = weak.upgrade() {
                                ed.show_commit(&ui, sha.clone());
                            }
                        });
                        history_box.append(&b);
                    }
                    if more && e.git_history.get() < HISTORY_MOST {
                        let next = button(&format!("Show {HISTORY_PAGE} more"), "quiet");
                        next.add_css_class("code-history-more");
                        next.set_widget_name("git-history-more");
                        let (ed, weak) = (Rc::downgrade(&e), Rc::downgrade(&ui));
                        next.connect_clicked(move |next| {
                            if let (Some(ed), Some(ui)) = (ed.upgrade(), weak.upgrade()) {
                                next.set_sensitive(false);
                                ed.git_history.set((ed.git_history.get() + HISTORY_PAGE).min(HISTORY_MOST));
                                ed.refresh_git(&ui);
                            }
                        });
                        history_box.append(&next);
                    }
                    for graph in &graphs {
                        graph.set_visible(graph_key.is_active());
                    }
                    graph_key.connect_toggled(move |key| {
                        for graph in &graphs {
                            graph.set_visible(key.is_active());
                        }
                    });
                }
                Err(err) => history_box.append(&label(&readable(&err), "code-git-hint")),
            }
            restore_scrolls(e.git.upcast_ref(), scrolls);
            if typing {
                e.commit_message.grab_focus();
            } else if let Some(focused) = focused {
                refocus_change(&e.git, focused);
            }
        });
    }

    /// One VS Code Source Control row: type icon, name, dim folder, stage key, status letter.
    /// Ask for the project's pull requests, unless a lookup is already in flight, and fill the
    /// panel's row with the answer when it comes. The engine keeps a listing (or a failure) for
    /// a minute; Refresh asks GitHub again.
    fn request_prs(self: &Rc<Self>, ui: &Rc<Ui>, project: i64) {
        let refresh = self.pr_refresh.replace(false);
        {
            let mut prs = self.prs.borrow_mut();
            if prs.pending == Some(project) {
                prs.again |= refresh;
                return;
            }
            prs.pending = Some(project);
        }
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let answer = ui
                .call("git.pr.list", json!({"project_id":project,"refresh":refresh}))
                .await
                .map_err(|_| ());
            let row = {
                let mut prs = e.prs.borrow_mut();
                if prs.pending == Some(project) {
                    prs.pending = None;
                }
                // An answer for a project left behind must not stand in for the current one's.
                if ui.project.get() == project {
                    prs.answer = Some((project, answer.clone()));
                }
                prs.row.clone()
            };
            if let Some(row) = row.filter(|row| row.project == project && row.row.is_ancestor(&e.git)) {
                row.fill(&answer);
            }
            let again = std::mem::take(&mut e.prs.borrow_mut().again);
            if again && ui.project.get() == project {
                e.pr_refresh.set(true);
                e.request_prs(&ui, project);
            }
        });
    }

    fn change_row(self: &Rc<Self>, ui: &Rc<Ui>, file: &Value, staged: bool) -> gtk::Box {
        let path = text(file, "path").to_string();
        let code = if is_conflict(file) {
            "U"
        } else if staged {
            text(file, "index")
        } else {
            text(file, "worktree")
        };
        let status = project_files::status_letter(code);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.add_css_class("scm-row");
        let open = button("", "scm-open");
        open.set_hexpand(true);
        open.set_widget_name(&format!("git-change:{}:{path}", if staged { "staged" } else { "worktree" }));
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        content.append(&project_files::file_image(&path, 14));
        let (directory, name) = split_path(&path);
        let title = label(name, "scm-name");
        if let Some((_, class, _)) = status {
            title.add_css_class(class);
        }
        title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        content.append(&title);
        let place = label(directory, "scm-dir");
        place.set_ellipsize(gtk::pango::EllipsizeMode::Start);
        place.set_hexpand(true);
        content.append(&place);
        open.set_child(Some(&content));
        let mut tip = format!(
            "{path} · {}{}",
            status.map(|(_, _, word)| word).unwrap_or("Changed"),
            if staged { " · staged" } else { "" }
        );
        if let Some(from) = file["renamed_from"].as_str() {
            tip.push_str(&format!("\nRenamed from {from}"));
        }
        open.set_tooltip_text(Some(&tip));
        let ed = self.clone();
        let weak = Rc::downgrade(ui);
        let diff_path = path.clone();
        // The change this row lists: a staged row's against the index, a rename's against
        // the path it had.
        let renamed = file["renamed_from"].as_str().map(str::to_string);
        open.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let (path, renamed) = (diff_path.clone(), renamed.clone());
                ed.after_unsaved(&ui, move |ed, ui| {
                    ed.open_diff(ui, path, staged, renamed);
                });
            }
        });
        self.bind_image_hover(ui, &open, &path, format!("{code}:{}", self.git_revision.get()), None);
        row.append(&open);
        let (icon, caption, op) = if staged {
            ("minus", "Unstage changes", "git.unstage")
        } else if code == "U" {
            ("check", "Mark as resolved", "git.stage")
        } else {
            ("plus", "Stage changes", "git.stage")
        };
        let action = crate::app::icon_button(icon, caption);
        action.add_css_class("scm-action");
        action.set_valign(gtk::Align::Center);
        let ed = self.clone();
        let weak = Rc::downgrade(ui);
        let resolve = !staged && code == "U";
        action.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                if resolve {
                    ed.mark_resolved(&ui, vec![path.clone()]);
                } else {
                    ed.git_action(&ui, op, json!({"paths":[path]}), None);
                }
            }
        });
        row.append(&action);
        if let Some((letter, class, word)) = status {
            let mark = label(letter, "scm-letter");
            mark.add_css_class(class);
            mark.set_tooltip_text(Some(word));
            mark.set_valign(gtk::Align::Center);
            row.append(&mark);
        }
        row
    }

    /// The checkout holding a branch other than the one shown, as a path `select_checkout` takes.
    pub(super) fn branch_holder(&self, ui: &Ui, branch: &str) -> Option<(String, Option<String>)> {
        let primary = ui
            .projects
            .borrow()
            .iter()
            .find(|p| p["id"] == ui.project.get())
            .map(|p| text(p, "path").to_owned())
            .unwrap_or_default();
        let current = self.worktree.borrow().clone();
        self.worktrees
            .borrow()
            .iter()
            .find(|w| text(w, "branch") == branch)
            .map(|w| {
                let path = text(w, "path");
                let path = if path == primary { String::new() } else { path.to_owned() };
                (path, w["session"].as_str().map(str::to_owned))
            })
            .filter(|(path, _)| *path != current)
    }

    /// The branch picker: search, create, then local and remote branches.
    fn render_branch_picker(
        self: &Rc<Self>,
        ui: &Rc<Ui>,
        body: &gtk::Box,
        popover: &gtk::Popover,
        v: &Value,
    ) {
        clear(body);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Switch to a branch…"));
        search.add_css_class("branch-search");
        body.append(&search);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("branch-list");
        let scroll = scrolled(&list);
        scroll.set_max_content_height(380);
        scroll.set_propagate_natural_height(true);
        scroll.set_vexpand(false);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        body.append(&scroll);
        let mut entries: Vec<(gtk::Widget, String, gtk::Button)> = Vec::new();

        let create_key = branch_key("plus", "Create new branch…");
        create_key.set_widget_name("branch-create");
        create_key.add_css_class("branch-create");
        list.append(&create_key);
        let form = gtk::Box::new(gtk::Orientation::Vertical, 6);
        form.add_css_class("branch-create-form");
        form.set_visible(false);
        let name = self.branch_name.clone();
        detach(&name);
        name.set_placeholder_text(Some("New branch name"));
        form.append(&name);
        let start = self.branch_start.clone();
        detach(&start);
        start.set_placeholder_text(Some("From (optional) · defaults to HEAD"));
        form.append(&start);
        let create = button("Create and switch", "primary");
        create.add_css_class("small-key");
        create.set_halign(gtk::Align::End);
        form.append(&create);
        list.append(&form);
        let reveal = form.clone();
        // Weak: search's Enter handler holds this key, so a strong search here is a cycle that
        // keeps every rebuilt picker, and through `entries` every branch row, alive.
        let query = search.downgrade();
        let focus = name.clone();
        create_key.connect_clicked(move |_| {
            let open = !reveal.is_visible();
            reveal.set_visible(open);
            if open {
                if let Some(query) = query.upgrade().filter(|_| focus.text().is_empty()) {
                    focus.set_text(query.text().trim());
                }
                focus.grab_focus();
            }
        });
        // The entry outlives each picker; only the newest picker answers Enter.
        let submit = create.downgrade();
        let handler = name.connect_activate(move |_| {
            if let Some(submit) = submit.upgrade() {
                submit.emit_clicked();
            }
        });
        if let Some(previous) = BRANCH_ENTER.with(|slot| slot.borrow_mut().replace(handler)) {
            name.disconnect(previous);
        }
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        let pop = popover.downgrade();
        create.connect_clicked(move |_| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if name.text().trim().is_empty() {
                ui.show_error("Enter a branch name.");
                return;
            }
            if let Some(pop) = pop.upgrade() {
                pop.popdown();
            }
            e.git_action(
                &ui,
                "git.branch.create",
                json!({"name":name.text().trim(),"start_point":optional_scope(start.text().trim()),"checkout":true}),
                Some(name.clone()),
            );
        });

        let locals = rows(v, "branches");
        list.append(&label("Local", "branch-heading"));
        for branch in &locals {
            let name = text(branch, "name").to_string();
            let current = branch["current"] == true;
            let holder = if current { None } else { self.branch_holder(ui, &name) };
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            row.add_css_class("branch-row-wrap");
            let key = branch_key(if current { "check" } else { "branch" }, &name);
            if let Some(meta) = key.child().and_downcast::<gtk::Box>() {
                if let Some(counts) = sync_counts(branch["ahead"].as_i64(), branch["behind"].as_i64()) {
                    meta.append(&counts);
                }
                let tag = if let Some((_, session)) = &holder {
                    Some(session.clone().unwrap_or_else(|| String::from("checkout")))
                } else if let Some(session) = branch["session"].as_str() {
                    Some(session.to_owned())
                } else if branch["upstream"].is_null() {
                    Some(String::from("local"))
                } else if branch["merged"] == true && !current {
                    Some(String::from("merged"))
                } else {
                    None
                };
                if let Some(tag) = tag {
                    let tag = label(&tag, "branch-tag");
                    tag.set_valign(gtk::Align::Center);
                    tag.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    tag.set_max_width_chars(14);
                    meta.append(&tag);
                }
            }
            let pop = popover.downgrade();
            let e = self.clone();
            let weak = Rc::downgrade(ui);
            if current {
                key.add_css_class("current");
                key.set_tooltip_text(Some("The branch this checkout is on"));
                key.connect_clicked(move |_| {
                    if let Some(pop) = pop.upgrade() {
                        pop.popdown();
                    }
                });
            } else if let Some((path, session)) = holder {
                key.add_css_class("held-elsewhere");
                key.set_widget_name(&format!("branch-switch-{name}"));
                let place = if path.is_empty() { String::from("the primary checkout") } else { path.clone() };
                key.set_tooltip_text(Some(&match &session {
                    Some(session) => format!("{name} is checked out by the session {session} in {place}. Open that checkout to work on it."),
                    None => format!("{name} is checked out in {place}. Open that checkout to work on it."),
                }));
                key.connect_clicked(move |_| {
                    if let Some(pop) = pop.upgrade() {
                        pop.popdown();
                    }
                    if let Some(ui) = weak.upgrade() {
                        e.select_checkout(&ui, &path);
                    }
                });
            } else {
                key.set_widget_name(&format!("branch-switch-{name}"));
                key.set_tooltip_text(Some(&format!("Switch this checkout to {name}")));
                let target = name.clone();
                key.connect_clicked(move |_| {
                    if let Some(pop) = pop.upgrade() {
                        pop.popdown();
                    }
                    if let Some(ui) = weak.upgrade() {
                        e.git_action(&ui, "git.branch.switch", json!({"name":target}), None);
                    }
                });
            }
            row.append(&key);
            let deletable = branch["merged"] == true
                && branch["session"].is_null()
                && !current
                && !self.worktrees.borrow().iter().any(|w| text(w, "branch") == name)
                && !ui
                    .projects
                    .borrow()
                    .iter()
                    .any(|p| p["id"] == ui.project.get() && text(p, "base_branch") == name);
            if deletable {
                let delete = crate::app::icon_button("trash", "Delete this merged branch");
                delete.add_css_class("branch-delete");
                delete.set_valign(gtk::Align::Center);
                let e = self.clone();
                let weak = Rc::downgrade(ui);
                let pop = popover.downgrade();
                let target = name.clone();
                delete.set_tooltip_text(Some("Delete this merged branch. The engine refuses protected, checked-out and session-owned branches"));
                crate::app::confirm_inline(&delete, "Delete?", move |_| {
                    if let Some(pop) = pop.upgrade() {
                        pop.popdown();
                    }
                    if let Some(ui) = weak.upgrade() {
                        e.git_action(&ui, "git.branch.delete", json!({"name":target}), None);
                    }
                });
                row.append(&delete);
            }
            list.append(&row);
            entries.push((row.upcast(), name.to_lowercase(), key));
        }
        let remotes: Vec<Value> = rows(v, "remote_branches")
            .into_iter()
            .filter(|remote| remote["local"].is_null())
            .collect();
        if !remotes.is_empty() {
            list.append(&label("Remote", "branch-heading"));
        }
        for remote in remotes {
            let name = text(&remote, "name").to_string();
            let key = branch_key("cloud", &name);
            key.add_css_class("remote");
            key.set_widget_name(&format!("branch-switch-{name}"));
            key.set_tooltip_text(Some(&format!(
                "Check out {} as a local branch tracking {name}",
                text(&remote, "branch")
            )));
            let pop = popover.downgrade();
            let e = self.clone();
            let weak = Rc::downgrade(ui);
            let target = name.clone();
            key.connect_clicked(move |_| {
                if let Some(pop) = pop.upgrade() {
                    pop.popdown();
                }
                if let Some(ui) = weak.upgrade() {
                    e.git_action(&ui, "git.branch.switch", json!({"name":target}), None);
                }
            });
            list.append(&key);
            entries.push((key.clone().upcast(), name.to_lowercase(), key));
        }
        if locals.is_empty() {
            list.append(&label("No local branches yet.", "branch-note"));
        }
        let entries = Rc::new(entries);
        let filter = entries.clone();
        let create_title = create_key.child().and_then(|face| face.last_child()).and_downcast::<gtk::Label>();
        search.connect_search_changed(move |search| {
            let query = search.text().trim().to_lowercase();
            for (row, needle, _) in filter.iter() {
                row.set_visible(needle.contains(&query));
            }
            if let Some(title) = &create_title {
                title.set_text(&if query.is_empty() {
                    String::from("Create new branch…")
                } else {
                    format!("Create branch “{}”…", search.text().trim())
                });
            }
        });
        let first = entries.clone();
        let create = create_key.clone();
        search.connect_activate(move |_| {
            match first.iter().find(|(row, _, _)| row.is_visible()) {
                Some((_, _, key)) => key.emit_clicked(),
                None => create.emit_clicked(),
            }
        });
        let clear_search = search.clone();
        popover.connect_show(move |_| {
            clear_search.set_text("");
            clear_search.grab_focus();
        });
    }

    pub(super) fn git_action(
        self: &Rc<Self>,
        ui: &Rc<Ui>,
        op: &'static str,
        extra: Value,
        input: Option<gtk::Entry>,
    ) {
        if self.git_busy.get() {
            return;
        }
        // Only a branch switch rewrites the files under the editor: staging, committing and
        // the remote leave them alone, so unsaved edits hold back only that.
        let rewrites = matches!(op, "git.branch.create" | "git.branch.switch");
        if rewrites && self.is_occupied() {
            ui.show_error(if self.buffer.is_modified() && !self.busy.get() {
                "Save or discard your editor changes before switching branches."
            } else {
                "Wait for the file to finish opening or saving."
            });
            return;
        }
        if self.writing.get() {
            ui.show_error("Wait for the save or file change to finish.");
            return;
        }
        let retry = extra.clone();
        let mut payload = self.payload(ui, extra);
        if matches!(op, "git.fetch" | "git.branch.delete") {
            payload.as_object_mut().unwrap().remove("worktree");
        }
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let revision = self.revision.get();
        let input_snapshot = input.as_ref().map(|e| e.text());
        let commit_snapshot = (op == "git.commit").then(|| commit_text(&self.commit_message));
        let e = self.clone();
        let ui = ui.clone();
        // The panel goes insensitive while the action runs, which takes the keyboard focus
        // off whatever key had it.
        let focus = self.git.root().and_then(|root| root.focus()).filter(|focus| focus.is_ancestor(&self.git));
        self.git_busy.set(true);
        self.git.set_sensitive(false);
        if rewrites {
            self.view.set_editable(false);
        }
        glib::spawn_future_local(async move {
            // Push, Open PR and Delete branch were confirmed on their keys (confirm_inline).
            let result = guarded(&ui, op, payload).await;
            e.git_busy.set(false);
            e.git.set_sensitive(true);
            // It comes back to that key (a rebuild then moves it to the same row or the next,
            // `refocus_change`), unless it went somewhere else meanwhile.
            if let Some(focus) = focus.filter(|focus| focus.is_mapped() && focus.is_ancestor(&e.git)) {
                if e.git.root().and_then(|root| root.focus()).is_none() {
                    focus.grab_focus();
                }
            }
            if !e.matches(&ui, project, &worktree) {
                return;
            }
            // A file opened meanwhile owns the document: leave it alone, but still report the
            // outcome (RA-453).
            let document = revision == e.revision.get();
            if document && rewrites {
                e.set_busy(false);
            }
            match result {
                Ok(Some(v)) => {
                    if commit_snapshot
                        .as_ref()
                        .is_some_and(|snapshot| commit_text(&e.commit_message) == *snapshot)
                    {
                        e.commit_message.buffer().set_text("");
                    }
                    if let (Some(field), Some(snapshot)) = (input, input_snapshot) {
                        if field.text() == snapshot {
                            field.set_text("");
                        }
                    }
                    if let Some(url) = v["url"].as_str() {
                        ui.show_info(url);
                    } else if let Some(sha) = v["sha"].as_str() {
                        ui.show_info(&format!("Committed {}", &sha[..sha.len().min(7)]));
                    } else if op == "git.branch.switch" && v["created"] == true {
                        ui.show_info(&format!(
                            "Created the local branch {} tracking {}",
                            text(&v, "branch"),
                            text(&retry, "name")
                        ));
                    } else if op == "git.push" {
                        ui.show_info("Pushed");
                    } else if op == "git.fetch" {
                        // The counts are the primary checkout's branch, so only it shows them.
                        ui.show_info(&match v["behind"].as_i64().filter(|_| worktree.is_empty()) {
                            Some(0) => String::from("Fetched · up to date"),
                            Some(behind) => format!("Fetched · {behind} to pull"),
                            None => String::from("Fetched"),
                        });
                    }
                    if rewrites {
                        if document {
                            // The file shown is shown again from the new branch, where its caret
                            // was; one the branch does not have closes without a word.
                            let reopen = (!e.diff.get() && !e.image_mode.get() && !e.path.borrow().is_empty())
                                .then(|| (e.path.borrow().clone(), e.caret()));
                            let (agents, design) = (e.agents_visible(), e.design_visible());
                            e.clear_document();
                            if let Some((path, caret)) = reopen {
                                e.goto.set(Some(caret));
                                e.quiet_missing.set(true);
                                e.open(&ui, path);
                                // Loading shows the source; the view that was up stays (RA-215).
                                if agents {
                                    e.show_agents();
                                } else if design {
                                    e.show_design();
                                }
                            }
                        }
                        e.load_tree(&ui);
                    }
                    e.refresh_git(&ui);
                    e.refresh_scopes(&ui);
                }
                Ok(None) => {}
                Err(Error::Bus(err))
                    if op == "git.branch.switch"
                        && err.code == "git.checkout_dirty"
                        && retry["carry_changes"].is_null() =>
                {
                    // Offer what `git switch` does by default; only this retry names the field,
                    // so an engine that predates it still answers the first request.
                    let target = text(&retry, "name").to_string();
                    let dialog = crate::panel::Panel::new(&ui, "Uncommitted changes", 480);
                    let copy = label(&err.message, "body");
                    copy.set_wrap(true);
                    copy.set_selectable(true);
                    dialog.body.append(&copy);
                    let note = label(
                        "Bringing them along works like git switch: they stay uncommitted on the other branch, and git refuses if that branch changes the same files. Nothing is discarded.",
                        "dim",
                    );
                    note.set_wrap(true);
                    dialog.body.append(&note);
                    if dialog.response(&format!("Bring changes to {target}")).await
                        && e.matches(&ui, project, &worktree)
                    {
                        e.git_action(
                            &ui,
                            "git.branch.switch",
                            json!({"name":target,"carry_changes":true}),
                            None,
                        );
                    }
                }
                Err(err) => ui.show_error(&readable(&err)),
            }
        });
    }
    /// Stage conflicted files as resolved, asking first when one still holds conflict markers.
    fn mark_resolved(self: &Rc<Self>, ui: &Rc<Ui>, paths: Vec<String>) {
        if self.git_busy.get() {
            return;
        }
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let mut marked = Vec::new();
            for path in &paths {
                // A side that deleted the file, or a binary one, has no text to check.
                let read = ui
                    .call(
                        "file.read",
                        json!({"project_id":project,"worktree":optional_scope(&worktree),"path":path,"max_bytes":1048576}),
                    )
                    .await;
                if read.is_ok_and(|v| v["text"].as_str().is_some_and(has_conflict_markers)) {
                    marked.push(path.as_str());
                }
            }
            if !e.matches(&ui, project, &worktree) {
                return;
            }
            if !marked.is_empty() {
                let copy = format!(
                    "These files still contain conflict markers (<<<<<<< … >>>>>>>). Marking them resolved stages the markers, and the next commit records them.\n\n{}",
                    marked.join("\n")
                );
                if !confirm(&ui, "Mark as resolved with conflict markers?", &copy, "Mark as resolved").await
                    || !e.matches(&ui, project, &worktree)
                {
                    return;
                }
            }
            e.git_action(&ui, "git.stage", json!({"paths":paths}), None);
        });
    }
    /// Show `path`'s changes against HEAD: what is staged when `staged`, from `old_path` for a
    /// rename. False when it was refused, or is a picture, which shows instead.
    fn open_diff(self: &Rc<Self>, ui: &Rc<Ui>, path: String, staged: bool, old_path: Option<String>) -> bool {
        if let Some(refusal) = self.replace_refusal(Some("Save or discard your edits before opening a diff.")) {
            ui.show_error(refusal);
            return false;
        }
        // An image's diff is bytes; show the picture instead.
        if super::image_preview::is_image(&path) {
            self.open_image(ui, path);
            return false;
        }
        self.revision.set(self.revision.get() + 1);
        let revision = self.revision.get();
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let payload = self.payload(ui, json!({"path":path,"staged":staged,"old_path":old_path}));
        self.set_busy(true);
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let result = ui.call("git.diff.file", payload).await;
            if revision != e.revision.get() {
                return;
            }
            e.set_busy(false);
            if !e.matches(&ui, project, &worktree) {
                return;
            }
            match result {
                Ok(v) => {
                    let old = text(&v, "old");
                    let new = text(&v, "new");
                    if old.len() + new.len() > 1048576 {
                        ui.show_error("Diff exceeds the 1 MiB editor limit.");
                        return;
                    }
                    e.diff.set(true);
                    e.image_mode.set(false);
                    e.image.clear();
                    *e.diff_target.borrow_mut() = (staged, old_path);
                    *e.path.borrow_mut() = path.clone();
                    e.project.set(project);
                    *e.diff_data.borrow_mut() = Some(v);
                    e.rerender_diff();
                    e.set_busy(false);
                }
                Err(err) => ui.show_error(&readable(&err)),
            }
        });
        true
    }
    /// The open diff read again after a file or git change, and drawn only when it differs:
    /// "HEAD ↔ working tree" and its counts follow an agent's edits.
    pub(super) fn refresh_diff(self: &Rc<Self>, ui: &Rc<Ui>) {
        let path = self.path.borrow().clone();
        let (staged, old_path) = self.diff_target.borrow().clone();
        let (revision, project, worktree) = (self.revision.get(), ui.project.get(), self.worktree.borrow().clone());
        let payload = self.payload(ui, json!({"path":path,"staged":staged,"old_path":old_path}));
        let (e, ui) = (self.clone(), ui.clone());
        glib::spawn_future_local(async move {
            let Ok(v) = ui.call("git.diff.file", payload).await else { return };
            let current = e.revision.get() == revision && e.diff.get() && *e.path.borrow() == path && e.matches(&ui, project, &worktree);
            if !current || e.diff_data.borrow().as_ref() == Some(&v) || text(&v, "old").len() + text(&v, "new").len() > 1048576 {
                return;
            }
            *e.diff_data.borrow_mut() = Some(v);
            e.rerender_diff();
        });
    }
    /// Draw the open diff split or inline, with removed lines red and added lines green
    /// over the file's own syntax colours.
    pub(super) fn rerender_diff(&self) {
        let Some(v) = self.diff_data.borrow().clone() else {
            return;
        };
        if !self.diff.get() {
            return;
        }
        let path = self.path.borrow().clone();
        let old = text(&v, "old");
        let new = text(&v, "new");
        let unified: String = rows(&v, "hunks").iter().map(|hunk| text(hunk, "text")).collect();
        let (removed, added) = diff_marks(&unified);
        let language = sourceview5::LanguageManager::default().guess_language(Some(&path), None);
        self.buffer.set_language(language.as_ref());
        self.before.set_language(language.as_ref());
        // The line of the first change, where the caret goes.
        let first;
        if self.diff_inline.get() {
            let lines = inline_diff(&unified, new);
            first = lines.iter().position(|(mark, _)| !matches!(mark, Mark::Same));
            let body = lines.iter().map(|(_, line)| *line).collect::<Vec<_>>().join("\n");
            self.buffer.set_text(&body);
            self.before.set_text("");
            self.before_scroll.set_visible(false);
            self.view.set_show_line_numbers(false);
            let gone = line_tag(&self.buffer, "diff-removed", REMOVED);
            let new_tag = line_tag(&self.buffer, "diff-added", ADDED);
            for (index, (mark, _)) in lines.iter().enumerate() {
                match mark {
                    Mark::Removed => tag_line(&self.buffer, &gone, index),
                    Mark::Added => tag_line(&self.buffer, &new_tag, index),
                    Mark::Same => {}
                }
            }
            self.caption.set_text(&format!("{path} · inline diff"));
        } else {
            // Lines before the first change are the same on both sides, so whichever side it
            // starts on, it starts at the same line on the other.
            first = added.first().copied().into_iter().chain(removed.first().copied()).min();
            self.before.set_text(old);
            self.buffer.set_text(new);
            self.before_scroll.set_visible(true);
            self.view.set_show_line_numbers(true);
            let gone = line_tag(&self.before, "diff-removed", REMOVED);
            for line in &removed {
                tag_line(&self.before, &gone, *line);
            }
            let new_tag = line_tag(&self.buffer, "diff-added", ADDED);
            for line in &added {
                tag_line(&self.buffer, &new_tag, *line);
            }
            self.caption.set_text(&format!("{path} · HEAD ↔ working tree"));
        }
        self.buffer.set_modified(false);
        // A change past the last line (lines added at the end) is at the end.
        let line = |buffer: &sourceview5::Buffer| match first {
            Some(line) => buffer.iter_at_line(line as i32).unwrap_or_else(|| buffer.end_iter()),
            None => buffer.start_iter(),
        };
        self.buffer.place_cursor(&line(&self.buffer));
        // Scrolled once the views have their new size, as `reveal` does; HEAD's side with it.
        let mut views = vec![(self.view.clone(), self.buffer.get_insert())];
        if self.before_scroll.is_visible() {
            if let Some(before) = self.before_scroll.child().and_downcast::<sourceview5::View>() {
                self.before.place_cursor(&line(&self.before));
                views.push((before, self.before.get_insert()));
            }
        }
        glib::idle_add_local_once(move || {
            for (view, mark) in views {
                view.scroll_to_mark(&mark, 0.1, true, 0.0, 0.3);
            }
        });
        // After the caret moved, whose position line this replaces.
        self.position.set_text(&if unified.is_empty() {
            String::from("No changes against HEAD · read only")
        } else {
            format!("+{} −{} · read only", added.len(), removed.len())
        });
    }
    fn show_commit(self: &Rc<Self>, ui: &Rc<Ui>, sha: String) {
        let project = ui.project.get();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            match ui
                .call("git.show", json!({"project_id":project,"sha":sha}))
                .await
            {
                Ok(v) if ui.project.get() == project => {
                    let commit = &v["commit"];
                    let full = text(commit, "sha");
                    let dialog = crate::panel::Panel::new(
                        &ui,
                        &format!("Commit {}", &full[..full.len().min(7)]),
                        720,
                    );
                    dialog.add_css_class("commit-panel");
                    let subject = label(text(commit, "subject"), "commit-title");
                    subject.set_wrap(true);
                    subject.set_selectable(true);
                    dialog.body.append(&subject);
                    let meta = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    meta.add_css_class("commit-meta");
                    meta.append(&crate::icons::image("user", 13));
                    meta.append(&label(text(commit, "author"), "commit-author"));
                    // "3 h ago", the moment itself on hover.
                    let at = text(commit, "at");
                    let when = label(&crate::relative::ago(at, crate::relative::Form::Long).unwrap_or_else(|| at.to_string()), "commit-when");
                    when.set_tooltip_text(Some(at));
                    meta.append(&when);
                    let sha_label = label(full, "commit-sha");
                    sha_label.set_selectable(true);
                    sha_label.set_hexpand(true);
                    sha_label.set_xalign(1.0);
                    meta.append(&sha_label);
                    dialog.body.append(&meta);
                    let body_text = text(commit, "body").trim();
                    if !body_text.is_empty() {
                        let body = label(body_text, "commit-body");
                        body.set_wrap(true);
                        body.set_selectable(true);
                        dialog.body.append(&body);
                    }
                    let files = rows(&v, "files");
                    let (added, removed) = files.iter().fold((0, 0), |(a, r), file| {
                        (a + file["added"].as_i64().unwrap_or(0), r + file["removed"].as_i64().unwrap_or(0))
                    });
                    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    heading.add_css_class("commit-files-head");
                    let count = label(
                        &format!("{} FILE{} CHANGED", files.len(), if files.len() == 1 { "" } else { "S" }),
                        "scm-title",
                    );
                    count.set_hexpand(true);
                    heading.append(&count);
                    heading.append(&label(&format!("+{added}"), "diff-plus"));
                    heading.append(&label(&format!("−{removed}"), "diff-minus"));
                    dialog.body.append(&heading);
                    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    list.add_css_class("commit-files");
                    for file in files.iter().take(CHANGE_ROWS) {
                        let path = text(file, "path");
                        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                        row.add_css_class("commit-file");
                        let status = project_files::status_letter(text(file, "status"));
                        let letter = label(status.map(|(l, _, _)| l).unwrap_or("·"), "scm-letter");
                        if let Some((_, class, word)) = status {
                            letter.add_css_class(class);
                            letter.set_tooltip_text(Some(word));
                        }
                        row.append(&letter);
                        row.append(&project_files::file_image(path, 14));
                        let (directory, name) = split_path(path);
                        let title = label(name, "scm-name");
                        if let Some((_, class, _)) = status {
                            title.add_css_class(class);
                        }
                        row.append(&title);
                        let place = label(
                            &match file["old_path"].as_str() {
                                Some(old) if old != path => format!("{directory} ← {old}"),
                                _ => directory.to_owned(),
                            },
                            "scm-dir",
                        );
                        place.set_hexpand(true);
                        place.set_ellipsize(gtk::pango::EllipsizeMode::Start);
                        row.append(&place);
                        if file["binary"] == true {
                            row.append(&label("binary", "branch-tag"));
                        } else {
                            row.append(&label(&format!("+{}", file["added"]), "diff-plus"));
                            row.append(&label(&format!("−{}", file["removed"]), "diff-minus"));
                            row.append(&change_bar(
                                file["added"].as_i64().unwrap_or(0),
                                file["removed"].as_i64().unwrap_or(0),
                            ));
                        }
                        row.set_tooltip_text(Some(path));
                        list.append(&row);
                    }
                    if let Some(more) = more_files(files.len()) {
                        list.append(&more);
                    }
                    dialog.body.append(&list);
                    dialog.present();
                }
                Ok(_) => {}
                Err(err) => ui.show_error(&readable(&err)),
            }
        });
    }
}

/// A row in the Remote section: icon, title and hint, then the caller's key.
fn remote_row(parent: &gtk::Box, icon: &str, title: &str, hint: &str) -> (gtk::Box, gtk::Box, gtk::Label, gtk::Label) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("scm-remote-row");
    let glyph = crate::icons::image(icon, 14);
    glyph.set_valign(gtk::Align::Center);
    row.append(&glyph);
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 1);
    copy.set_hexpand(true);
    copy.set_valign(gtk::Align::Center);
    let title = label(title, "scm-remote-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    copy.append(&title);
    let hint = label(hint, "scm-remote-hint");
    hint.set_ellipsize(gtk::pango::EllipsizeMode::End);
    copy.append(&hint);
    row.append(&copy);
    parent.append(&row);
    (row, copy, title, hint)
}

/// A picker row: leading icon and a name, with room after it for counts and tags.
fn branch_key(icon: &str, title: &str) -> gtk::Button {
    let key = gtk::Button::new();
    key.add_css_class("branch-row");
    key.set_hexpand(true);
    let face = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    face.append(&crate::icons::image(icon, 14));
    let name = label(title, "branch-name");
    name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    name.set_hexpand(true);
    face.append(&name);
    key.set_child(Some(&face));
    key
}

/// GitHub's five-block change bar, green then red.
fn change_bar(added: i64, removed: i64) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    bar.add_css_class("change-bar");
    bar.set_valign(gtk::Align::Center);
    let total = (added + removed).max(1) as f64;
    let mut green = (added as f64 * 5. / total).round() as i64;
    if added > 0 {
        green = green.max(1);
    }
    if removed > 0 {
        green = green.min(4);
    }
    let red = if removed > 0 { 5 - green } else { 0 };
    for index in 0..5 {
        let block = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        block.add_css_class(if index < green {
            "bar-added"
        } else if index < green + red {
            "bar-removed"
        } else {
            "bar-empty"
        });
        block.set_size_request(7, 7);
        bar.append(&block);
    }
    bar
}

const REMOVED: &str = "rgba(229,72,62,0.17)";
const ADDED: &str = "rgba(46,196,105,0.14)";

fn line_tag(buffer: &sourceview5::Buffer, name: &str, color: &str) -> gtk::TextTag {
    let table = buffer.tag_table();
    if let Some(tag) = table.lookup(name) {
        return tag;
    }
    let tag = gtk::TextTag::builder()
        .name(name)
        .paragraph_background(color)
        .build();
    table.add(&tag);
    tag
}
fn tag_line(buffer: &sourceview5::Buffer, tag: &gtk::TextTag, line: usize) {
    if let Some(start) = buffer.iter_at_line(line as i32) {
        let mut end = start;
        if !end.forward_line() {
            end = buffer.end_iter();
        }
        buffer.apply_tag(tag, &start, &end);
    }
}

/// A yes or no over `copy` shown as code (a held request, a list of paths); `accept` names
/// what yes does.
async fn confirm(ui: &Ui, title: &str, copy: &str, accept: &str) -> bool {
    let dialog = crate::panel::Panel::new(ui, title, 620);
    let view = gtk::TextView::new();
    view.set_editable(false);
    view.set_monospace(true);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.buffer().set_text(copy);
    dialog.body.append(&scrolled(&view));
    dialog.response(accept).await
}

pub(super) async fn guarded(ui: &Ui, op: &str, payload: Value) -> Result<Option<Value>, Error> {
    match ui.call(op, payload.clone()).await {
        Ok(v) => Ok(Some(v)),
        Err(Error::Bus(err)) if err.kind == relay_bus::ErrorKind::Held => {
            let Some(confirmation) = err.confirm.as_ref().filter(|c| c.op == "guardrail.confirm")
            else {
                return Err(Error::Bus(err));
            };
            let inspection = ui
                .call("guardrail.hold.get", confirmation.payload.clone())
                .await?;
            if text(&inspection["request"], "op") != op
                || inspection["request"]["payload"] != payload
            {
                return Err(Error::Protocol(
                    "Held action does not match the requested operation. Inspect it in Guardrails."
                        .into(),
                ));
            }
            // A held save carries the whole file; the dialog shows its head, not all of it.
            let mut request = serde_json::to_string_pretty(&inspection["request"]).unwrap_or_default();
            if request.len() > 2000 {
                let end = (0..=2000).rev().find(|&i| request.is_char_boundary(i)).unwrap_or(0);
                request.truncate(end);
                request.push('…');
            }
            let copy = format!("{}\n\n{}", err.message, request);
            if !confirm(ui, "Allow this held action once?", &copy, "Allow once").await {
                return Ok(None);
            }
            let result = ui
                .call("guardrail.confirm", confirmation.payload.clone())
                .await?;
            if result["outcome"]["ok"] == true {
                Ok(Some(result["outcome"]["result"].clone()))
            } else {
                Err(Error::Protocol(format!(
                    "Held action was not completed: {}",
                    result["outcome"]["error"]
                )))
            }
        }
        Err(err) => Err(err),
    }
}

fn detach(entry: &impl IsA<gtk::Widget>) {
    if let Some(parent) = entry.parent() {
        if let Some(parent) = parent.downcast_ref::<gtk::Box>() {
            parent.remove(entry);
        } else if let Some(parent) = parent.downcast_ref::<gtk::Overlay>() {
            parent.set_child(gtk::Widget::NONE);
        }
    }
}

fn commit_text(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}
fn graph_widget(row: GraphRow, lanes: usize) -> gtk::DrawingArea {
    let graph = gtk::DrawingArea::new();
    graph.set_content_width(lanes as i32 * 14);
    graph.set_content_height(44);
    graph.set_draw_func(move |widget, cr, _, _| {
        let colors = [
            (79.0 / 255.0, 143.0 / 255.0, 217.0 / 255.0),
            (201.0 / 255.0, 161.0 / 255.0, 59.0 / 255.0),
            (165.0 / 255.0, 108.0 / 255.0, 214.0 / 255.0),
            (63.0 / 255.0, 176.0 / 255.0, 165.0 / 255.0),
            (217.0 / 255.0, 122.0 / 255.0, 79.0 / 255.0),
        ];
        let node = row.lane as f64 * 14.0 + 7.0;
        cr.set_line_width(1.5);
        for &(kind, lane) in &row.links {
            let x = lane as f64 * 14.0 + 7.0;
            let (r, g, b) = colors[lane % colors.len()];
            cr.set_source_rgba(r, g, b, 0.8);
            match kind {
                0 => {
                    cr.move_to(x, 0.0);
                    cr.line_to(x, 44.0);
                }
                1 => {
                    cr.move_to(x, 0.0);
                    if x == node {
                        cr.line_to(node, 22.0);
                    } else {
                        cr.curve_to(x, 22.0, node, 11.0, node, 22.0);
                    }
                }
                _ => {
                    cr.move_to(node, 22.0);
                    if x == node {
                        cr.line_to(x, 44.0);
                    } else {
                        cr.curve_to(node, 33.0, x, 22.0, x, 44.0);
                    }
                }
            }
            let _ = cr.stroke();
        }
        cr.arc(node, 22.0, 3.5, 0.0, std::f64::consts::TAU);
        if let Some(color) =
            widget
                .style_context()
                .lookup_color(if row.merge { "ink" } else { "console" })
        {
            cr.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                color.alpha() as f64,
            );
        }
        let _ = cr.fill_preserve();
        let (r, g, b) = colors[row.lane % colors.len()];
        cr.set_source_rgb(r, g, b);
        cr.set_line_width(2.0);
        let _ = cr.stroke();
    });
    graph
}
#[cfg(test)]
mod graph_tests {
    use super::*;
    // The diff, graph and status tests live with the logic in relay-client's git_view.
    #[test]
    fn status_letters_and_file_glyphs() {
        assert_eq!(project_files::status_letter("?").map(|s| s.0), Some("U"));
        assert_eq!(project_files::file_glyph("src/main.rs"), ("file-rust", "ft-rust"));
        assert_eq!(project_files::file_glyph("Cargo.lock").0, "file-lock");
        assert_eq!(project_files::file_glyph("art/Logo.PNG").0, "file-image");
        assert_eq!(project_files::file_glyph("web/App.tsx").0, "file-react");
        assert_eq!(project_files::file_glyph(".env.local").0, "file-config");
        assert_eq!(project_files::file_glyph("notes").0, "file");
    }
}
