//! A column at most `maximum` wide, at the start of the room it is given and never holding the
//! window wider than its child's least width: a page's form and list read at a measure instead
//! of across a 1440px window (the Mailbox).

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, OnceCell};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Clamp {
        pub child: OnceCell<gtk::Widget>,
        pub maximum: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Clamp {
        const NAME: &'static str = "RelayClamp";
        type Type = super::Clamp;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Clamp {
        fn dispose(&self) {
            if let Some(child) = self.child.get() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Clamp {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.child.get() else {
                return (0, 0, -1, -1);
            };
            let maximum = self.maximum.get();
            match orientation {
                gtk::Orientation::Horizontal => {
                    let (min, _, _, _) = child.measure(orientation, for_size);
                    (min, maximum.max(min), -1, -1)
                }
                _ => {
                    let width = if for_size < 0 { -1 } else { for_size.min(maximum) };
                    let (min, nat, _, _) = child.measure(orientation, width);
                    (min, nat, -1, -1)
                }
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.child.get() {
                let (min, _, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
                child.allocate(width.min(self.maximum.get()).max(min), height, baseline, None);
            }
        }
    }
}

glib::wrapper! {
    pub struct Clamp(ObjectSubclass<imp::Clamp>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Clamp {
    pub fn new(child: &impl IsA<gtk::Widget>, maximum: i32) -> Self {
        let clamp: Clamp = glib::Object::new();
        child.set_parent(&clamp);
        let _ = clamp.imp().child.set(child.clone().upcast());
        clamp.imp().maximum.set(maximum);
        clamp
    }
}
