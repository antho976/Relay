//! The Files view as a workbench: a strip of the files opened, the open file's path as a
//! breadcrumb, and a status line of what the file is (language, encoding, line endings, indent).
//!
//! The editor holds one document, so a tab is a file to come back to, not a second buffer:
//! choosing one reads it again, and unsaved edits are asked about first, as opening any other
//! file is (`after_unsaved`). As in VS Code, the file a click opens takes a preview tab, in
//! italics, which the next click replaces; editing it, saving it or double-clicking its tab keeps
//! it.
use super::*;

/// One tab: the file, and whether it is kept or only previewed.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct Tab {
    pub path: String,
    pub kept: bool,
}

/// Where `path` goes among `tabs` once it is open: its own tab if it has one, else the preview
/// tab's place (the preview it replaces), else the end. Returns the tabs and the open one's index.
pub(super) fn opened(tabs: &[Tab], path: &str) -> (Vec<Tab>, usize) {
    let mut tabs = tabs.to_vec();
    if let Some(at) = tabs.iter().position(|t| t.path == path) {
        return (tabs, at);
    }
    let tab = Tab { path: path.to_owned(), kept: false };
    match tabs.iter().position(|t| !t.kept) {
        Some(at) => {
            tabs[at] = tab;
            (tabs, at)
        }
        None => {
            tabs.push(tab);
            let at = tabs.len() - 1;
            (tabs, at)
        }
    }
}

/// The open file's path as a breadcrumb, in Pango markup: folders quiet, the name in ink, and a
/// dot while it has unsaved edits.
pub(super) fn crumb(path: &str, modified: bool) -> String {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    let Some((name, folders)) = parts.split_last() else {
        return String::new();
    };
    let mut markup = String::new();
    for folder in folders {
        markup.push_str(&format!("<span alpha=\"60%\">{}  ›  </span>", glib::markup_escape_text(folder)));
    }
    markup.push_str(&format!("<b>{}</b>", glib::markup_escape_text(name)));
    if modified {
        markup.push_str("  <span alpha=\"70%\">•</span>");
    }
    markup
}

/// How a file indents: tabs, or the smallest run of spaces that starts an indented line,
/// whichever more lines use; a file with no indented line reads as four spaces.
pub(super) fn indent(text: &str) -> String {
    let (mut tabs, mut spaced) = (0, 0);
    let mut least: Option<usize> = None;
    for line in text.lines().take(2000).filter(|l| !l.trim().is_empty()) {
        if line.starts_with('\t') {
            tabs += 1;
            continue;
        }
        let spaces = line.len() - line.trim_start_matches(' ').len();
        if spaces > 0 {
            spaced += 1;
            least = Some(least.map_or(spaces, |l: usize| l.min(spaces)));
        }
    }
    match least {
        _ if tabs > spaced => "Tabs".into(),
        Some(n) => format!("Spaces: {}", n.min(8)),
        None => "Spaces: 4".into(),
    }
}

impl Editor {
    /// The status line's facts about the open file, after the caret's place.
    pub(super) fn describe(&self, content: &str) {
        let language = self.buffer.language().map(|l| l.name().to_string()).unwrap_or_else(|| "Plain text".into());
        let endings = if crlf_dominant(content) { "CRLF" } else { "LF" };
        self.facts.set_text(&format!("{language}    UTF-8    {endings}    {}", indent(content)));
    }

    /// `path` was opened: it takes its tab (see [`opened`]) and the strip is drawn again.
    pub(super) fn note_opened(self: &Rc<Self>, ui: &Rc<Ui>, path: &str) {
        let (tabs, _) = opened(&self.tabs.borrow(), path);
        *self.tabs.borrow_mut() = tabs;
        self.draw_tabs(ui);
    }

    /// Keep the open file's tab: it was edited, saved, or its tab double-clicked.
    pub(super) fn keep_tab(self: &Rc<Self>, ui: &Rc<Ui>, path: &str) {
        let changed = {
            let mut tabs = self.tabs.borrow_mut();
            match tabs.iter_mut().find(|t| t.path == path && !t.kept) {
                Some(tab) => {
                    tab.kept = true;
                    true
                }
                None => false,
            }
        };
        if changed {
            self.draw_tabs(ui);
        }
    }

    /// `keep_tab` from where no `Ui` is at hand (the buffer's edit signal): the tab is kept and
    /// loses its italics in place.
    pub(super) fn keep_shown_tab(&self, path: &str) {
        if let Some(tab) = self.tabs.borrow_mut().iter_mut().find(|t| t.path == path && !t.kept) {
            tab.kept = true;
        } else {
            return;
        }
        let mut child = self.tab_strip.first_child();
        while let Some(key) = child {
            if key.tooltip_text().as_deref() == Some(path) {
                key.remove_css_class("preview");
            }
            child = key.next_sibling();
        }
    }

    fn close_tab(self: &Rc<Self>, ui: &Rc<Ui>, path: &str) {
        let showing = *self.path.borrow() == path && !self.diff.get();
        let (next, left) = {
            let tabs = self.tabs.borrow();
            let at = tabs.iter().position(|t| t.path == path);
            let left: Vec<Tab> = tabs.iter().filter(|t| t.path != path).cloned().collect();
            let next = at.and_then(|at| left.get(at.min(left.len().saturating_sub(1))).map(|t| t.path.clone()));
            (next, left)
        };
        if !showing {
            *self.tabs.borrow_mut() = left;
            self.draw_tabs(ui);
            return;
        }
        // The file showing: its edits are asked about first, then the tab beside it opens.
        let path = path.to_owned();
        self.after_unsaved(ui, move |e, ui| {
            e.tabs.borrow_mut().retain(|t| t.path != path);
            match next.clone() {
                Some(next) => e.open(ui, next),
                None => {
                    e.clear_document();
                    e.show_files();
                }
            }
            e.draw_tabs(ui);
        });
    }

    /// The strip, drawn from `tabs`: the open file's tab lit, a preview tab in italics.
    pub(super) fn draw_tabs(self: &Rc<Self>, ui: &Rc<Ui>) {
        clear(&self.tab_strip);
        let tabs = self.tabs.borrow().clone();
        self.tab_scroll.set_visible(!tabs.is_empty() && !self.agents_visible() && !self.design_visible());
        let open = self.path.borrow().clone();
        for tab in tabs {
            let key = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            key.add_css_class("editor-tab");
            if tab.path == open && !self.diff.get() {
                key.add_css_class("open");
            }
            if !tab.kept {
                key.add_css_class("preview");
            }
            key.set_tooltip_text(Some(&tab.path));
            let (glyph, tint) = project_files::file_glyph(&tab.path);
            let icon = crate::icons::image(glyph, 13);
            icon.add_css_class(tint);
            key.append(&icon);
            let name = tab.path.rsplit('/').next().unwrap_or(&tab.path).to_owned();
            let caption = label(&name, "editor-tab-name");
            caption.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            caption.set_max_width_chars(28);
            key.append(&caption);
            let close = crate::app::icon_button("close", "Close");
            close.add_css_class("editor-tab-close");
            close.set_child(Some(&crate::icons::image("close", 11)));
            key.append(&close);
            let (weak, path) = (Rc::downgrade(self), tab.path.clone());
            let ui_ref = Rc::downgrade(ui);
            close.connect_clicked(move |_| {
                if let (Some(e), Some(ui)) = (weak.upgrade(), ui_ref.upgrade()) {
                    e.close_tab(&ui, &path);
                }
            });
            // A click opens it, a double click keeps it, a middle click closes it.
            let click = gtk::GestureClick::new();
            click.set_button(0);
            let (weak, path) = (Rc::downgrade(self), tab.path.clone());
            let ui_ref = Rc::downgrade(ui);
            click.connect_released(move |gesture, presses, _, _| {
                let (Some(e), Some(ui)) = (weak.upgrade(), ui_ref.upgrade()) else { return };
                match gesture.current_button() {
                    2 => e.close_tab(&ui, &path),
                    1 if presses >= 2 => e.keep_tab(&ui, &path),
                    1 => e.open_path(&ui, path.clone()),
                    _ => {}
                }
            });
            key.add_controller(click);
            self.tab_strip.append(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{crumb, indent, opened, Tab};

    fn tab(path: &str, kept: bool) -> Tab {
        Tab { path: path.into(), kept }
    }

    #[test]
    fn a_new_file_takes_the_preview_tab_and_a_kept_one_stays() {
        let (tabs, at) = opened(&[tab("a.rs", true), tab("b.rs", false)], "c.rs");
        assert_eq!(tabs, vec![tab("a.rs", true), tab("c.rs", false)]);
        assert_eq!(at, 1);
        let (tabs, at) = opened(&tabs, "a.rs");
        assert_eq!((tabs.len(), at), (2, 0));
        let (tabs, at) = opened(&[tab("a.rs", true)], "d.rs");
        assert_eq!((tabs, at), (vec![tab("a.rs", true), tab("d.rs", false)], 1));
    }

    #[test]
    fn the_crumb_names_folders_quietly_and_marks_edits() {
        assert_eq!(crumb("main.rs", false), "<b>main.rs</b>");
        assert!(crumb("src/a&b/lib.rs", true).contains("a&amp;b"));
        assert!(crumb("src/lib.rs", true).ends_with("•</span>"));
    }

    #[test]
    fn indent_is_read_from_the_file() {
        assert_eq!(indent("fn a() {\n    x\n        y\n}\n"), "Spaces: 4");
        assert_eq!(indent("a:\n  b: 1\n"), "Spaces: 2");
        assert_eq!(indent("x\n\ty\n"), "Tabs");
        assert_eq!(indent("one line"), "Spaces: 4");
    }
}
