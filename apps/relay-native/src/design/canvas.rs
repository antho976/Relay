//! The canvas widget: a plain widget whose snapshot is the board's drawing. GTK's renderer
//! draws its render nodes on the GPU, so panning and zooming cost a transform, not a repaint
//! of every shape on the CPU.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;

type Draw = Box<dyn Fn(&gtk::Snapshot, f64, f64)>;
type Resized = Box<dyn Fn(i32, i32)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Canvas {
        pub draw: RefCell<Option<Draw>>,
        pub resized: RefCell<Option<Resized>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Canvas {
        const NAME: &'static str = "RelayDesignCanvas";
        type Type = super::Canvas;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Canvas {}

    impl WidgetImpl for Canvas {
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            if let Some(resized) = self.resized.borrow().as_ref() {
                resized(width, height);
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            if let Some(draw) = self.draw.borrow().as_ref() {
                draw(snapshot, widget.width() as f64, widget.height() as f64);
            }
        }
    }
}

glib::wrapper! {
    pub struct Canvas(ObjectSubclass<imp::Canvas>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Canvas {
    pub fn new() -> Self {
        let canvas: Canvas = glib::Object::new();
        canvas.set_focusable(true);
        canvas.set_focus_on_click(true);
        canvas.set_hexpand(true);
        canvas.set_vexpand(true);
        canvas.set_overflow(gtk::Overflow::Hidden);
        canvas.set_widget_name("design-canvas");
        canvas.update_property(&[gtk::accessible::Property::Label("Design canvas")]);
        canvas
    }

    pub fn set_draw(&self, draw: impl Fn(&gtk::Snapshot, f64, f64) + 'static) {
        *self.imp().draw.borrow_mut() = Some(Box::new(draw));
    }

    /// Called with the new size whenever the canvas is allocated, before it draws.
    pub fn set_resized(&self, resized: impl Fn(i32, i32) + 'static) {
        *self.imp().resized.borrow_mut() = Some(Box::new(resized));
    }
}
