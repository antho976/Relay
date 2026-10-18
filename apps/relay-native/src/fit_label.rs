//! A label with a shorter wording for when its own does not fit: the command search reads
//! "Search or run a command" where there is room and "Search" where there is not, rather than
//! "Search or r…". Both are laid out every time; the one that fits is the one drawn, so the
//! choice follows the width without changing anything during the layout that gives it.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, OnceCell};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct FitLabel {
        pub long: OnceCell<gtk::Label>,
        pub short: OnceCell<gtk::Label>,
        pub fits: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FitLabel {
        const NAME: &'static str = "RelayFitLabel";
        type Type = super::FitLabel;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for FitLabel {
        fn dispose(&self) {
            for child in [self.long.get(), self.short.get()].into_iter().flatten() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for FitLabel {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let (Some(long), Some(short)) = (self.long.get(), self.short.get()) else {
                return (0, 0, -1, -1);
            };
            let (lmin, lnat, _, _) = long.measure(orientation, for_size);
            let (smin, snat, _, _) = short.measure(orientation, for_size);
            // As narrow as the short wording allows, as wide as the long one wants.
            match orientation {
                gtk::Orientation::Horizontal => (smin, lnat.max(snat), -1, -1),
                _ => (lmin.max(smin), lnat.max(snat), -1, -1),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let (Some(long), Some(short)) = (self.long.get(), self.short.get()) else {
                return;
            };
            let (_, natural, _, _) = long.measure(gtk::Orientation::Horizontal, -1);
            let fits = width >= natural;
            if self.fits.replace(fits) != fits {
                self.obj().queue_draw();
            }
            long.allocate(width, height, baseline, None);
            short.allocate(width, height, baseline, None);
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let shown = if self.fits.get() { self.long.get() } else { self.short.get() };
            if let Some(child) = shown {
                self.obj().snapshot_child(child, snapshot);
            }
        }
    }
}

glib::wrapper! {
    pub struct FitLabel(ObjectSubclass<imp::FitLabel>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl FitLabel {
    pub fn new(long: &str, short: &str, class: &str) -> Self {
        let fit: FitLabel = glib::Object::new();
        let make = |text: &str| {
            let l = gtk::Label::new(Some(text));
            if !class.is_empty() {
                l.add_css_class(class);
            }
            l.set_xalign(0.0);
            l.set_ellipsize(gtk::pango::EllipsizeMode::End);
            l.set_parent(&fit);
            l
        };
        let long_label = make(long);
        let short_label = make(short);
        // Read once, as the long wording; the short one is only what is drawn.
        short_label.update_state(&[gtk::accessible::State::Hidden(true)]);
        let _ = fit.imp().long.set(long_label);
        let _ = fit.imp().short.set(short_label);
        fit.imp().fits.set(true);
        fit
    }
}
