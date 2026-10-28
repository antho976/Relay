//! App-owned surfaces share the main window; only Notes and the device mirror detach.
use crate::app::{button, clear, icon_button, label, scrolled, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

pub struct Panel {
    pub body: gtk::Box,
    title: String,
    modal: Cell<bool>,
    /// A full page: it hides the surface under it while open, so what shows through its
    /// see-through panel is the wallpaper, not the page it covers.
    page: bool,
    layer: gtk::Overlay,
    frame: gtk::Box,
    scroll: gtk::ScrolledWindow,
    host: gtk::Overlay,
    panels: Weak<RefCell<Vec<Rc<Panel>>>>,
    guard: RefCell<Option<Box<dyn Fn() -> bool>>>,
    closed: RefCell<Vec<Box<dyn Fn()>>>,
    focus: Option<gtk::Widget>,
}

impl Panel {
    pub fn new(ui: &Ui, title: &str, width: i32) -> Rc<Self> {
        Self::build(ui, title, width, &ui.overlay, false)
    }

    /// Repeated activation closes the current surface before any work is started.
    pub fn toggle(ui: &Ui, title: &str, width: i32) -> Option<Rc<Self>> {
        let open = ui.panels.borrow().iter().any(|panel| panel.title == title);
        if !ui.dismiss_panels() || open {
            return None;
        }
        Some(Self::new(ui, title, width))
    }

    pub fn page(ui: &Ui, title: &str) -> Rc<Self> {
        Self::build(ui, title, -1, &ui.page_overlay, true)
    }

    /// A place under the scrolled body, always in view, for the actions of a form too long for
    /// the window (shown while it holds something). Its width never holds the panel wider than
    /// the window: it is cut, never scrolled, until the panel's `watch_width` narrows it.
    pub fn footer_slot(&self) -> gtk::Box {
        let pinned = gtk::Box::new(gtk::Orientation::Vertical, 0);
        pinned.add_css_class("panel-footer");
        let holder = gtk::ScrolledWindow::builder()
            .child(&pinned)
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .visible(false)
            .build();
        self.frame.append(&holder);
        pinned
    }

    /// The scrolled body's vertical adjustment: whether the body fits, and where it is.
    pub fn body_adjustment(&self) -> gtk::Adjustment {
        self.scroll.vadjustment()
    }

    /// Calls `on_width` with the panel's own width as it changes: the room it has, which its
    /// body (scrolled, and as wide as its content) does not tell.
    pub fn watch_width(&self, on_width: impl Fn(i32) + 'static) {
        crate::app::watch_width(&self.frame, on_width);
    }

    fn build(ui: &Ui, title: &str, width: i32, host: &gtk::Overlay, page: bool) -> Rc<Self> {
        let layer = gtk::Overlay::new();
        let scrim = button("", "panel-scrim");
        scrim.set_focusable(false);
        scrim.update_property(&[gtk::accessible::Property::Label("Close panel")]);
        if page {
            scrim.add_css_class("page-scrim");
        }
        layer.set_child(Some(&scrim));
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        frame.add_css_class("app-panel");
        frame.set_widget_name("app-panel");
        frame.set_halign(if page {
            gtk::Align::Fill
        } else {
            gtk::Align::End
        });
        frame.set_size_request(width, -1);
        frame.set_focusable(true);
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        heading.add_css_class("panel-heading");
        let title_text = title.to_string();
        let title = label(title, "section-title");
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let close = icon_button(
            if page { "arrow-left" } else { "close" },
            if page { "Back" } else { "Close panel" },
        );
        if page {
            heading.append(&close);
            frame.add_css_class("detail-page");
        }
        heading.append(&title);
        close.set_widget_name("panel-close");
        if !page {
            heading.append(&close);
        }
        frame.append(&heading);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
        body.add_css_class("panel-body");
        let scroll = scrolled(&body);
        frame.append(&scroll);
        layer.add_overlay(&frame);
        let panel = Rc::new(Self {
            body,
            title: title_text,
            modal: Cell::new(true),
            page,
            layer,
            frame,
            scroll,
            host: host.clone(),
            panels: Rc::downgrade(&ui.panels),
            guard: RefCell::default(),
            closed: RefCell::default(),
            focus: gtk::prelude::GtkWindowExt::focus(&ui.window),
        });
        for key in [close, scrim] {
            let weak = Rc::downgrade(&panel);
            key.connect_clicked(move |_| {
                if let Some(panel) = weak.upgrade() {
                    panel.close();
                }
            });
        }
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&panel);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if matches!(key, gtk::gdk::Key::Tab | gtk::gdk::Key::ISO_Left_Tab) {
                if let Some(panel) = weak.upgrade() {
                    // An editable text area that takes Tab types it; Ctrl+Tab still leaves it.
                    let typing = !modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                        && panel.frame.root().and_then(|root| root.focus()).and_downcast::<gtk::TextView>()
                            .is_some_and(|view| view.accepts_tab() && view.is_editable());
                    if typing {
                        return glib::Propagation::Proceed;
                    }
                    let direction = if modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                        gtk::DirectionType::TabBackward
                    } else {
                        gtk::DirectionType::TabForward
                    };
                    if !panel.frame.child_focus(direction) {
                        panel.frame.grab_focus();
                        panel.frame.child_focus(direction);
                    }
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        panel.layer.add_controller(keys);
        panel
    }

    pub fn compact(&self, centered: bool, height: i32) {
        self.frame.add_css_class("compact-panel");
        self.frame.set_valign(gtk::Align::Start);
        self.frame.set_halign(if centered {
            gtk::Align::Center
        } else {
            gtk::Align::End
        });
        self.frame.set_margin_top(if centered {
            self.host.height() * 12 / 100
        } else {
            0
        });
        self.frame.set_margin_end(if centered { 0 } else { 126 });
        self.scroll
            .set_min_content_height(height.min((self.host.height() - 160).max(200)));
    }

    pub fn centered(&self, height: i32) {
        self.frame.set_halign(gtk::Align::Center);
        self.frame.set_valign(gtk::Align::Center);
        self.scroll.set_vexpand(false);
        self.scroll
            .set_max_content_height(height.min((self.host.height() - 48).max(200)));
        self.scroll.set_propagate_natural_height(true);
    }

    /// Scrolls by wheel and touchpad only, with no scrollbar drawn.
    pub fn hide_scrollbar(&self) {
        self.scroll.set_vscrollbar_policy(gtk::PolicyType::External);
        self.scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    }

    pub fn header_action(&self, widget: &impl IsA<gtk::Widget>) {
        if let Some(heading) = self.frame.first_child().and_downcast::<gtk::Box>() {
            let previous = heading.last_child().and_then(|close| close.prev_sibling());
            heading.insert_child_after(widget, previous.as_ref());
        }
    }

    /// Grow to the content once it has arrived. A scrolled window asks its child for a height
    /// at no particular width, so wrapped text counts as one line and the last rows are clipped.
    pub fn fit(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let width = match panel.scroll.width() {
                0 => panel.frame.width_request() - 2,
                width => width,
            };
            if width <= 0 {
                return;
            }
            let (_, natural, _, _) = panel.body.measure(gtk::Orientation::Vertical, width);
            let ceiling = panel.scroll.max_content_height();
            panel.scroll.set_min_content_height(if ceiling > 0 { natural.min(ceiling) } else { natural });
        });
    }

    pub fn bottom(&self, height: i32) {
        self.modal.set(false);
        self.layer.add_css_class("utility-layer");
        self.frame.add_css_class("utility-panel");
        self.frame.set_valign(gtk::Align::End);
        self.frame.set_margin_end(8);
        let footer = self
            .host
            .next_sibling()
            .filter(|widget| widget.is_visible())
            .map(|widget| widget.height())
            .unwrap_or(0);
        self.frame.set_margin_bottom((30 - footer).max(0));
        self.scroll.set_vexpand(false);
        self.scroll
            .set_min_content_height(height.min((self.host.height() - 100).max(200)));
        self.scroll
            .set_max_content_height((self.host.height() - 70).max(200));
        self.scroll.set_propagate_natural_height(true);
    }

    /// Opens just inside `content`'s left edge with its foot level with `anchor`'s: the usage
    /// card's panel, beside the sidebar. An anchor that is not on screen (a hidden sidebar)
    /// leaves it to `bottom`.
    pub fn beside(&self, content: &impl IsA<gtk::Widget>, anchor: &impl IsA<gtk::Widget>, height: i32) {
        let bounds = anchor.is_mapped().then(|| anchor.compute_bounds(&self.host)).flatten();
        // A point, not `compute_bounds`: the sidebar box beside it reads 0 wide while laid out.
        let edge = content.compute_point(&self.host, &gtk::graphene::Point::new(0.0, 0.0));
        let (Some(bounds), Some(edge)) = (bounds, edge) else {
            return self.bottom(height);
        };
        self.modal.set(false);
        self.layer.add_css_class("utility-layer");
        self.frame.add_css_class("utility-panel");
        self.frame.add_css_class("floating-panel");
        // Rounded like the card it opens from; the corners clip what scrolls under them.
        self.frame.set_overflow(gtk::Overflow::Hidden);
        self.frame.set_halign(gtk::Align::Start);
        self.frame.set_valign(gtk::Align::End);
        self.frame.set_margin_start(edge.x() as i32 + 8);
        let bottom = (self.host.height() - (bounds.y() + bounds.height()) as i32).max(8);
        self.frame.set_margin_bottom(bottom);
        // The heading takes about 50px of what is left above the anchor's foot.
        let room = (self.host.height() - bottom - 66).max(200);
        self.scroll.set_vexpand(false);
        self.scroll.set_min_content_height(height.min(room));
        self.scroll.set_max_content_height(room);
        self.scroll.set_propagate_natural_height(true);
    }

    pub fn add_css_class(&self, name: &str) {
        self.frame.add_css_class(name);
    }

    /// Blocks what is under it; a `top`, `bottom` or `beside` utility panel does not.
    pub fn is_modal(&self) -> bool {
        self.modal.get()
    }

    pub fn set_guard(&self, guard: impl Fn() -> bool + 'static) {
        *self.guard.borrow_mut() = Some(Box::new(guard));
    }

    pub fn can_close(&self) -> bool {
        self.guard.borrow().as_ref().is_none_or(|guard| guard())
    }

    pub fn on_closed(&self, callback: impl Fn() + 'static) {
        self.closed.borrow_mut().push(Box::new(callback));
    }

    pub fn present(self: &Rc<Self>) {
        if self.layer.parent().is_none() {
            let Some(panels) = self.panels.upgrade() else {
                return;
            };
            if let Some(previous) = panels.borrow().last() {
                previous.layer.set_sensitive(false);
            }
            if self.modal.get() {
                if let Some(content) = self.host.child() {
                    content.set_sensitive(false);
                    if self.page {
                        content.set_opacity(0.);
                    }
                }
            }
            self.host.add_overlay(&self.layer);
            panels.borrow_mut().push(self.clone());
        }
        if !self.modal.get() {
            self.frame.child_focus(gtk::DirectionType::TabForward);
        } else if !self.focus_body() {
            // A modal panel opens on its first control; with none in its body yet (still
            // loading), on the heading's close key, from the top as Tab wraps.
            self.frame.grab_focus();
            self.frame.child_focus(gtk::DirectionType::TabForward);
        }
    }

    /// Bring an open panel back to the top, as a link to a page already open under others
    /// does: the panels over it stay open, now under it, and its Back returns to them.
    pub fn raise(self: &Rc<Self>) {
        let Some(panels) = self.panels.upgrade() else { return };
        let on_top = panels.borrow().last().is_some_and(|top| Rc::ptr_eq(top, self));
        if self.layer.parent().is_some() && !on_top {
            panels.borrow_mut().retain(|p| !Rc::ptr_eq(p, self));
            for other in panels.borrow().iter() {
                other.layer.set_sensitive(false);
            }
            // An overlay draws its children in the order they were added.
            self.host.remove_overlay(&self.layer);
            self.host.add_overlay(&self.layer);
            self.layer.set_sensitive(true);
            panels.borrow_mut().push(self.clone());
        }
        self.present();
    }

    /// Focus the body's first control. A selectable paragraph is passed over: focusing it
    /// selects all of its text.
    fn focus_body(&self) -> bool {
        while self.body.child_focus(gtk::DirectionType::TabForward) {
            match self.frame.root().and_then(|root| root.focus()).and_downcast::<gtk::Label>() {
                Some(text) => text.select_region(0, 0),
                None => return true,
            }
        }
        false
    }

    pub fn close(&self) {
        if !self.can_close() || self.layer.parent().is_none() {
            return;
        }
        self.host.remove_overlay(&self.layer);
        if let Some(panels) = self.panels.upgrade() {
            panels.borrow_mut().retain(|p| p.layer != self.layer);
            if let Some(previous) = panels.borrow().last() {
                previous.layer.set_sensitive(true);
            }
            if !panels
                .borrow()
                .iter()
                .any(|panel| panel.host == self.host && panel.modal.get())
            {
                if let Some(content) = self.host.child() {
                    content.set_sensitive(true);
                }
            }
            if !panels.borrow().iter().any(|panel| panel.host == self.host && panel.page) {
                if let Some(content) = self.host.child() {
                    content.set_opacity(1.);
                }
            }
        }
        for callback in self.closed.borrow_mut().drain(..) {
            callback();
        }
        clear(&self.body);
        if let Some(focus) = &self.focus {
            if focus.is_sensitive() && focus.is_mapped() {
                focus.grab_focus();
            }
        }
    }

    pub async fn response(self: &Rc<Self>, caption: &str) -> bool {
        let (send, recv) = async_channel::bounded(1);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let cancel = button("Cancel", "quiet");
        let accept = button(caption, "primary");
        accept.set_widget_name("panel-accept");
        actions.append(&cancel);
        actions.append(&accept);
        self.body.append(&actions);
        let weak = Rc::downgrade(self);
        cancel.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.close();
            }
        });
        let accepted = send.clone();
        accept.connect_clicked(move |_| {
            let _ = accepted.try_send(true);
        });
        self.on_closed(move || {
            let _ = send.try_send(false);
        });
        self.compact(true, 280);
        // A dialog that asks for a name starts in its field, keeping the caret or selection
        // its caller set, and Enter there accepts it; a plain yes or no starts on Cancel.
        let entry = std::iter::successors(self.body.first_child(), |w| w.next_sibling())
            .find_map(|w| w.downcast::<gtk::Entry>().ok());
        // Read before present(), whose Tab into the field selects all of it.
        let (selection, caret) = entry.as_ref().map_or((None, 0), |e| (e.selection_bounds(), e.position()));
        self.present();
        if let Some(entry) = entry {
            let accept = accept.downgrade();
            entry.connect_activate(move |_| {
                if let Some(accept) = accept.upgrade() {
                    accept.emit_clicked();
                }
            });
            entry.grab_focus_without_selecting();
            match selection {
                Some((start, end)) => entry.select_region(start, end),
                None => entry.set_position(caret),
            }
        } else {
            cancel.grab_focus();
        }
        let accepted = recv.recv().await.unwrap_or(false);
        self.close();
        accepted
    }
}

/// A non-modal surface docked to the right edge of the main window, beside the page rather than
/// over it: the page narrows to make room, so nothing of it (the view switch, a page's text) is
/// hidden under the dock. It sits outside the panel stack on purpose: navigation and other panels
/// leave it alone, and its body is not scrolled — a live view sizes itself to the height it is
/// given. The device mirror is its one user.
pub struct Dock {
    pub body: gtk::Box,
    host: gtk::Box,
}

impl Dock {
    pub fn new(ui: &Ui, width: i32) -> Self {
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.add_css_class("app-panel");
        body.add_css_class("dock-panel");
        body.set_valign(gtk::Align::Fill);
        body.set_hexpand(false);
        body.set_size_request(width, -1);
        ui.dock_host.append(&body);
        Self {
            body,
            host: ui.dock_host.clone(),
        }
    }

    pub fn close(&self) {
        if self.body.parent().is_some() {
            self.host.remove(&self.body);
        }
    }
}
