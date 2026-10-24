//! A widget with a full form and a compact one: the title bar's search key, its utility keys and
//! New agent. It always asks only for the compact form's width, so the window's minimum is what
//! the folded bar needs and never what the full one does; which form it prefers is set from the
//! bar's width after layout (`app::fold_top_bar`), and the full form shows whenever it is preferred
//! and fits.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, OnceCell};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Fold {
        pub full: OnceCell<gtk::Widget>,
        pub compact: OnceCell<gtk::Widget>,
        pub prefer_full: Cell<bool>,
        /// The full form shortens (the search key's words): its least width is its natural one,
        /// and it grows only by expanding.
        pub grows: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Fold {
        const NAME: &'static str = "RelayFold";
        type Type = super::Fold;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Fold {
        fn dispose(&self) {
            for child in [self.full.get(), self.compact.get()].into_iter().flatten() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Fold {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let (Some(full), Some(compact)) = (self.full.get(), self.compact.get()) else {
                return (0, 0, -1, -1);
            };
            let (fmin, fnat, _, _) = full.measure(orientation, for_size);
            let (cmin, cnat, _, _) = compact.measure(orientation, for_size);
            match orientation {
                gtk::Orientation::Horizontal if self.prefer_full.get() && self.grows.get() => (cmin, fmin.max(cnat), -1, -1),
                gtk::Orientation::Horizontal if self.prefer_full.get() => (cmin, fnat.max(cnat), -1, -1),
                gtk::Orientation::Horizontal => (cmin, cnat, -1, -1),
                _ => (fmin.max(cmin), fnat.max(cnat), -1, -1),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let (Some(full), Some(compact)) = (self.full.get(), self.compact.get()) else {
                return;
            };
            let (fmin, _, _, _) = full.measure(gtk::Orientation::Horizontal, -1);
            let show_full = self.prefer_full.get() && width >= fmin;
            // As GtkPaned does with a collapsed side: the form not shown is unmapped, so it is
            // neither drawn, picked nor reached by Tab.
            full.set_child_visible(show_full);
            compact.set_child_visible(!show_full);
            let shown = if show_full { full } else { compact };
            shown.allocate(width, height, baseline, None);
        }
    }
}

glib::wrapper! {
    pub struct Fold(ObjectSubclass<imp::Fold>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Fold {
    pub fn new(full: &impl IsA<gtk::Widget>, compact: &impl IsA<gtk::Widget>) -> Self {
        let fold: Fold = glib::Object::new();
        full.set_parent(&fold);
        compact.set_parent(&fold);
        compact.set_child_visible(false);
        let _ = fold.imp().full.set(full.clone().upcast());
        let _ = fold.imp().compact.set(compact.clone().upcast());
        fold.imp().prefer_full.set(true);
        fold
    }

    /// Prefer the full form or the compact one. Never call it during layout.
    pub fn set_prefer_full(&self, full: bool) {
        if self.imp().prefer_full.replace(full) != full {
            self.queue_resize();
        }
    }

    /// See `grows`.
    pub fn set_grows(&self, grows: bool) {
        self.imp().grows.set(grows);
        self.queue_resize();
    }

    /// The width this fold asks for in one form or the other, as the bar lays it out.
    pub fn width_of(&self, full: bool) -> i32 {
        let child = if full { self.imp().full.get() } else { self.imp().compact.get() };
        child.map_or(0, |c| {
            let (min, nat, _, _) = c.measure(gtk::Orientation::Horizontal, -1);
            if full && self.imp().grows.get() { min } else { nat }
        })
    }
}
