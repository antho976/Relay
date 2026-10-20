use crate::app::{Ui, button, clear, label, rows, scrolled, text};
use gtk4 as gtk;
use serde_json::{Value, json};
#[path = "code_git.rs"]
mod code_git;
#[path = "project_files.rs"]
mod project_files;
#[path = "image_preview.rs"]
pub(crate) mod image_preview;
#[path = "find_bar.rs"]
pub mod find_bar;
use sourceview5::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct Editor {
    pub root: gtk::Box,
    tree: gtk::Box,
    expanded: RefCell<std::collections::BTreeSet<String>>,
    scope: gtk::MenuButton,
    scope_label: gtk::Label,
    file_sidebar: gtk::Box,
    files_split: gtk::Paned,
    git_split: gtk::Paned,
    document_tools: gtk::Box,
    selected_path: RefCell<String>,
    selected_directory: Cell<bool>,
    worktree: RefCell<String>,
    worktrees: RefCell<Vec<Value>>,

    scope_project: Cell<i64>,
    scope_revision: Cell<u64>,
    search: gtk::SearchEntry,
    git: gtk::Box,
    git_revision: Cell<u64>,
    /// The project and checkout the Git panel's rows belong to.
    git_scope: RefCell<(i64, String)>,
    git_refresh_pending: Cell<bool>,
    git_refresh_dirty: Cell<bool>,
    /// The project's last `git.pr.list` answer, the Remote section's pull request row it
    /// fills, and whether a lookup is in flight (`code_git`).
    prs: RefCell<code_git::PrState>,
    /// The next refresh asks GitHub for pull requests again instead of taking the engine's
    /// minute-old listing: set by the Refresh button, never by a file event (D130).
    pr_refresh: Cell<bool>,
    /// A `file.search` is in flight; `search_dirty` asks for one more when it answers, so file
    /// events during a slow search coalesce instead of queueing searches on the connection.
    search_pending: Cell<bool>,
    search_dirty: Cell<bool>,
    /// Set when an event arrived while the panel was hidden; consumed when it is shown.
    git_stale: Cell<bool>,
    tree_stale: Cell<bool>,
    /// What the Git panel last drew (`git_signature`), and the view state a rebuild keeps.
    git_signature: Cell<u64>,
    git_deferred: Cell<bool>,
    git_split_position: Cell<i32>,
    git_graph: Cell<bool>,
    git_merge_picks: RefCell<std::collections::HashSet<String>>,
    /// Commits the History section lists: `HISTORY_PAGE`, raised by its "Show more".
    git_history: Cell<u32>,
    /// The commit message and new-branch fields typed for each project and checkout, kept for
    /// the session when another is shown (`refresh_git`).
    git_drafts: RefCell<std::collections::HashMap<(i64, String), [String; 3]>>,
    tree_load_pending: Cell<bool>,
    /// A `load_tree` arrived while one was in flight: run once more when it answers.
    tree_load_again: Cell<bool>,
    git_busy: Cell<bool>,
    commit_message: gtk::TextView,
    branch_name: gtk::Entry,
    branch_start: gtk::Entry,
    /// An `invalidate` timer is armed; the smoke waits for none before timing a burst.
    pub(crate) invalidate_pending: Cell<bool>,
    diff: Cell<bool>,
    diff_switch: gtk::Box,
    diff_inline: Cell<bool>,
    diff_data: RefCell<Option<Value>>,
    before: sourceview5::Buffer,
    before_scroll: gtk::ScrolledWindow,
    content_stack: gtk::Stack,
    image: image_preview::Preview,
    image_mode: Cell<bool>,
    position: gtk::Label,
    /// Notes' find and replace bar (`find_bar`), over the document.
    find: Rc<find_bar::FindBar>,
    file_actions: gtk::Box,
    file_undo: gtk::Box,
    buffer: sourceview5::Buffer,
    view: sourceview5::View,
    caption: gtk::Label,
    path: RefCell<String>,
    original: RefCell<String>,
    project: Cell<i64>,
    revision: Cell<u64>,
    save: gtk::Button,
    discard: gtk::Button,
    handlers: Cell<bool>,
    busy: Cell<bool>,
    /// A save or an explorer change is in flight. Unlike `busy`, which a file still loading
    /// also sets, it is work that closing or switching away would lose (RA-455).
    writing: Cell<bool>,
    pub(crate) tree_revision: Cell<u64>,
    /// What the explorer last drew (`load_tree`); an identical listing is not rebuilt.
    tree_signature: Cell<u64>,
    /// Entries drawn per folder beyond the first `TREE_PAGE`, raised by "Show more".
    tree_limits: RefCell<std::collections::BTreeMap<String, usize>>,
    /// The explorer's rows by path: a refresh updates them in place instead of rebuilding
    /// the tree, so expansion, focus, hover and scroll stay.
    tree_rows: RefCell<std::collections::HashMap<String, Rc<TreeRow>>>,
    /// Each changed file's status letter from the Git panel's `git.status`, which tints the
    /// explorer: `file.tree` is asked without badges, which cost it a `git status` per folder.
    tree_badges: RefCell<TreeBadges>,
    /// `git.status` requests begun or promised (a refresh queued behind one in flight): a tree
    /// load that saw none start while it ran asks for one itself (`refresh_badges`).
    pub(super) status_requests: Cell<u64>,
    /// Where the next `read_file` puts the caret: 1-based line, 1-based byte column, and the
    /// length in characters to select (a search hit).
    goto: Cell<Option<(u32, u32, usize)>>,
    /// The next `read_file` says nothing when the file is not there: a branch switch reopening
    /// the file it showed, which the other branch may not have.
    quiet_missing: Cell<bool>,
    /// The trash entry the explorer's Restore key brings back.
    undo_trash: Cell<i64>,
    /// Shown when the open document changed on disk while it has unsaved edits.
    conflict_bar: gtk::Box,
    conflict_label: gtk::Label,
    /// The on-disk text the conflict bar was raised for.
    disk_text: RefCell<Option<String>>,
    disk_check_pending: Cell<bool>,
}
/// Entries of one folder the explorer draws before a "Show more" row.
const TREE_PAGE: usize = 500;
/// What runs once a save lands (`save_then`).
type AfterSave = Box<dyn FnOnce(&Rc<Editor>, &Rc<Ui>)>;
thread_local! {
    /// The last search Code's find bar ran, which F3 takes up when the field is empty.
    static FIND_QUERY: RefCell<find_bar::Memory> = const { RefCell::new(find_bar::Memory::new()) };
}
struct TreeLoadGuard(Rc<Editor>, Rc<Ui>);
impl Drop for TreeLoadGuard {
    fn drop(&mut self) {
        self.0.tree_load_pending.set(false);
        if self.0.tree_load_again.replace(false) {
            self.0.load_tree(&self.1);
        }
    }
}

impl Editor {
    pub fn set_palette(&self, mode: &str) {
        let manager = sourceview5::StyleSchemeManager::default();
        if let Some(scheme) = manager
            .scheme(&format!("relay-{mode}"))
            .or_else(|| manager.scheme("relay-matte"))
        {
            self.buffer.set_style_scheme(Some(&scheme));
            self.before.set_style_scheme(Some(&scheme));
        }
    }

    pub fn new() -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("code-surface");
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tools.add_css_class("toolbar");
        tools.add_css_class("code-bar");
        let scope = gtk::MenuButton::new();
        scope.set_widget_name("project-scope");
        scope.set_valign(gtk::Align::Center);
        scope.set_tooltip_text(Some("Search projects and checkouts"));
        let scope_label = label("Select a project", "mono");
        scope_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        scope_label.set_max_width_chars(42);
        let scope_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        scope_content.append(&crate::icons::image("branch", 13));
        scope_content.append(&scope_label);
        scope_content.append(&crate::icons::image("chevron-down", 12));
        scope.set_child(Some(&scope_content));
        let project_tools = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        project_tools.add_css_class("code-bar");
        project_tools.add_css_class("workspace-strip");
        project_tools.add_css_class("page-layer");
        project_tools.append(&scope);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        project_tools.append(&spacer);
        // The project's views, one segmented control: the one showing is lit.
        let views = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        views.add_css_class("view-tabs");
        views.set_valign(gtk::Align::Center);
        project_tools.append(&views);
        let agents_button = button("Agents", "quiet");
        agents_button.set_widget_name("project-agents");
        agents_button.set_tooltip_text(Some("The project's agents: their terminals, side by side"));
        agents_button.add_css_class("selected");
        views.append(&agents_button);
        let design_button = button("Design", "quiet");
        design_button.set_widget_name("project-design");
        design_button.set_tooltip_text(Some("The project's designs: canvases you and its agents draw on"));
        views.append(&design_button);
        root.append(&project_tools);

        let caption = label("No file open", "title");
        caption.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        caption.set_hexpand(true);
        tools.append(&caption);
        // Split / inline, shown only while a diff is open.
        let diff_switch = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        diff_switch.add_css_class("linked");
        diff_switch.add_css_class("diff-switch");
        diff_switch.set_visible(false);
        let split_key = gtk::ToggleButton::with_label("Split");
        split_key.set_widget_name("project-diff-split");
        split_key.set_tooltip_text(Some("HEAD and the working tree side by side"));
        split_key.set_active(true);
        let inline_key = gtk::ToggleButton::with_label("Inline");
        inline_key.set_widget_name("project-diff-inline");
        inline_key.set_tooltip_text(Some("One column with removed lines above added ones"));
        inline_key.set_group(Some(&split_key));
        diff_switch.append(&split_key);
        diff_switch.append(&inline_key);
        tools.append(&diff_switch);
        let save = button("Save", "primary");
        save.set_widget_name("project-save");
        save.set_tooltip_text(Some("Save (Ctrl+S)"));
        let discard = crate::app::icon_button("undo", "Discard changes");
        discard.set_widget_name("project-discard");
        tools.append(&discard);
        tools.append(&save);

        let split = gtk::Paned::new(gtk::Orientation::Horizontal);
        split.set_position(260);
        split.add_css_class("code-split");
        split.set_resize_start_child(false);
        split.set_shrink_start_child(false);
        split.set_shrink_end_child(false);
        split.set_vexpand(true);
        let tree = gtk::Box::new(gtk::Orientation::Vertical, 2);
        tree.add_css_class("file-tree");
        let file_sidebar = gtk::Box::new(gtk::Orientation::Vertical, 6);
        file_sidebar.set_size_request(260, -1);
        file_sidebar.set_spacing(0);
        file_sidebar.add_css_class("code-files");
        file_sidebar.add_css_class("page-layer");
        let file_header = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        file_header.add_css_class("code-bar");
        file_header.add_css_class("explorer-head");
        let explorer_title = label("EXPLORER", "section-label");
        explorer_title.set_hexpand(true);
        file_header.append(&explorer_title);
        file_sidebar.append(&file_header);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search in worktree"));
        search.set_tooltip_text(Some(
            "Search file contents; press Enter to search, Escape to return to files",
        ));
        search.add_css_class("code-search");
        file_sidebar.append(&search);
        let file_actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        file_actions.add_css_class("explorer-actions");
        file_actions.set_valign(gtk::Align::Center);
        file_header.append(&file_actions);
        let file_undo = gtk::Box::new(gtk::Orientation::Vertical, 2);
        file_sidebar.append(&file_undo);
        file_sidebar.append(&scrolled(&tree));
        split.set_start_child(Some(&file_sidebar));
        let buffer = sourceview5::Buffer::new(None);
        let view = sourceview5::View::with_buffer(&buffer);
        view.set_widget_name("project-source");
        view.set_monospace(true);
        view.add_css_class("code-source");
        view.set_show_line_numbers(true);
        view.set_highlight_current_line(true);
        view.set_tab_width(4);
        view.set_auto_indent(true);
        view.set_editable(false);
        if let Some(scheme) = sourceview5::StyleSchemeManager::default().scheme("relay-matte") {
            buffer.set_style_scheme(Some(&scheme));
        }
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&tools);
        let find = find_bar::FindBar::new(&view, &buffer, "code", "Find in file", &FIND_QUERY);
        find.bar.add_css_class("toolbar");
        find.bar.add_css_class("code-find");
        content.append(&find.bar);
        let conflict_bar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        conflict_bar.add_css_class("toolbar");
        conflict_bar.set_widget_name("project-disk-conflict");
        let conflict_label = label("", "body");
        conflict_label.set_hexpand(true);
        conflict_label.set_wrap(true);
        conflict_label.set_xalign(0.0);
        conflict_bar.append(&conflict_label);
        let reload_key = button("Reload from disk", "quiet");
        reload_key.set_tooltip_text(Some("Drop your edits and show the file as it is on disk"));
        let keep_key = button("Keep my edits", "quiet");
        keep_key.set_tooltip_text(Some("Saving will replace the version on disk"));
        conflict_bar.append(&reload_key);
        conflict_bar.append(&keep_key);
        conflict_bar.set_visible(false);
        content.append(&conflict_bar);
        let before = sourceview5::Buffer::new(None);
        before.set_style_scheme(buffer.style_scheme().as_ref());
        let before_view = sourceview5::View::with_buffer(&before);
        before_view.set_monospace(true);
        before_view.add_css_class("code-source");
        before_view.set_show_line_numbers(true);
        before_view.set_editable(false);
        let before_scroll = scrolled(&before_view);
        before_scroll.set_visible(false);
        let compare = gtk::Paned::new(gtk::Orientation::Horizontal);
        compare.set_start_child(Some(&before_scroll));
        compare.set_end_child(Some(&scrolled(&view)));
        compare.set_resize_start_child(true);
        compare.set_resize_end_child(true);
        compare.set_position(360);
        compare.set_vexpand(true);
        let content_stack = gtk::Stack::new();
        content_stack.set_hhomogeneous(false);
        content_stack.set_vhomogeneous(false);
        content_stack.set_vexpand(true);
        content_stack.add_named(&compare, Some("source"));
        let image = image_preview::Preview::new();
        content_stack.add_named(&image.root, Some("image"));
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.set_halign(gtk::Align::Center);
        empty.set_valign(gtk::Align::Start);
        empty.set_margin_top(38);
        empty.add_css_class("code-empty");
        let glyph = crate::icons::image("code", 28);
        glyph.set_halign(gtk::Align::Center);
        glyph.add_css_class("faint");
        empty.append(&glyph);
        let title = label("Code, review, integrate", "title");
        title.set_xalign(0.5);
        title.set_margin_top(12);
        empty.append(&title);
        let hint = label(
            "Open a file from the tree, a change from the git panel, or a commit from a task. Saves go through the bus; guardrails apply to your edits too.",
            "dim",
        );
        hint.set_wrap(true);
        hint.set_max_width_chars(58);
        hint.set_justify(gtk::Justification::Center);
        hint.set_xalign(0.5);
        empty.append(&hint);
        content_stack.add_named(&empty, Some("empty"));
        content_stack.set_visible_child_name("empty");
        content.append(&content_stack);
        // The terminals' plates top up the page stack themselves (theme.css .page-stack);
        // a file, a picture or the empty state is a page like any other.
        let page = content.clone();
        content_stack.connect_visible_child_name_notify(move |stack| {
            if stack.visible_child_name().as_deref() == Some("agents") {
                page.remove_css_class("page-layer");
            } else {
                page.add_css_class("page-layer");
            }
        });
        content.add_css_class("page-layer");
        let position = label("Select a file to edit", "dim");
        position.add_css_class("code-position");
        content.append(&position);
        let workspace = gtk::Paned::new(gtk::Orientation::Horizontal);
        workspace.add_css_class("code-split");
        workspace.set_start_child(Some(&content));
        workspace.set_resize_start_child(true);
        workspace.set_shrink_start_child(false);
        workspace.set_resize_end_child(false);
        workspace.set_position(760);
        let git = gtk::Box::new(gtk::Orientation::Vertical, 0);
        git.set_vexpand(true);
        git.set_size_request(320, -1);
        git.add_css_class("code-git");
        git.add_css_class("page-layer");
        let git_panel = git.clone();
        workspace.set_shrink_end_child(false);
        workspace.set_end_child(Some(&git_panel));
        split.set_end_child(Some(&workspace));
        let find_button = crate::app::icon_button("search", "Find in file (Ctrl+F) · replace (Ctrl+H)");
        tools.append(&find_button);
        let files_button = button("Files", "quiet");
        files_button.set_widget_name("project-files");
        files_button.set_tooltip_text(Some("Show or hide the explorer: the checkout's files, and a search in them"));
        views.append(&files_button);
        let edit_button = crate::app::icon_button("edit", "Edit file");
        // Only a diff has a file to open for editing.
        edit_button.set_visible(false);
        diff_switch.bind_property("visible", &edit_button, "visible").sync_create().build();
        tools.append(&edit_button);
        let git_button = button("Git", "quiet");
        git_button.set_widget_name("project-git");
        git_button.set_tooltip_text(Some("Show or hide the Git panel: changes, commits, branches and the remote"));
        views.append(&git_button);
        let mut control = tools.first_child();
        while let Some(widget) = control {
            control = widget.next_sibling();
            widget.set_valign(gtk::Align::Center);
        }
        file_sidebar.set_visible(false);
        git.set_visible(false);
        root.append(&split);
        save.set_sensitive(false);
        discard.set_sensitive(false);
        let editor = Rc::new(Self {
            root,
            tree,
            expanded: RefCell::default(),
            scope,
            scope_label,
            file_sidebar,
            files_split: split,
            git_split: workspace,
            document_tools: tools,
            selected_path: RefCell::default(),
            selected_directory: Cell::new(false),
            worktree: RefCell::default(),
            worktrees: RefCell::default(),
            scope_project: Cell::new(0),
            scope_revision: Cell::new(0),
            search,
            git,
            git_revision: Cell::new(0),
            git_scope: RefCell::new((0, String::new())),
            git_busy: Cell::new(false),
            commit_message: gtk::TextView::new(),
            branch_name: gtk::Entry::new(),
            branch_start: gtk::Entry::new(),
            invalidate_pending: Cell::new(false),
            git_refresh_pending: Cell::new(false),
            git_refresh_dirty: Cell::new(false),
            prs: RefCell::default(),
            pr_refresh: Cell::new(false),
            search_pending: Cell::new(false),
            search_dirty: Cell::new(false),
            git_stale: Cell::new(false),
            tree_stale: Cell::new(false),
            git_signature: Cell::new(0),
            git_deferred: Cell::new(false),
            git_split_position: Cell::new(0),
            git_graph: Cell::new(true),
            git_merge_picks: RefCell::default(),
            git_history: Cell::new(code_git::HISTORY_PAGE),
            git_drafts: RefCell::default(),
            tree_load_pending: Cell::new(false),
            tree_load_again: Cell::new(false),
            diff: Cell::new(false),
            diff_switch,
            diff_inline: Cell::new(false),
            diff_data: RefCell::new(None),
            before,
            before_scroll,
            content_stack,
            image,
            image_mode: Cell::new(false),
            position,
            find,
            file_actions,
            file_undo,
            buffer,
            view,
            caption,
            path: RefCell::default(),
            original: RefCell::default(),
            project: Cell::new(0),
            revision: Cell::new(0),
            save,
            discard,
            handlers: Cell::new(false),
            busy: Cell::new(false),
            writing: Cell::new(false),
            tree_revision: Cell::new(0),
            tree_signature: Cell::new(0),
            tree_limits: RefCell::default(),
            tree_rows: RefCell::default(),
            tree_badges: RefCell::default(),
            status_requests: Cell::new(0),
            goto: Cell::new(None),
            quiet_missing: Cell::new(false),
            undo_trash: Cell::new(0),
            conflict_bar,
            conflict_label,
            disk_text: RefCell::new(None),
            disk_check_pending: Cell::new(false),
        });
        let weak = Rc::downgrade(&editor);
        keep_key.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                // The disk version becomes the base the save is checked against, so the next
                // Save writes the draft over it instead of failing as a conflict.
                if let Some(disk) = e.disk_text.borrow_mut().take() {
                    *e.original.borrow_mut() = disk;
                }
                e.conflict_bar.set_visible(false);
            }
        });
        let weak = Rc::downgrade(&editor);
        reload_key.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                // The bar already asked, so Discard runs without asking again.
                e.buffer.set_modified(false);
                e.discard.emit_clicked();
            }
        });
        let weak = Rc::downgrade(&editor);
        inline_key.connect_toggled(move |key| {
            if let Some(e) = weak.upgrade() {
                if e.diff_inline.replace(key.is_active()) != key.is_active() {
                    e.rerender_diff();
                }
            }
        });
        let weak = Rc::downgrade(&editor);
        editor.buffer.connect_modified_changed(move |b| {
            if let Some(e) = weak.upgrade() {
                e.save
                    .set_sensitive(b.is_modified() && e.view.is_editable());
                e.discard.set_sensitive(b.is_modified() && !e.busy.get());
                // Unsaved edits show by the name too, as a note's do in Notes.
                let path = e.path.borrow();
                if !path.is_empty() && !e.diff.get() && !e.image_mode.get() {
                    e.caption.set_text(&if b.is_modified() { format!("{path}  •") } else { path.clone() });
                }
            }
        });
        let weak = Rc::downgrade(&editor);
        find_button.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                if e.find.visible() {
                    e.find.close();
                } else {
                    e.find.open(false);
                }
            }
        });
        let weak = Rc::downgrade(&editor);
        editor.buffer.connect_cursor_position_notify(move |buffer| {
            if let Some(e) = weak.upgrade() {
                let iter = buffer.iter_at_offset(buffer.cursor_position());
                e.position.set_text(&format!(
                    "Ln {}, Col {}{}",
                    iter.line() + 1,
                    iter.line_offset() + 1,
                    if e.diff.get() {
                        " · HEAD / working tree · read only"
                    } else {
                        ""
                    }
                ));
            }
        });
        let weak = Rc::downgrade(&editor);
        edit_button.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                if e.diff.get() {
                    e.discard.emit_clicked();
                }
            }
        });
        let weak = Rc::downgrade(&editor);
        agents_button.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.show_agents();
            }
        });
        let weak = Rc::downgrade(&editor);
        design_button.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.show_design();
            }
        });
        let weak = Rc::downgrade(&editor);
        files_button.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.toggle_files();
            }
        });
        let weak = Rc::downgrade(&editor);
        git_button.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.toggle_git();
            }
        });
        // Files and Git open beside the view rather than replacing it, so their keys show
        // "open" (a lighter mark than the lit view) while their panel is up.
        for (panel, key) in [(editor.file_sidebar.clone(), files_button), (editor.git.clone(), git_button)] {
            panel.connect_visible_notify(move |panel| {
                if panel.is_visible() { key.add_css_class("open") } else { key.remove_css_class("open") }
            });
        }
        let weak = Rc::downgrade(&editor);
        let agents_tab = agents_button.clone();
        let design_tab = design_button.clone();
        editor
            .content_stack
            .connect_visible_child_name_notify(move |_| {
                if let Some(editor) = weak.upgrade() {
                    let design = editor.design_visible();
                    let document = !editor.agents_visible() && !design;
                    if editor.agents_visible() { agents_tab.add_css_class("selected") } else { agents_tab.remove_css_class("selected") }
                    if design { design_tab.add_css_class("selected") } else { design_tab.remove_css_class("selected") }
                    editor.document_tools.set_visible(document && !editor.image_mode.get());
                    editor.position.set_visible(document);
                    if !document || editor.image_mode.get() {
                        editor.find.hide();
                    }
                }
            });
        editor
    }
    /// Unsaved edits, or a write or Git operation in flight. A file that is only loading is
    /// not dirty: whatever replaces the document bumps `revision`, which drops the load.
    pub fn is_dirty(&self) -> bool {
        self.buffer.is_modified() || self.writing.get() || self.git_busy.get()
    }
    /// `is_dirty`, or a load in flight: the editor's own operations wait for either.
    fn is_occupied(&self) -> bool {
        self.is_dirty() || self.busy.get()
    }
    /// Why the document cannot be replaced now, if it cannot. A save, an explorer change or a
    /// Git action finishes first, and unsaved edits are saved or discarded first (`unsaved`
    /// says so; None when they are the ones being discarded). A file or diff still loading is
    /// no reason: whatever replaces it bumps `revision`, which drops the load.
    fn replace_refusal(&self, unsaved: Option<&'static str>) -> Option<&'static str> {
        if self.writing.get() || self.git_busy.get() {
            return Some("Wait for the save or Git action to finish.");
        }
        unsaved.filter(|_| self.buffer.is_modified() && !self.busy.get())
    }
    /// Run `then`, which replaces the document, once its unsaved edits are dealt with: asked
    /// Save / Don't Save / Cancel, as Notes asks before closing a tab. Save runs it once the
    /// save lands; Don't Save puts the file back as it was read first, so an open that fails
    /// leaves no edits passing for saved. Without edits it runs at once, and a save or Git
    /// action in flight is waited for.
    fn after_unsaved(self: &Rc<Self>, ui: &Rc<Ui>, then: impl FnOnce(&Rc<Self>, &Rc<Ui>) + 'static) {
        if let Some(refusal) = self.replace_refusal(None) {
            ui.show_error(refusal);
            return;
        }
        // Loading, the edits are already being discarded.
        if !self.buffer.is_modified() || self.busy.get() {
            then(self, ui);
            return;
        }
        let path = self.path.borrow().clone();
        let (project, worktree, revision) = (ui.project.get(), self.worktree.borrow().clone(), self.revision.get());
        let (e, ui) = (self.clone(), ui.clone());
        glib::spawn_future_local(async move {
            let dialog = gtk::AlertDialog::builder()
                .message(format!("Save changes to “{path}”?"))
                .detail("If you don't save, your changes will be lost.")
                .buttons(["Cancel", "Don't Save", "Save"])
                .cancel_button(0)
                .default_button(2)
                .modal(true)
                .build();
            let answer = dialog.choose_future(Some(&ui.window)).await;
            // Saved, discarded or replaced meanwhile: what this asked about is gone.
            if !e.matches(&ui, project, &worktree) || e.revision.get() != revision || !e.buffer.is_modified() {
                return;
            }
            match answer {
                Ok(1) => {
                    let original = e.original.borrow().clone();
                    e.reload_in_place(&original);
                    then(&e, &ui);
                }
                Ok(2) => e.save_then(&ui, Some(Box::new(then))),
                _ => {}
            }
        });
    }
    /// The caret as `goto` takes it (1-based line and byte column), for reopening a document
    /// where it was.
    fn caret(&self) -> (u32, u32, usize) {
        let at = self.buffer.iter_at_mark(&self.buffer.get_insert());
        let mut start = at;
        start.set_line_offset(0);
        (at.line() as u32 + 1, self.buffer.text(&start, &at, true).len() as u32 + 1, 0)
    }
    fn set_busy(&self, busy: bool) {
        if busy || !(self.agents_visible() || self.design_visible()) {
            self.content_stack
                .set_visible_child_name(if self.image_mode.get() {
                    "image"
                } else if busy || !self.path.borrow().is_empty() {
                    "source"
                } else {
                    "empty"
                });
        }
        self.set_locked(busy);
    }
    /// Busy without changing what the main area shows: an explorer operation must not pull
    /// the agents view away (RA-215).
    fn set_locked(&self, busy: bool) {
        self.busy.set(busy);
        self.view
            .set_editable(!busy && !self.diff.get() && !self.image_mode.get() && !self.path.borrow().is_empty());
        self.save.set_sensitive(!busy && self.buffer.is_modified());
        self.discard
            .set_sensitive(!busy && self.buffer.is_modified());
        self.diff_switch.set_visible(!busy && self.diff.get());
    }
    pub fn reset(&self) {
        self.scope_revision.set(self.scope_revision.get() + 1);
        self.git_revision.set(self.git_revision.get() + 1);
        clear(&self.file_undo);
        self.scope_project.set(0);
        self.selected_path.borrow_mut().clear();
        self.expanded.borrow_mut().clear();
        self.tree_limits.borrow_mut().clear();
        self.show_agents();
        // The commit message and branch fields stay with their checkout (`refresh_git`).
        self.worktree.borrow_mut().clear();
        self.worktrees.borrow_mut().clear();
        self.scope_label.set_text("Select a project");
        clear(&self.git);
        self.search.set_text("");
        self.clear_document();
    }
    fn clear_document(&self) {
        self.image_mode.set(false);
        self.image.clear();
        self.diff.set(false);
        self.before_scroll.set_visible(false);
        self.revision.set(self.revision.get() + 1);
        self.tree_revision.set(self.tree_revision.get() + 1);
        self.buffer.set_text("");
        self.buffer.set_modified(false);
        self.path.borrow_mut().clear();
        self.original.borrow_mut().clear();
        self.view.set_editable(false);
        self.set_busy(false);
        self.disk_text.borrow_mut().take();
        self.conflict_bar.set_visible(false);
        clear(&self.tree);
        self.tree_rows.borrow_mut().clear();
        self.tree_signature.set(0);
        // As before any file was opened: emptying the text put the caret at "Ln 1, Col 1".
        self.caption.set_text("No file open");
        self.position.set_text("Select a file to edit");
    }
    /// A file, git, worktree or integration event: `event` is its name and payload, and
    /// `None` refreshes unconditionally.
    pub fn invalidate(self: &Rc<Self>, ui: &Rc<Ui>, event: Option<(&str, &Value)>) {
        if event.is_some_and(|(ev, payload)| !self.concerns(ui, ev, payload)) {
            return;
        }
        // An open document is compared with the disk on any page; the panels refresh only
        // where they are seen.
        let shown = matches!(ui.page.borrow().as_str(), "code" | "agents");
        if (shown || !self.path.borrow().is_empty()) && !self.invalidate_pending.replace(true) {
            let weak = Rc::downgrade(ui);
            let e = self.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(1000), move || {
                e.invalidate_pending.set(false);
                if let Some(ui) = weak.upgrade() {
                    if matches!(ui.page.borrow().as_str(), "code" | "agents") {
                        e.refresh_shown(&ui);
                    }
                    e.check_disk(&ui);
                }
            });
        }
    }
    /// Compare the open document with its file on disk. Events carry no paths, and git or an
    /// agent can rewrite the file under any of them, so one read here is what tells a real
    /// change from the echo of our own save or a neighbour's edit. A clean document takes the
    /// new text in place; an edited one raises the conflict bar once per disk version.
    fn check_disk(self: &Rc<Self>, ui: &Rc<Ui>) {
        let path = self.path.borrow().clone();
        if path.is_empty()
            || self.diff.get()
            || self.image_mode.get()
            || self.busy.get()
            || self.disk_check_pending.replace(true)
        {
            return;
        }
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let revision = self.revision.get();
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let result = ui
                .call(
                    "file.read",
                    json!({"project_id":project,"worktree":optional_scope(&worktree),"path":path,"max_bytes":1048576}),
                )
                .await;
            e.disk_check_pending.set(false);
            if e.revision.get() != revision
                || e.busy.get()
                || !e.matches(&ui, project, &worktree)
                || *e.path.borrow() != path
            {
                return;
            }
            // A removed, binary or oversized file is left to Save, whose check reports it.
            let Some(disk) = result
                .ok()
                .filter(|v| v["truncated"] != true)
                .and_then(|v| v["text"].as_str().map(str::to_owned))
            else {
                return;
            };
            if disk == *e.original.borrow() {
                e.disk_text.borrow_mut().take();
                e.conflict_bar.set_visible(false);
                return;
            }
            if !e.buffer.is_modified() {
                e.reload_in_place(&disk);
                return;
            }
            if e.disk_text.borrow().as_deref() == Some(disk.as_str()) {
                return;
            }
            *e.disk_text.borrow_mut() = Some(disk);
            e.conflict_label.set_text(&format!(
                "{path} changed on disk while you were editing it. Reload it, or keep your edits and save over it."
            ));
            e.conflict_bar.set_visible(true);
        });
    }
    /// Replace a clean document's text with the disk's, keeping the caret line and scroll.
    fn reload_in_place(&self, disk: &str) {
        let cursor = self.buffer.iter_at_mark(&self.buffer.get_insert());
        let (line, offset) = (cursor.line(), cursor.line_offset());
        let scroll = self.view.vadjustment().map(|a| (a.value(), a));
        self.buffer.set_text(disk);
        self.buffer.set_modified(false);
        *self.original.borrow_mut() = disk.into();
        self.disk_text.borrow_mut().take();
        self.conflict_bar.set_visible(false);
        let at = self
            .buffer
            .iter_at_line_offset(line, offset)
            .or_else(|| self.buffer.iter_at_line(line))
            .unwrap_or_else(|| self.buffer.end_iter());
        self.buffer.place_cursor(&at);
        if let Some((value, adjustment)) = scroll {
            glib::idle_add_local_once(move || adjustment.set_value(value));
        }
    }
    /// Whether an event can change what this editor shows. Watcher events carry no envelope
    /// project, so their payload's project is checked here; a file event from another
    /// checkout is ignored, except the primary root's, which holds the shared `.git`.
    fn concerns(&self, ui: &Ui, ev: &str, payload: &Value) -> bool {
        if payload["project_id"].as_i64().is_some_and(|id| id != ui.project.get()) {
            return false;
        }
        let Some(changed) = payload["worktree"].as_str().filter(|_| ev == "file.changed") else {
            return true;
        };
        let canonical = |path: &str| std::fs::canonicalize(path).unwrap_or_else(|_| path.into());
        let changed = canonical(changed);
        let primary = ui
            .projects
            .borrow()
            .iter()
            .find(|p| p["id"] == ui.project.get())
            .map(|p| canonical(text(p, "path")));
        if primary.as_ref() == Some(&changed) {
            return true;
        }
        let shown = self.worktree.borrow();
        !shown.is_empty() && canonical(&shown) == changed
    }

    /// Refresh what is on screen. A hidden tree or Git panel is marked stale instead and
    /// refreshed when it is shown (`bind_controls`).
    fn refresh_shown(self: &Rc<Self>, ui: &Rc<Ui>) {
        if self.file_sidebar.is_mapped() {
            // A search's results stay until it is run again: re-reading the whole worktree
            // on every file event is a cost the explorer must not pay.
            if self.search.text().trim().is_empty() {
                self.load_tree(ui);
            }
        } else {
            self.tree_stale.set(true);
        }
        // The explorer's change tints need the status even while the Git panel is hidden;
        // refresh_git fetches only that when the panel is not on screen.
        if self.git.is_mapped() || self.file_sidebar.is_mapped() {
            self.refresh_git(ui);
        } else {
            self.git_stale.set(true);
        }
        self.refresh_scopes(ui);
    }

    /// List the explorer from the checkout's root, with every expanded folder open. The
    /// explorer has no directory scope (RA-687).
    pub fn load_tree(self: &Rc<Self>, ui: &Rc<Ui>) {
        if ui.project.get() == 0 {
            return;
        }
        if self.tree_load_pending.replace(true) {
            self.tree_revision.set(self.tree_revision.get() + 1);
            self.tree_load_again.set(true);
            return;
        }
        if !self.handlers.replace(true) {
            let weak = Rc::downgrade(ui);
            let editor = self.clone();
            self.bind_controls(ui);
            self.save.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    editor.save_file(&ui);
                }
            });
            let weak = Rc::downgrade(ui);
            let editor = self.clone();
            let buffer = self.buffer.clone();
            // `set_text` cannot be undone, so unsaved edits are asked about first.
            crate::app::confirm_inline_if(&self.discard, "Discard edits?", move || buffer.is_modified(), move |_| {
                if let Some(ui) = weak.upgrade() {
                    if editor.busy.get() {
                        return;
                    }
                    let path = editor.path.borrow().clone();
                    editor.read_file(&ui, path, true);
                }
            });
        }
        if self.scope_project.get() != ui.project.get() {
            self.refresh_scopes(ui);
            self.refresh_git(ui);
        }
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        self.tree_revision.set(self.tree_revision.get() + 1);
        let revision = self.tree_revision.get();
        let statuses = self.status_requests.get();
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let _pending = TreeLoadGuard(e.clone(), ui.clone());
            let result = ui
                .call(
                    "file.tree",
                    json!({"project_id":project,"worktree":optional_scope(&worktree),"path":"","depth":1,"git_badges":false}),
                )
                .await;
            if !e.matches(&ui, project, &worktree) || e.tree_revision.get() != revision {
                return;
            }
            let v = match result {
                Ok(v) => v,
                Err(err) => return ui.show_error(&readable(&err)),
            };
            // Every open folder is listed before a widget is touched, so the tree is rebuilt
            // in one go: rebuilding a level per round trip let the content collapse and the
            // view snap to the top on every refresh (RA-209).
            let mut prefetched = std::collections::BTreeMap::new();
            for folder in e.open_folders() {
                let listing = ui
                    .call(
                        "file.tree",
                        json!({"project_id":project,"worktree":optional_scope(&worktree),"path":folder,"depth":1,"git_badges":false}),
                    )
                    .await;
                if !e.matches(&ui, project, &worktree) || e.tree_revision.get() != revision {
                    return;
                }
                // A folder that cannot be listed now is fetched when it is drawn, as before.
                if let Ok(listing) = listing {
                    prefetched.insert(folder, rows(&listing, "entries"));
                }
            }
            // The badges are the Git panel's status. A file event refreshes both; a load of
            // the tree on its own (a file action, Refresh) asks for the status here, once.
            if e.status_requests.get() == statuses {
                e.refresh_badges(&ui);
            }
            let signature = {
                use std::hash::{Hash, Hasher};
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                v["entries"].to_string().hash(&mut hash);
                for (folder, entries) in &prefetched {
                    folder.hash(&mut hash);
                    for entry in entries {
                        entry.to_string().hash(&mut hash);
                    }
                }
                e.tree_limits.borrow().hash(&mut hash);
                e.path.borrow().hash(&mut hash);
                e.selected_path.borrow().hash(&mut hash);
                hash.finish()
            };
            // Nothing the explorer draws changed: the rows on screen stay, with their focus
            // and any open menu.
            if signature == e.tree_signature.get() {
                return;
            }
            let scroll = e
                .tree
                .ancestor(gtk::ScrolledWindow::static_type())
                .and_downcast::<gtk::ScrolledWindow>()
                .map(|s| s.vadjustment())
                .map(|a| (a.value(), a));
            e.fill_tree(&ui, &e.tree, "", rows(&v, "entries"), &mut prefetched);
            e.prune_tree_rows();
            e.tree_signature.set(signature);
            if let Some((value, adjustment)) = scroll {
                adjustment.set_value(value);
                glib::idle_add_local_once(move || adjustment.set_value(value));
            }
        });
    }
    /// Expanded folders whose every ancestor is expanded too: the ones a rebuild draws open.
    fn open_folders(&self) -> Vec<String> {
        let expanded = self.expanded.borrow();
        expanded
            .iter()
            .filter(|path| {
                path.match_indices('/')
                    .all(|(at, _)| expanded.contains(&path[..at]))
            })
            .cloned()
            .collect()
    }
    fn open(self: &Rc<Self>, ui: &Rc<Ui>, path: String) {
        self.read_file(ui, path, false);
    }
    pub fn verify_open(self: &Rc<Self>, ui: &Rc<Ui>) {
        self.open(ui, "README.md".into());
        assert!(self.busy.get());
        assert!(
            !self.view.is_editable() && !self.save.is_sensitive() && !self.discard.is_sensitive()
        );
    }
    pub fn verify_save(self: &Rc<Self>, ui: &Rc<Ui>) {
        assert!(!self.busy.get());
        assert_eq!(self.path.borrow().as_str(), "README.md");
        self.buffer
            .insert_at_cursor("\nNative editor save verified.\n");
        self.save_file(ui);
        assert!(self.busy.get());
        assert!(
            !self.view.is_editable() && !self.save.is_sensitive() && !self.discard.is_sensitive()
        );
    }
    fn read_file(self: &Rc<Self>, ui: &Rc<Ui>, path: String, discard: bool) {
        let goto = self.goto.take();
        let quiet = self.quiet_missing.take();
        if let Some(refusal) = self.replace_refusal(
            (!discard).then_some("Save or discard this file's changes before opening another file."),
        ) {
            ui.show_error(refusal);
            return;
        }
        if image_preview::is_image(&path) {
            self.open_image(ui, path);
            return;
        }
        self.revision.set(self.revision.get() + 1);
        self.set_busy(true);
        let revision = self.revision.get();
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let result = ui
                .call(
                    "file.read",
                    json!({"project_id":project,"worktree":optional_scope(&worktree),"path":path,"max_bytes":1048576}),
                )
                .await;
            if e.revision.get() != revision {
                return;
            }
            e.set_busy(false);
            if !e.matches(&ui, project, &worktree) {
                return;
            }
            match result {
                Ok(v) => {
                    if v["truncated"] == true {
                        ui.show_error("This file is larger than the 1 MiB editor limit, so it was not opened.");
                        return;
                    }
                    if v["text"].as_str().is_none() {
                        ui.show_error("This looks like a binary file, so it was not opened for editing.");
                        return;
                    }
                    e.diff.set(false);
                    e.diff_data.borrow_mut().take();
                    e.view.set_show_line_numbers(true);
                    e.image_mode.set(false);
                    e.image.clear();
                    e.before_scroll.set_visible(false);
                    let content = text(&v, "text");
                    e.buffer.set_text(content);
                    e.buffer.set_modified(false);
                    e.project.set(project);
                    *e.path.borrow_mut() = path.clone();
                    *e.selected_path.borrow_mut() = path.clone();
                    e.selected_directory.set(false);
                    *e.original.borrow_mut() = content.into();
                    e.disk_text.borrow_mut().take();
                    e.conflict_bar.set_visible(false);
                    e.set_busy(false);
                    let manager = sourceview5::LanguageManager::default();
                    e.buffer
                        .set_language(manager.guess_language(Some(&path), None).as_ref());
                    e.caption.set_text(&path);
                    // set_text leaves the caret after the inserted text (RA-456).
                    if let Some((line, col, length)) = goto {
                        e.reveal(line, col, length);
                    } else {
                        e.buffer.place_cursor(&e.buffer.start_iter());
                        e.view.scroll_to_mark(&e.buffer.get_insert(), 0.0, false, 0.0, 0.0);
                    }
                }
                Err(crate::client::Error::Bus(bus)) if quiet && bus.code == "file.not_found" => {}
                Err(err) => ui.show_error(&readable(&err)),
            }
        });
    }
    fn save_file(self: &Rc<Self>, ui: &Rc<Ui>) {
        self.save_then(ui, None);
    }
    /// Save, then run `then` once the file is written; it is dropped when the save is not.
    fn save_then(self: &Rc<Self>, ui: &Rc<Ui>, then: Option<AfterSave>) {
        let project = self.project.get();
        // A branch switch locks the text (`git_action`); staging or a push leaves it to be
        // edited and saved.
        if project != ui.project.get()
            || !self.buffer.is_modified()
            || self.busy.get()
            || !self.view.is_editable()
        {
            return;
        }
        let path = self.path.borrow().clone();
        let worktree = self.worktree.borrow().clone();
        let original = self.original.borrow().clone();
        // The view inserts a bare LF for Enter: keep the file's own convention (RA-457).
        let content = with_newlines(
            &self.buffer.text(&self.buffer.start_iter(), &self.buffer.end_iter(), false),
            crlf_dominant(&original),
        );
        let e = self.clone();
        let ui = ui.clone();
        self.revision.set(self.revision.get() + 1);
        let revision = self.revision.get();
        self.set_busy(true);
        self.writing.set(true);
        glib::spawn_future_local(async move {
            use sha2::Digest;
            let expected = sha2::Sha256::digest(original.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            // A held write (a protected path) is offered for confirmation here, as Git's are.
            let result = code_git::guarded(&ui, "file.write", json!({"project_id":project,"worktree":optional_scope(&worktree),"path":path,"text":content,"expected_sha256":expected})).await;
            e.writing.set(false);
            if e.revision.get() != revision {
                return;
            }
            e.set_busy(false);
            if !e.matches(&ui, project, &worktree) {
                return;
            }
            match result {
                Ok(None) => e.save.set_sensitive(true),
                Ok(Some(_)) => {
                    *e.original.borrow_mut() = content;
                    e.buffer.set_modified(false);
                    e.disk_text.borrow_mut().take();
                    e.conflict_bar.set_visible(false);
                    e.refresh_git(&ui);
                    if let Some(then) = then {
                        then(&e, &ui);
                    }
                }
                Err(err) => {
                    ui.show_error(&readable(&err));
                    e.save.set_sensitive(true);
                    // The file changed under the draft: offer reload or keep-and-overwrite
                    // instead of a Save that fails the same way again (RA-208).
                    if matches!(&err, crate::client::Error::Bus(bus) if bus.code == "file.edit_conflict")
                    {
                        e.check_disk(&ui);
                    }
                }
            }
        });
    }
}

impl Editor {
    /// Put the caret on a 1-based line and byte column, select `length` characters and scroll
    /// there. The column is a byte offset, cut back to a character boundary.
    fn reveal(&self, line: u32, col: u32, length: usize) {
        let Some(mut start) = self.buffer.iter_at_line(line.saturating_sub(1) as i32) else {
            return;
        };
        let mut line_end = start;
        if !line_end.ends_line() {
            line_end.forward_to_line_end();
        }
        let text = self.buffer.text(&start, &line_end, true);
        let mut byte = (col.saturating_sub(1) as usize).min(text.len());
        while !text.is_char_boundary(byte) {
            byte -= 1;
        }
        start.forward_chars(text[..byte].chars().count() as i32);
        let mut end = start;
        end.forward_chars(length as i32);
        if end > line_end {
            end = line_end;
        }
        self.buffer.select_range(&start, &end);
        self.view.grab_focus();
        // The view may not have its size yet right after the text was set.
        let view = self.view.clone();
        let mark = self.buffer.get_insert();
        glib::idle_add_local_once(move || view.scroll_to_mark(&mark, 0.1, true, 0.0, 0.3));
    }
}

/// At most this many characters of a search hit's line are drawn, around the match.
const SNIPPET_CHARS: usize = 240;

/// The part of `line` around the 1-based byte column `col` that a search row shows: a hit in a
/// minified bundle or source map is one line of megabytes, which no label should shape.
fn snippet(line: &str, col: u32) -> String {
    let mut at = (col.saturating_sub(1) as usize).min(line.len());
    while !line.is_char_boundary(at) {
        at -= 1;
    }
    // A quarter of the room is context before the match.
    let start = line[..at]
        .char_indices()
        .rev()
        .nth(SNIPPET_CHARS / 4 - 1)
        .map_or(0, |(i, _)| i);
    let end = line[start..]
        .char_indices()
        .nth(SNIPPET_CHARS)
        .map_or(line.len(), |(i, _)| start + i);
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        line[start..end].trim(),
        if end < line.len() { "…" } else { "" }
    )
}

/// Whether CRLF ends more of `text`'s lines than a bare LF does.
fn crlf_dominant(text: &str) -> bool {
    let crlf = text.matches("\r\n").count();
    crlf > text.matches('\n').count() - crlf
}

/// `text` with every CRLF or bare LF written as `crlf` asks; a lone CR is left alone.
fn with_newlines(text: &str, crlf: bool) -> String {
    let lf = text.replace("\r\n", "\n");
    if crlf { lf.replace('\n', "\r\n") } else { lf }
}

/// One explorer row, kept across refreshes (`fill_tree`).
struct TreeRow {
    /// What sits in the parent folder's box: a file's row, or a folder's row and children.
    widget: gtk::Widget,
    row: gtk::Button,
    name: gtk::Label,
    content: gtk::Box,
    directory: bool,
    /// A file's size and modification time, which its image preview is keyed by: a file
    /// whose stamp changed gets a new row.
    stamp: String,
    /// The status letter and folder tint drawn now.
    painted: Cell<Option<(Option<&'static str>, Option<&'static str>)>>,
    folder: Option<TreeFolder>,
}

struct TreeFolder {
    children: gtk::Box,
    /// The children box holds this folder's listing (or one is on its way).
    loaded: Rc<Cell<bool>>,
    /// Open or close the folder, listing it when it opens unloaded.
    toggle: Rc<dyn Fn(bool)>,
}

/// The explorer's status letters, from `git.status`.
#[derive(Default)]
pub(super) struct TreeBadges {
    files: std::collections::HashMap<String, String>,
    /// Folders holding nothing tracked, which `git.status` lists whole as `dir/`: every file
    /// under one is untracked.
    untracked: Vec<String>,
}

impl TreeBadges {
    fn from_status(files: &[Value]) -> Self {
        let mut badges = Self::default();
        for file in files {
            // The worktree's code when there is one, else the index's: as file.tree badges.
            let code = match text(file, "worktree").trim() {
                "" => text(file, "index").trim(),
                code => code,
            };
            if code.is_empty() {
                continue;
            }
            let path = text(file, "path");
            if path.ends_with('/') {
                if code == "?" {
                    badges.untracked.push(path.to_owned());
                }
                continue;
            }
            badges.files.insert(path.to_owned(), code.to_owned());
        }
        badges
    }
    /// A path's status code, or "" for none. Only files inside an untracked folder are
    /// untracked themselves; the folders under it are tinted by `folder_status`.
    fn badge(&self, path: &str, directory: bool) -> &str {
        if let Some(code) = self.files.get(path) {
            return code;
        }
        if !directory && self.untracked.iter().any(|folder| path.starts_with(folder.as_str())) {
            return "?";
        }
        ""
    }
}

const STATUS_CLASSES: [&str; 7] = [
    "git-modified", "git-added", "git-untracked", "git-deleted", "git-renamed", "git-conflict", "git-ignored",
];

/// Make `target`'s children exactly `wanted`, in order, moving only what is out of place.
fn sync_children(target: &gtk::Box, wanted: &[gtk::Widget]) {
    let keep: std::collections::HashSet<usize> = wanted.iter().map(|w| w.as_ptr() as usize).collect();
    let mut child = target.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if !keep.contains(&(widget.as_ptr() as usize)) {
            target.remove(&widget);
        }
    }
    let parent: &gtk::Widget = target.upcast_ref();
    let mut previous: Option<gtk::Widget> = None;
    for widget in wanted {
        if widget.parent().as_ref() != Some(parent) {
            target.insert_child_after(widget, previous.as_ref());
        } else if widget.prev_sibling() != previous {
            target.reorder_child_after(widget, previous.as_ref());
        }
        previous = Some(widget.clone());
    }
}

fn optional_scope(worktree: &str) -> Option<&str> {
    if worktree.is_empty() {
        None
    } else {
        Some(worktree)
    }
}

impl Editor {
    /// Show `entries` in `target`, reusing the rows already there by path: a refresh that
    /// changed one entry touches one row, and the rest keep their state and stay where they are.
    /// An expanded folder's listing in `prefetched` fills it the same way; one without is left
    /// as it is.
    fn fill_tree(
        self: &Rc<Self>,
        ui: &Rc<Ui>,
        target: &gtk::Box,
        directory: &str,
        entries: Vec<Value>,
        prefetched: &mut std::collections::BTreeMap<String, Vec<Value>>,
    ) {
        let mut wanted: Vec<gtk::Widget> = Vec::new();
        if entries.is_empty() {
            wanted.push(label("Empty folder", "tree-empty").upcast());
        }
        // Each row is a dozen widgets and controllers: a folder of thousands is drawn a page
        // at a time (RA-210).
        let limit = self.tree_limits.borrow().get(directory).copied().unwrap_or(TREE_PAGE);
        let hidden = entries.len().saturating_sub(limit);
        for entry in entries.into_iter().take(limit) {
            let path = text(&entry, "path").to_string();
            let is_folder = match text(&entry, "kind") {
                "dir" => true,
                "symlink" => self.links_to_folder(ui, &path),
                _ => false,
            };
            let stamp = if is_folder { String::new() } else { format!("{}:{}", entry["size"], entry["modified_at"]) };
            let kept = self.tree_rows.borrow().get(&path).cloned().filter(|row| {
                row.directory == is_folder
                    && row.stamp == stamp
                    && row.widget.parent().as_ref() == Some(target.upcast_ref::<gtk::Widget>())
            });
            let row = match kept {
                Some(row) => row,
                None => {
                    let row = self.tree_row(ui, &entry, &path, is_folder, stamp);
                    self.tree_rows.borrow_mut().insert(path.clone(), row.clone());
                    row
                }
            };
            self.paint_row(&row, &path);
            if path == *self.path.borrow() || path == *self.selected_path.borrow() {
                project_files::mark_selected(&row.row);
            } else {
                row.row.remove_css_class("selected");
            }
            if let Some(folder) = &row.folder {
                if self.expanded.borrow().contains(&path) {
                    if let Some(entries) = prefetched.remove(&path) {
                        folder.loaded.set(true);
                        self.fill_tree(ui, &folder.children, &path, entries, prefetched);
                    }
                    if !folder.children.is_visible() {
                        (folder.toggle)(true);
                    }
                } else if folder.children.is_visible() || folder.loaded.get() {
                    (folder.toggle)(false);
                }
            }
            wanted.push(row.widget.clone());
        }
        if hidden > 0 {
            let more = button("", "tree-row");
            more.set_widget_name("project-files-more");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            let depth = if directory.is_empty() { 0 } else { directory.matches('/').count() + 1 };
            if depth > 0 {
                content.append(&project_files::indent_guides(depth));
            }
            let caption = label(
                &format!("Show {} more ({hidden} hidden)", hidden.min(TREE_PAGE)),
                "tree-empty",
            );
            caption.set_margin_start(project_files::INDENT + 4);
            content.append(&caption);
            more.set_child(Some(&content));
            let e = self.clone();
            let weak = Rc::downgrade(ui);
            let folder = directory.to_owned();
            more.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    e.tree_limits.borrow_mut().insert(folder.clone(), limit + TREE_PAGE);
                    e.load_tree(&ui);
                }
            });
            wanted.push(more.upcast());
        }
        sync_children(target, &wanted);
    }

    /// One explorer row, VS Code's anatomy: guides, chevron, type icon, name, status letter.
    /// A folder's row comes with the box its children are drawn in, filled when it opens.
    fn tree_row(self: &Rc<Self>, ui: &Rc<Ui>, entry: &Value, path: &str, directory: bool, stamp: String) -> Rc<TreeRow> {
        let path = path.to_owned();
        let depth = path.matches('/').count();
        let row = button("", "tree-row");
        row.add_css_class(if directory { "code-folder" } else { "code-file" });
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        if depth > 0 {
            content.append(&project_files::indent_guides(depth));
        }
        let chevron = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        chevron.add_css_class("tree-chevron");
        chevron.set_size_request(project_files::INDENT + 4, -1);
        // The arrow inside expands to centre itself; stop that reaching the row, or the
        // chevron shares the spare width with the name and the folder drifts right.
        chevron.set_hexpand(false);
        content.append(&chevron);
        let glyph = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        glyph.add_css_class("tree-glyph");
        content.append(&glyph);
        let name = label(text(entry, "name"), "tree-name");
        name.set_hexpand(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        content.append(&name);
        row.set_child(Some(&content));
        self.bind_tree_row(ui, &row, &path, directory);
        if !directory {
            glyph.append(&project_files::file_image(&path, 14));
            self.bind_image_hover(ui, &row, &path, stamp.clone(), entry["size"].as_u64());
            let e = self.clone();
            let weak = Rc::downgrade(ui);
            let open = path.clone();
            row.connect_clicked(move |row| {
                if let Some(ui) = weak.upgrade() {
                    project_files::mark_selected(row);
                    *e.selected_path.borrow_mut() = open.clone();
                    e.selected_directory.set(false);
                    e.open_path(&ui, open.clone());
                }
            });
            return Rc::new(TreeRow {
                widget: row.clone().upcast(),
                row,
                name,
                content,
                directory,
                stamp,
                painted: Cell::new(None),
                folder: None,
            });
        }
        let children = gtk::Box::new(gtk::Orientation::Vertical, 0);
        children.set_visible(false);
        // A drop on any row inside this folder lands in this folder, not the root.
        self.bind_folder_drop(ui, &children, &path);
        let node = gtk::Box::new(gtk::Orientation::Vertical, 0);
        node.append(&row);
        node.append(&children);
        let paint = move |open: bool| {
            clear(&chevron);
            clear(&glyph);
            let arrow = crate::icons::image(if open { "chevron-down" } else { "chevron-right" }, 12);
            arrow.set_halign(gtk::Align::Center);
            arrow.set_hexpand(true);
            chevron.append(&arrow);
            let folder = crate::icons::image(if open { "folder-open" } else { "folder" }, 14);
            folder.add_css_class("file-icon");
            folder.add_css_class("ft-folder");
            glyph.append(&folder);
        };
        paint(false);
        let loaded = Rc::new(Cell::new(false));
        let e = Rc::downgrade(self);
        let weak = Rc::downgrade(ui);
        let (shown, done, folder) = (children.downgrade(), loaded.clone(), path.clone());
        let toggle: Rc<dyn Fn(bool)> = Rc::new(move |open: bool| {
            let (Some(e), Some(children)) = (e.upgrade(), shown.upgrade()) else { return };
            children.set_visible(open);
            paint(open);
            if !open {
                e.expanded.borrow_mut().remove(&folder);
                // Listed afresh when reopened: a refresh with the same listing
                // keeps these rows, so what they held could be stale by then.
                done.set(false);
                clear(&children);
                return;
            }
            e.expanded.borrow_mut().insert(folder.clone());
            if done.replace(true) {
                return;
            }
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let project = ui.project.get();
            let worktree = e.worktree.borrow().clone();
            let payload = e.payload(&ui, json!({"path":folder,"depth":1,"git_badges":false}));
            let (loaded, folder) = (done.clone(), folder.clone());
            glib::spawn_future_local(async move {
                let result = ui.call("file.tree", payload).await;
                if !e.matches(&ui, project, &worktree) || !loaded.get() {
                    return;
                }
                // A close and reopen while this ran asked again; the last answer wins.
                match result {
                    Ok(v) => {
                        e.fill_tree(&ui, &children, &folder, rows(&v, "entries"), &mut Default::default());
                    }
                    Err(err) => {
                        clear(&children);
                        loaded.set(false);
                        ui.show_error(&readable(&err));
                    }
                }
            });
        });
        let click = toggle.clone();
        let select = path.clone();
        let e = Rc::downgrade(self);
        row.connect_clicked(move |row| {
            let Some(e) = e.upgrade() else { return };
            project_files::mark_selected(row);
            *e.selected_path.borrow_mut() = select.clone();
            e.selected_directory.set(true);
            let open = !e.expanded.borrow().contains(&select);
            click(open);
        });
        Rc::new(TreeRow {
            widget: node.upcast(),
            row,
            name,
            content,
            directory,
            stamp,
            painted: Cell::new(None),
            folder: Some(TreeFolder { children, loaded, toggle }),
        })
    }

    /// Tint a row and letter it with its status from the last `git.status`: a changed file by
    /// its own code, a folder by the strongest change under it.
    fn paint_row(&self, row: &TreeRow, path: &str) {
        let status = project_files::status_letter(self.tree_badges.borrow().badge(path, row.directory));
        let folder = if status.is_none() && row.directory { project_files::folder_status(path) } else { None };
        let painted = (status.map(|(letter, ..)| letter), folder);
        // A folder's tint is also redrawn by `note_changes`, behind this cache's back.
        if !row.directory && row.painted.get() == Some(painted) {
            return;
        }
        row.painted.set(Some(painted));
        for class in STATUS_CLASSES {
            row.name.remove_css_class(class);
        }
        if let Some(class) = status.map(|(_, class, _)| class).or(folder) {
            row.name.add_css_class(class);
        }
        if let Some(mark) = row.content.last_child().filter(|mark| mark.has_css_class("tree-badge")) {
            row.content.remove(&mark);
        }
        if let Some((letter, class, _)) = status {
            let mark = label(letter, "tree-badge");
            mark.add_css_class(class);
            row.content.append(&mark);
        }
        row.row.set_tooltip_text(Some(&match status {
            Some((_, _, word)) => format!("{path} · {word}"),
            None => path.to_owned(),
        }));
    }

    /// Take the Git panel's latest `git.status` as the explorer's badges, and repaint the rows
    /// on screen whose status changed. Called after `note_changes`, whose folder tints it reads.
    pub(super) fn note_badges(&self, files: &[Value]) {
        *self.tree_badges.borrow_mut() = TreeBadges::from_status(files);
        let rows: Vec<(String, Rc<TreeRow>)> =
            self.tree_rows.borrow().iter().map(|(path, row)| (path.clone(), row.clone())).collect();
        for (path, row) in rows {
            if row.widget.parent().is_some() {
                self.paint_row(&row, &path);
            }
        }
    }

    /// Fetch `git.status` for the explorer's tints alone, as the hidden Git panel does.
    fn refresh_badges(self: &Rc<Self>, ui: &Rc<Ui>) {
        self.status_requests.set(self.status_requests.get() + 1);
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let payload = self.payload(ui, json!({}));
        let (e, ui) = (self.clone(), ui.clone());
        glib::spawn_future_local(async move {
            let Ok(status) = ui.call("git.status", payload).await else { return };
            if e.matches(&ui, project, &worktree) {
                let files = rows(&status, "files");
                e.note_changes(&files);
                e.note_badges(&files);
            }
        });
    }

    /// Forget rows a refresh or a collapse took out of the explorer.
    fn prune_tree_rows(&self) {
        let tree: &gtk::Widget = self.tree.upcast_ref();
        self.tree_rows.borrow_mut().retain(|_, row| row.widget.is_ancestor(tree));
    }
    /// A symlink to a folder inside this checkout, which file.tree lists and file.read
    /// refuses: file.tree reports a link without what it points at (RA-459).
    fn links_to_folder(&self, ui: &Ui, path: &str) -> bool {
        let root = match self.worktree.borrow().as_str() {
            "" => ui.projects.borrow().iter().find(|p| p["id"] == ui.project.get()).map(|p| text(p, "path").to_owned()),
            worktree => Some(worktree.to_owned()),
        };
        let Some(root) = root.and_then(|root| std::fs::canonicalize(root).ok()) else {
            return false;
        };
        std::fs::canonicalize(root.join(path)).is_ok_and(|target| target.starts_with(&root) && target.is_dir())
    }
    fn matches(&self, ui: &Ui, project: i64, worktree: &str) -> bool {
        ui.project.get() == project && self.worktree.borrow().as_str() == worktree
    }
    fn payload(&self, ui: &Ui, mut extra: Value) -> Value {
        extra["project_id"] = json!(ui.project.get());
        if !self.worktree.borrow().is_empty() {
            extra["worktree"] = json!(*self.worktree.borrow());
        }
        extra
    }
    /// A file clicked in the explorer. Switching checkout is `select_checkout`'s job.
    fn open_path(self: &Rc<Self>, ui: &Rc<Ui>, path: String) {
        // The file already open is brought forward as it is: reading it again was refused
        // with edits, and put the caret back on line 1 without them. One still loading is
        // read again, as another file would be.
        if *self.path.borrow() == path
            && !self.diff.get()
            && !self.image_mode.get()
            && (!self.busy.get() || self.writing.get())
        {
            self.show_files();
            self.view.grab_focus();
            return;
        }
        self.after_unsaved(ui, move |e, ui| e.open(ui, path));
    }
    fn refresh_scopes(self: &Rc<Self>, ui: &Rc<Ui>) {
        self.scope_project.set(ui.project.get());
        self.scope_revision.set(self.scope_revision.get() + 1);
        let revision = self.scope_revision.get();
        let project = ui.project.get();
        if project == 0 {
            // No project (a fresh install, or one used for Threads alone): no worktrees to list,
            // where asking showed "NotFound project.not_found: no project 0" on every start.
            self.worktrees.borrow_mut().clear();
            self.update_scope_label(ui);
            return;
        }
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let result = ui
                .call(
                    "worktree.list",
                    json!({"project_id":project,"include_dirty":false}),
                )
                .await;
            if project != ui.project.get() || revision != e.scope_revision.get() {
                return;
            }
            match result {
                Ok(v) => {
                    let items = rows(&v, "worktrees");
                    *e.worktrees.borrow_mut() = items;
                    e.update_scope_label(&ui);
                }
                Err(err) => ui.show_error(&readable(&err)),
            }
        });
    }
    fn bind_controls(self: &Rc<Self>, ui: &Rc<Ui>) {
        // Whatever shows a panel (its key, a layout restore, a page switch), a refresh that
        // was skipped while it was hidden runs now.
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        self.file_sidebar.connect_map(move |_| {
            if let Some(ui) = weak.upgrade().filter(|_| e.tree_stale.replace(false)) {
                if e.search.text().trim().is_empty() {
                    e.load_tree(&ui);
                }
            }
        });
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        self.git.connect_map(move |_| {
            if let Some(ui) = weak.upgrade().filter(|_| e.git_stale.replace(false)) {
                e.refresh_git(&ui);
            }
        });
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        self.scope.set_create_popup_func(move |button| {
            if let Some(ui) = weak.upgrade() {
                e.scope_picker(&ui, button);
            }
        });
        self.bind_folder_drop(ui, &self.tree, "");
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        self.search.connect_activate(move |_| {
            if let Some(ui) = weak.upgrade() {
                e.run_search(&ui);
            }
        });
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        self.search.connect_stop_search(move |search| {
            search.set_text("");
            if let Some(ui) = weak.upgrade() {
                e.load_tree(&ui);
            }
        });
        // The field's clear icon empties it as Escape does: the files come back in place of
        // the results. A tree on screen (a signature drawn) or on its way is left alone.
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        self.search.connect_search_changed(move |search| {
            let results = e.tree_signature.get() == 0 && !e.tree_load_pending.get();
            if let Some(ui) = weak.upgrade().filter(|_| results && search.text().trim().is_empty()) {
                e.load_tree(&ui);
            }
        });
        for (icon, title, op) in [
            ("file-plus", "New file or folder", "file.create"),
            ("edit", "Rename the selected file", "file.rename"),
            ("trash", "Move the selected file to Relay trash", "file.delete"),
        ] {
            let action = crate::app::icon_button(icon, title);
            action.add_css_class("explorer-key");
            action.set_widget_name(&format!("project-{op}"));
            let e = self.clone();
            let weak = Rc::downgrade(ui);
            action.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    e.file_action(&ui, op);
                }
            });
            self.file_actions.append(&action);
        }
        let refresh = crate::app::icon_button("refresh", "Refresh the explorer");
        refresh.add_css_class("explorer-key");
        refresh.set_widget_name("project-files-refresh");
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        refresh.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                e.load_tree(&ui);
                e.refresh_git(&ui);
            }
        });
        self.file_actions.append(&refresh);
        let trash = crate::app::icon_button("history", "Recently trashed files");
        trash.add_css_class("explorer-key");
        trash.set_widget_name("project-files-trash");
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        trash.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                e.show_trash(&ui);
            }
        });
        self.file_actions.append(&trash);
        let collapse = crate::app::icon_button("collapse", "Collapse folders");
        collapse.add_css_class("explorer-key");
        collapse.set_widget_name("project-files-collapse");
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        collapse.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                e.expanded.borrow_mut().clear();
                e.load_tree(&ui);
            }
        });
        self.file_actions.append(&collapse);
        let keys = gtk::EventControllerKey::new();
        let e = self.clone();
        let weak = Rc::downgrade(ui);
        keys.connect_key_pressed(move |_, key, keycode, state| {
            use gtk::gdk::ModifierType as M;
            // As Notes reads its keys: Caps Lock gives S, and a Cyrillic layout another letter.
            let key = crate::shortcuts::latin(key, keycode).to_lower();
            let mods = state & (M::CONTROL_MASK | M::SHIFT_MASK | M::ALT_MASK | M::SUPER_MASK | M::META_MASK);
            if mods == M::CONTROL_MASK {
                if key == gtk::gdk::Key::s {
                    // Only a document's buffer is saved: with a design (focus in its inspector,
                    // layers or text editor, which the canvas never sees), an image or the agents
                    // showing, the file behind them is not touched (TM-70).
                    if e.design_visible() || !e.document_tools.is_visible() {
                        return glib::Propagation::Proceed;
                    }
                    if let Some(ui) = weak.upgrade() {
                        e.save_file(&ui);
                    }
                    return glib::Propagation::Stop;
                }
                // Find, and Replace with its row, while a document shows (not an image, the
                // agents or a design).
                if key == gtk::gdk::Key::f || key == gtk::gdk::Key::h {
                    if !e.document_tools.is_visible() { return glib::Propagation::Proceed; }
                    e.find.open(key == gtk::gdk::Key::h);
                    return glib::Propagation::Stop;
                }
            }
            // F3 and Shift+F3 step through the matches with the bar open or closed, as in Notes.
            if key == gtk::gdk::Key::F3 && (mods.is_empty() || mods == M::SHIFT_MASK) && e.document_tools.is_visible() {
                e.find.step(mods == M::SHIFT_MASK);
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Escape && e.find.visible() {
                e.find.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.root.add_controller(keys);
    }
    fn run_search(self: &Rc<Self>, ui: &Rc<Ui>) {
        let query = self.search.text().to_string();
        if query.trim().is_empty() {
            self.load_tree(ui);
            return;
        }
        if self.search_pending.replace(true) {
            self.search_dirty.set(true);
            return;
        }
        self.tree_revision.set(self.tree_revision.get() + 1);
        let revision = self.tree_revision.get();
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let payload = self.payload(ui, json!({"query":query,"limit":100,"regex":false}));
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let result = ui.call("file.search", payload).await;
            e.search_pending.set(false);
            // Something changed while this search ran: its answer is already stale.
            if e.search_dirty.replace(false) {
                e.run_search(&ui);
                return;
            }
            if !e.matches(&ui, project, &worktree) || e.tree_revision.get() != revision {
                return;
            }
            match result {
                Ok(v) => {
                    clear(&e.tree);
                    e.tree_signature.set(0);
                    let hits = rows(&v, "hits");
                    e.tree.append(&label(
                        &format!(
                            "{} match{}{}",
                            hits.len(),
                            if hits.len() == 1 { "" } else { "es" },
                            if hits.len() == 100 {
                                " (limit reached)"
                            } else {
                                ""
                            }
                        ),
                        "tree-empty",
                    ));
                    for hit in hits {
                        let path = text(&hit, "path").to_string();
                        let b = button("", "tree-row");
                        b.add_css_class("search-hit");
                        let card = gtk::Box::new(gtk::Orientation::Vertical, 1);
                        let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                        head.append(&project_files::file_image(&path, 14));
                        let (directory, file) = match path.rsplit_once('/') {
                            Some((directory, file)) => (directory, file),
                            None => ("", path.as_str()),
                        };
                        head.append(&label(file, "tree-name"));
                        let place = label(&if directory.is_empty() {
                            format!("line {}", hit["line"])
                        } else {
                            format!("{directory} · line {}", hit["line"])
                        }, "tree-dir");
                        place.set_hexpand(true);
                        place.set_ellipsize(gtk::pango::EllipsizeMode::Start);
                        head.append(&place);
                        card.append(&head);
                        let line = hit["line"].as_u64().unwrap_or(1) as u32;
                        let col = hit["col"].as_u64().unwrap_or(1) as u32;
                        let shown = snippet(text(&hit, "text"), col);
                        let snippet = label(&shown, "search-snippet");
                        snippet.set_ellipsize(gtk::pango::EllipsizeMode::End);
                        card.append(&snippet);
                        b.set_child(Some(&card));
                        b.set_tooltip_text(Some(&shown));
                        let ed = e.clone();
                        let weak = Rc::downgrade(&ui);
                        let length = query.chars().count();
                        b.connect_clicked(move |_| {
                            if let Some(ui) = weak.upgrade() {
                                // The open document is only moved within: re-reading it
                                // would be refused while it has edits.
                                if *ed.path.borrow() == path
                                    && !ed.diff.get()
                                    && !ed.image_mode.get()
                                    && !ed.busy.get()
                                {
                                    ed.show_files();
                                    ed.reveal(line, col, length);
                                    return;
                                }
                                let path = path.clone();
                                ed.after_unsaved(&ui, move |ed, ui| {
                                    ed.goto.set(Some((line, col, length)));
                                    ed.open(ui, path);
                                });
                            }
                        });
                        e.tree.append(&b);
                    }
                }
                Err(err) => ui.show_error(&readable(&err)),
            }
        });
    }
    fn file_action(self: &Rc<Self>, ui: &Rc<Ui>, op: &'static str) {
        if self.is_occupied() {
            ui.show_error("Save or discard the current file before changing files.");
            return;
        }
        let path = if self.selected_path.borrow().is_empty() {
            self.path.borrow().clone()
        } else {
            self.selected_path.borrow().clone()
        };
        if op != "file.create" && path.is_empty() {
            ui.show_error("Select a file or folder first.");
            return;
        }
        // The open document is a file; a selected row says what it is.
        let is_folder = !self.selected_path.borrow().is_empty() && self.selected_directory.get();
        let title = match op {
            "file.create" => "Create file",
            "file.rename" if is_folder => "Rename folder",
            "file.rename" => "Rename file",
            _ if is_folder => "Move folder to Relay trash",
            _ => "Move file to Relay trash",
        };
        let dialog = crate::panel::Panel::new(ui, title, 480);
        let caption = match op {
            "file.create" => "Create",
            "file.rename" => "Rename",
            _ => "Move to trash",
        };
        let entry = gtk::Entry::new();
        entry.set_widget_name("project-file-path");
        entry.set_placeholder_text(Some(if op == "file.create" {
            "relative/path.rs"
        } else if is_folder {
            "new-name"
        } else {
            "new-name.rs"
        }));
        if op == "file.create" && !path.is_empty() {
            let directory = if self.selected_directory.get() {
                path.clone()
            } else {
                std::path::Path::new(&path)
                    .parent()
                    .unwrap_or(std::path::Path::new(""))
                    .to_string_lossy()
                    .to_string()
            };
            if !directory.is_empty() {
                entry.set_text(&format!("{directory}/"));
                entry.set_position(-1);
            }
        }
        let folder = gtk::CheckButton::with_label("Create a folder");
        folder.set_widget_name("project-file-folder");
        if op == "file.delete" {
            let copy = label(
                &format!(
                    "{path}\nRelay keeps a copy inside this checkout. Restore it from Recently trashed in the explorer; removing the checkout removes the copy."
                ),
                "body",
            );
            copy.set_wrap(true);
            dialog.body.append(&copy);
        } else {
            if op == "file.rename" {
                let name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("");
                entry.set_text(name);
                // The name is selected without its extension, so typing replaces just that.
                let stem = name.rfind('.').filter(|&at| at > 0).map_or(name, |at| &name[..at]);
                entry.select_region(0, stem.chars().count() as i32);
            }
            dialog.body.append(&entry);
            if op == "file.create" {
                dialog.body.append(&folder);
            }
        }
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let accepted = dialog.response(caption).await;
            if !accepted || !e.matches(&ui, project, &worktree) || e.is_occupied() {
                return;
            }
            let value = entry.text().trim().to_string();
            if op != "file.delete" && value.is_empty() {
                ui.show_error("Enter a file name.");
                return;
            }
            let extra = match op {
                "file.create" => {
                    json!({"path":value,"kind":if folder.is_active(){"dir"}else{"file"},"text":""})
                }
                "file.rename" => json!({"path":path,"new_name":value}),
                _ => json!({"path":path}),
            };
            e.set_locked(true);
            e.writing.set(true);
            let result = code_git::guarded(&ui, op, e.payload(&ui, extra)).await;
            e.writing.set(false);
            e.set_locked(false);
            if !e.matches(&ui, project, &worktree) {
                return;
            }
            match result {
                Ok(None) => {}
                Ok(Some(v)) => {
                    *e.selected_path.borrow_mut() = if op == "file.delete" {
                        String::new()
                    } else {
                        text(&v, "path").to_owned()
                    };
                    e.selected_directory.set(text(&v, "kind") == "dir");
                    match op {
                        "file.create" => {
                            e.load_tree(&ui);
                            e.refresh_git(&ui);
                        }
                        "file.rename" => e.follow_change(&ui, &path, Some(text(&v, "path"))),
                        _ => e.follow_change(&ui, &path, None),
                    }
                    if op == "file.delete" {
                        // The ID is also kept by the engine; no permanent deletion is offered here.
                        // One key, for the latest trash; older ones are in Recently trashed (RA-460).
                        let id = v["trash_id"].as_i64().unwrap_or(0);
                        e.undo_trash.set(id);
                        clear(&e.file_undo);
                        let restore = button(&format!("Restore {path}"), "quiet");
                        restore.set_widget_name("project-file-undo");
                        restore.set_tooltip_text(Some(&format!("Restore {path} from Relay trash")));
                        if let Some(caption) = restore.child().and_downcast::<gtk::Label>() {
                            caption.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                        }
                        let weak = Rc::downgrade(&ui);
                        let ed = e.clone();
                        restore.connect_clicked(move |button| {
                            let Some(ui) = weak.upgrade() else {
                                return;
                            };
                            if ui.project.get() != project {
                                return;
                            }
                            button.set_sensitive(false);
                            let button = button.clone();
                            let ed = ed.clone();
                            glib::spawn_future_local(async move {
                                match restore_trash(&ui, project, id).await {
                                    Ok(_) => {
                                        if button.parent().as_ref() == Some(ed.file_undo.upcast_ref()) {
                                            ed.file_undo.remove(&button);
                                        }
                                        ed.load_tree(&ui);
                                        ed.refresh_git(&ui);
                                    }
                                    Err(err) => {
                                        button.set_sensitive(true);
                                        ui.show_error(&readable(&err));
                                    }
                                }
                            });
                        });
                        e.file_undo.append(&restore);
                    } else if op == "file.create" && text(&v, "kind") != "dir" {
                        e.open(&ui, text(&v, "path").to_string());
                    }
                }
                Err(err) => ui.show_error(&readable(&err)),
            }
        });
    }
    /// After `old` was renamed or moved to `new`, or trashed (`None`): the open document
    /// follows it, closes only when it was itself removed, and is left alone when the change
    /// was elsewhere. Open folders follow too, and the agents view stays up (RA-215).
    fn follow_change(self: &Rc<Self>, ui: &Rc<Ui>, old: &str, new: Option<&str>) {
        let agents = self.agents_visible();
        // None: not under `old`. Some(None): removed. Some(Some(path)): now at `path`.
        let remap = |item: &str| {
            item.strip_prefix(old)
                .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
                .map(|suffix| new.map(|new| format!("{new}{suffix}")))
        };
        let expanded = self
            .expanded
            .borrow()
            .iter()
            .filter_map(|item| remap(item).unwrap_or_else(|| Some(item.clone())))
            .collect();
        *self.expanded.borrow_mut() = expanded;
        let document = self.path.borrow().clone();
        match remap(&document).filter(|_| !document.is_empty()) {
            Some(Some(moved)) => {
                // A renamed or moved file reopens where its caret was.
                let caret = (!self.diff.get() && !self.image_mode.get()).then(|| self.caret());
                self.clear_document();
                self.goto.set(caret);
                self.open(ui, moved);
            }
            Some(None) => self.clear_document(),
            None => {}
        }
        self.load_tree(ui);
        self.refresh_git(ui);
        if agents {
            self.show_agents();
        }
    }
    /// Dropping a tree item on `widget` moves it into the folder `into`. A drop on the item's
    /// own folder is no move at all, and a drop into itself is refused.
    pub(super) fn bind_folder_drop(self: &Rc<Self>, ui: &Rc<Ui>, widget: &impl IsA<gtk::Widget>, into: &str) {
        let drop = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
        let ed = self.clone();
        let weak = Rc::downgrade(ui);
        let into = into.to_owned();
        drop.connect_drop(move |_, value, _, _| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let Some(data) = value
                .get::<String>()
                .ok()
                .and_then(|data| serde_json::from_str::<Value>(&data).ok())
            else {
                return false;
            };
            if data["project"] != ui.project.get() || data["worktree"] != *ed.worktree.borrow() {
                return false;
            }
            // Refused, the row snaps back to where it was: say why.
            if ed.is_occupied() {
                ui.show_error(if ed.buffer.is_modified() && !ed.busy.get() {
                    "Save or discard the current file before moving files."
                } else {
                    "Wait for the save or Git action to finish."
                });
                return false;
            }
            let Some(path) = data["path"].as_str().filter(|path| !path.is_empty()) else {
                return false;
            };
            let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
            if into == path || into == parent || into.starts_with(&format!("{path}/")) {
                return false;
            }
            let payload = ed.payload(&ui, json!({"path":path,"into":into}));
            let moved = path.to_owned();
            let (project, worktree) = (ui.project.get(), ed.worktree.borrow().clone());
            let ed = ed.clone();
            // Locked, not just busy: the view is read-only while the move is queued (RA-499).
            ed.set_locked(true);
            ed.writing.set(true);
            glib::spawn_future_local(async move {
                let result = code_git::guarded(&ui, "file.move", payload).await;
                ed.writing.set(false);
                ed.set_locked(false);
                if !ed.matches(&ui, project, &worktree) {
                    return;
                }
                match result {
                    Ok(None) => {}
                    // Edits made meanwhile are not thrown away by reopening the document.
                    Ok(Some(_)) if ed.buffer.is_modified() => {
                        ed.load_tree(&ui);
                        ed.refresh_git(&ui);
                    }
                    Ok(Some(value)) => ed.follow_change(&ui, &moved, Some(text(&value, "path"))),
                    Err(error) => ui.show_error(&readable(&error)),
                }
            });
            true
        });
        widget.add_controller(drop);
    }
    /// Files trashed in this project that are still in Relay trash, each with Restore. The
    /// audit log is the listing: no op lists `file_trash`, and `audit.undo` marks a restored
    /// row so it leaves this list.
    fn show_trash(self: &Rc<Self>, ui: &Rc<Ui>) {
        let Some(panel) = crate::panel::Panel::toggle(ui, "Recently trashed", 480) else {
            return;
        };
        let note = label(
            "Files moved to Relay trash in this project, newest first. Each copy is kept in the checkout it was deleted from.",
            "dim",
        );
        note.set_wrap(true);
        note.set_xalign(0.0);
        panel.body.append(&note);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
        panel.body.append(&list);
        panel.present();
        let project = ui.project.get();
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let result = ui.call("audit.list", trash_query(project)).await;
            if ui.project.get() != project {
                return;
            }
            let items = match result {
                Ok(v) => trashed(&v),
                Err(err) => return list.append(&label(&readable(&err), "dim")),
            };
            if items.is_empty() {
                list.append(&label("Nothing from this project is in Relay trash.", "dim"));
            }
            for row in items {
                let item = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
                words.set_hexpand(true);
                let name = label(text(&row["payload"], "path"), "body");
                name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                name.set_xalign(0.0);
                words.append(&name);
                let checkout = row["payload"]["worktree"]
                    .as_str()
                    .filter(|w| !w.is_empty())
                    .unwrap_or("Primary checkout");
                let when = glib::DateTime::from_iso8601(text(&row, "ts"), None)
                    .and_then(|t| t.to_local())
                    .and_then(|t| t.format("%Y-%m-%d %H:%M"))
                    .map(|t| t.to_string())
                    .unwrap_or_else(|_| text(&row, "ts").to_owned());
                let place = label(&format!("{checkout} · {when}"), "dim");
                place.set_ellipsize(gtk::pango::EllipsizeMode::Start);
                place.set_xalign(0.0);
                words.append(&place);
                item.append(&words);
                let restore = button("Restore", "quiet");
                restore.set_valign(gtk::Align::Center);
                restore.set_widget_name(&format!("project-trash-restore:{}", row["id"]));
                item.append(&restore);
                list.append(&item);
                let audit = row["id"].clone();
                let trash = row["undo_op"]["payload"]["trash_id"].as_i64();
                let weak = Rc::downgrade(&ui);
                let ed = e.clone();
                restore.connect_clicked(move |button| {
                    let Some(ui) = weak.upgrade() else {
                        return;
                    };
                    button.set_sensitive(false);
                    let (button, item, audit, ed) =
                        (button.clone(), item.clone(), audit.clone(), ed.clone());
                    glib::spawn_future_local(async move {
                        match ui.call("audit.undo", json!({"audit_id":audit})).await {
                            Ok(_) => {
                                item.set_visible(false);
                                // The explorer's Restore key for the same file would now fail.
                                if trash.is_some_and(|id| id == ed.undo_trash.get()) {
                                    clear(&ed.file_undo);
                                }
                                if ui.project.get() == project {
                                    ed.load_tree(&ui);
                                    ed.refresh_git(&ui);
                                }
                            }
                            Err(err) => {
                                button.set_sensitive(true);
                                ui.show_error(&readable(&err));
                            }
                        }
                    });
                });
            }
        });
    }
}

fn trash_query(project: i64) -> Value {
    json!({"project_id":project,"op_prefix":"file.delete","limit":200})
}

/// The `file.delete` audit rows whose file is still in Relay trash.
fn trashed(history: &Value) -> Vec<Value> {
    rows(history, "rows")
        .into_iter()
        .filter(|row| row["kind"] == "ok" && !row["undo_op"].is_null() && row["undone_by"].is_null())
        .collect()
}

/// Restore one trashed file through its audit row when it is found, so the row is marked
/// undone and leaves Recently trashed; `file.restore` directly otherwise.
async fn restore_trash(ui: &Ui, project: i64, trash_id: i64) -> Result<Value, crate::client::Error> {
    let history = ui.call("audit.list", trash_query(project)).await?;
    match trashed(&history)
        .into_iter()
        .find(|row| row["undo_op"]["payload"]["trash_id"] == trash_id)
    {
        Some(row) => ui.call("audit.undo", json!({"audit_id":row["id"]})).await,
        None => {
            ui.call("file.restore", json!({"project_id":project,"trash_id":trash_id}))
                .await
        }
    }
}

/// The engine's sentence alone: its kind and code ("Conflict file.edit_conflict: …") are for
/// programs, not people. Code, its Git panel and Notes say every refusal this way.
pub(crate) fn readable(err: &crate::client::Error) -> String {
    match err {
        crate::client::Error::Bus(bus) => bus.message.clone(),
        other => other.to_string(),
    }
}

/// Replace every match and return how many were replaced.
///
/// `SearchContext::replace_all` in sourceview5 0.11 asserts that a zero return means an
/// error, but the C function returns the replacement count: no match, an empty query or an
/// invalid pattern all return 0 with no error, and the assert aborts the client from inside
/// a click handler. Only a set GError is a failure here.
pub fn replace_all(search: &sourceview5::SearchContext, replace: &str) -> Result<u32, glib::Error> {
    use glib::translate::{from_glib_full, ToGlibPtr};
    let Ok(length) = i32::try_from(replace.len()) else {
        return Err(glib::Error::new(glib::FileError::Inval, "Replacement text is too long"));
    };
    // SAFETY: `search` and `replace` outlive the call; GtkSourceView copies the replacement
    // and either leaves `error` null or sets it to a GError we take ownership of.
    unsafe {
        let mut error = std::ptr::null_mut();
        let count = sourceview5::ffi::gtk_source_search_context_replace_all(
            search.to_glib_none().0,
            replace.to_glib_none().0,
            length,
            &mut error,
        );
        if error.is_null() { Ok(count) } else { Err(from_glib_full(error)) }
    }
}

#[cfg(test)]
mod tests {
    use super::{crlf_dominant, snippet, with_newlines, TreeBadges, SNIPPET_CHARS};
    use serde_json::json;

    /// The explorer's letters come from the Git panel's `git.status` (untracked folders
    /// whole), and must read as `file.tree`'s own badges did (every untracked file listed).
    #[test]
    fn explorer_badges_from_the_panels_status_match_file_tree_badges() {
        let badges = TreeBadges::from_status(&[
            json!({"path":"src/lib.rs","index":"","worktree":"M"}),
            json!({"path":"staged.rs","index":"A","worktree":""}),
            json!({"path":"both.rs","index":"M","worktree":"D"}),
            json!({"path":"renamed.rs","index":"R","worktree":"","renamed_from":"old.rs"}),
            json!({"path":"vendor/","index":"","worktree":"?"}),
            json!({"path":"conflict.rs","index":"U","worktree":"U"}),
        ]);
        assert_eq!(badges.badge("src/lib.rs", false), "M");
        assert_eq!(badges.badge("staged.rs", false), "A");
        assert_eq!(badges.badge("both.rs", false), "D", "the worktree's code wins, as in file.tree");
        assert_eq!(badges.badge("renamed.rs", false), "R");
        assert_eq!(badges.badge("old.rs", false), "");
        assert_eq!(badges.badge("conflict.rs", false), "U");
        assert_eq!(badges.badge("vendor/pkg/a.js", false), "?", "a file in an untracked folder is untracked");
        assert_eq!(badges.badge("vendor/pkg", true), "", "folders are tinted, not lettered");
        assert_eq!(badges.badge("vendor", true), "");
        assert_eq!(badges.badge("vendored.txt", false), "", "a sibling sharing the prefix is not inside");
        assert_eq!(badges.badge("src/main.rs", false), "");
    }

    #[test]
    fn a_save_keeps_the_files_own_line_endings() {
        assert!(crlf_dominant("[a]\r\nb=1\r\nc=2\n"));
        assert!(!crlf_dominant("a\nb\r\nc\n"));
        assert!(!crlf_dominant(""));
        assert_eq!(with_newlines("a\r\nnew\nb\r\n", true), "a\r\nnew\r\nb\r\n");
        assert_eq!(with_newlines("a\npasted\r\nb\r", false), "a\npasted\nb\r");
    }

    #[test]
    fn a_short_line_is_shown_whole() {
        assert_eq!(snippet("  let x = needle;  ", 11), "let x = needle;");
    }

    #[test]
    fn a_long_line_is_cut_around_the_match_on_character_boundaries() {
        let line = format!("{}needle{}", "é".repeat(100_000), "ü".repeat(100_000));
        let col = "é".repeat(100_000).len() as u32 + 1;
        let shown = snippet(&line, col);
        assert!(shown.starts_with('…') && shown.ends_with('…'));
        assert!(shown.contains("needle"));
        assert!(shown.chars().count() <= SNIPPET_CHARS + 2);
        // A column inside a multi-byte character does not panic.
        assert!(!snippet("éé", 2).is_empty());
    }
}
