// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

use adw::subclass::prelude::*;
use glib::clone;
use gtk::{glib, graphene, pango, prelude::*, subclass::prelude::*};

mod imp {
    use glib::{ParamFlags, ParamSpec, ParamSpecFloat, ParamSpecString, ParamSpecUInt, Value};
    use once_cell::sync::Lazy;

    use super::*;

    const DEFAULT_MIN_CHARS: u32 = 3;
    const DEFAULT_NAT_CHARS: u32 = 0;
    const DEFAULT_XALIGN: f32 = 0.0;
    const DEFAULT_YALIGN: f32 = 0.5;

    #[derive(Debug)]
    pub struct MarqueeLabel {
        pub label: RefCell<Option<String>>,
        pub layout: RefCell<Option<pango::Layout>>,
        pub min_chars: Cell<u32>,
        pub nat_chars: Cell<u32>,
        pub xalign: Cell<f32>,
        pub yalign: Cell<f32>,
        pub tick_id: RefCell<Option<gtk::TickCallbackId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MarqueeLabel {
        const NAME: &'static str = "MarqueeLabel";
        type Type = super::MarqueeLabel;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("label");
            klass.set_accessible_role(gtk::AccessibleRole::Label);
        }

        fn new() -> Self {
            Self {
                label: RefCell::new(None),
                layout: RefCell::new(None),
                min_chars: Cell::new(DEFAULT_MIN_CHARS),
                nat_chars: Cell::new(DEFAULT_NAT_CHARS),
                xalign: Cell::new(DEFAULT_XALIGN),
                yalign: Cell::new(DEFAULT_YALIGN),
                tick_id: RefCell::new(None),
            }
        }
    }

    impl ObjectImpl for MarqueeLabel {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::new("label", "", "", None, ParamFlags::READWRITE),
                    ParamSpecUInt::new(
                        "min-chars",
                        "",
                        "",
                        0,
                        u32::MAX,
                        DEFAULT_MIN_CHARS,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecUInt::new(
                        "nat-chars",
                        "",
                        "",
                        0,
                        u32::MAX,
                        DEFAULT_NAT_CHARS,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecFloat::new(
                        "xalign",
                        "",
                        "",
                        0.0,
                        1.0,
                        DEFAULT_XALIGN,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecFloat::new(
                        "yalign",
                        "",
                        "",
                        0.0,
                        1.0,
                        DEFAULT_YALIGN,
                        ParamFlags::READWRITE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "label" => obj.set_label(value.get::<&str>().unwrap()),
                "min-chars" => obj.set_min_chars(value.get::<u32>().unwrap()),
                "nat-chars" => obj.set_nat_chars(value.get::<u32>().unwrap()),
                "xalign" => obj.set_xalign(value.get::<f32>().unwrap()),
                "yalign" => obj.set_yalign(value.get::<f32>().unwrap()),
                _ => unimplemented!(),
            };
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "label" => obj.label().to_value(),
                "min-chars" => obj.min_chars().to_value(),
                "nat-chars" => obj.nat_chars().to_value(),
                "xalign" => obj.xalign().to_value(),
                "yalign" => obj.yalign().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            let layout = obj.create_pango_layout(None);
            layout.set_wrap(pango::WrapMode::WordChar);
            layout.set_ellipsize(pango::EllipsizeMode::None);

            self.layout.replace(Some(layout));
        }
    }

    impl WidgetImpl for MarqueeLabel {
        fn measure(
            &self,
            widget: &Self::Type,
            orientation: gtk::Orientation,
            _for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let mut minimum = 0;
            let mut natural = 0;
            let mut min_baseline = -1;
            let mut nat_baseline = -1;
            match orientation {
                gtk::Orientation::Horizontal => (minimum, natural) = widget.measure_width(),
                gtk::Orientation::Vertical => {
                    (minimum, natural, min_baseline, nat_baseline) = widget.measure_height()
                }
                _ => (),
            };

            // The measurement functions return Pango units
            (
                pixels_ceil(minimum),
                pixels_ceil(natural),
                pixels_ceil(min_baseline),
                pixels_ceil(nat_baseline),
            )
        }

        fn size_allocate(&self, widget: &Self::Type, _width: i32, _height: i32, _baseline: i32) {
            let layout = widget.layout().unwrap();
            layout.set_width(-1);
            layout.set_height(-1);
        }

        fn snapshot(&self, widget: &Self::Type, snapshot: &gtk::Snapshot) {
            let width = widget.width() as f32;
            let height = widget.height() as f32;
            snapshot.push_clip(&graphene::Rect::new(0.0, 0.0, width, height));
            let (lx, ly) = widget.layout_location();
            snapshot.render_layout(
                &widget.style_context(),
                lx as f64,
                ly as f64,
                &widget.layout().unwrap(),
            );
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    pub struct MarqueeLabel(ObjectSubclass<imp::MarqueeLabel>)
        @extends gtk::Widget,
        @implements gtk::Accessible;
}

impl Default for MarqueeLabel {
    fn default() -> Self {
        glib::Object::new(&[]).expect("Failed to create MarqueeLabel")
    }
}

fn pixels_ceil(d: i32) -> i32 {
    if d > 0 {
        d + 1023 >> 10
    } else {
        -1
    }
}

impl MarqueeLabel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_label(&self, label: &str) {
        let imp = self.imp();

        // Stop the animation
        if let Some(tick_id) = imp.tick_id.replace(None) {
            tick_id.remove();
        }

        if let Some(ref layout) = imp.layout.borrow().as_ref() {
            layout.set_text(label);

            // Schedule the animation
            glib::timeout_add_local(
                Duration::from_millis(500),
                clone!(@weak self as this => @default-return glib::Continue(false), move || {
                    let tick_id = this.add_tick_callback(move |w, _| {
                        w.queue_draw();
                        glib::Continue(true)
                    });
                    this.imp().tick_id.replace(Some(tick_id));
                    glib::Continue(false)
                }),
            );
        }

        imp.label.replace(Some(label.to_owned()));
        self.notify("label");
        self.queue_draw();
    }

    pub fn label(&self) -> String {
        match self.imp().label.borrow().as_ref() {
            Some(s) => s.clone(),
            None => "".to_owned(),
        }
    }

    pub fn set_min_chars(&self, min_chars: u32) {
        if min_chars != self.imp().min_chars.replace(min_chars) {
            self.notify("min-chars");
            self.queue_resize();
        }
    }

    pub fn min_chars(&self) -> u32 {
        self.imp().min_chars.get()
    }

    pub fn set_nat_chars(&self, nat_chars: u32) {
        if nat_chars != self.imp().nat_chars.replace(nat_chars) {
            self.notify("nat-chars");
            self.queue_resize();
        }
    }

    pub fn nat_chars(&self) -> u32 {
        self.imp().nat_chars.get()
    }

    pub fn set_xalign(&self, xalign: f32) {
        let xalign = xalign.clamp(0.0, 1.0);
        if xalign != self.imp().xalign.replace(xalign) {
            self.notify("xalign");
            self.queue_draw();
        }
    }

    pub fn xalign(&self) -> f32 {
        self.imp().xalign.get()
    }

    pub fn set_yalign(&self, yalign: f32) {
        let yalign = yalign.clamp(0.0, 1.0);
        if yalign != self.imp().yalign.replace(yalign) {
            self.notify("yalign");
            self.queue_draw();
        }
    }

    pub fn yalign(&self) -> f32 {
        self.imp().yalign.get()
    }

    fn is_animating(&self) -> bool {
        self.imp().tick_id.borrow().is_some()
    }

    fn layout(&self) -> Option<pango::Layout> {
        if let Some(ref layout) = &*self.imp().layout.borrow() {
            Some(layout.clone())
        } else {
            None
        }
    }

    fn font_metrics(&self) -> Option<pango::FontMetrics> {
        self.pango_context()
            .metrics(None::<&pango::FontDescription>, None::<&pango::Language>)
    }

    fn char_pixels(&self) -> i32 {
        if let Some(metrics) = self.font_metrics() {
            let char_width = metrics.approximate_char_width();
            let digit_width = metrics.approximate_digit_width();

            i32::max(char_width, digit_width)
        } else {
            0
        }
    }

    fn line_pixels(&self) -> (i32, i32) {
        if let Some(metrics) = self.font_metrics() {
            let ascent = metrics.ascent();
            let descent = metrics.descent();
            (ascent + descent, ascent)
        } else {
            (0, -1)
        }
    }

    fn measure_width(&self) -> (i32, i32) {
        let min_chars = self.min_chars() as i32;
        let nat_chars = self.nat_chars() as i32;
        if min_chars == 0 && nat_chars == 0 {
            return (0, 0);
        }

        let char_px = self.char_pixels();

        (
            min_chars * char_px,
            i32::max(min_chars, nat_chars) * char_px,
        )
    }

    fn measure_height(&self) -> (i32, i32, i32, i32) {
        let (line_pixels, baseline) = self.line_pixels();
        (line_pixels, line_pixels, baseline, baseline)
    }

    fn layout_location(&self) -> (f32, f32) {
        let xalign;
        if self.direction() != gtk::TextDirection::Ltr {
            xalign = 1.0 - self.xalign();
        } else {
            xalign = self.xalign();
        }

        let (logical, _) = self.layout().unwrap().pixel_extents();
        let widget_width = self.width() as f32;
        let widget_height = self.height() as f32;

        let x;
        let y;

        if self.is_animating() && widget_width < logical.width() as f32 {
            let repeat_time = (logical.width() as i64 - widget_width as i64) * 60_000;
            let mut frame_time = (self.frame_clock().unwrap().frame_time() % (2 * repeat_time)).abs();
            if frame_time > repeat_time {
                frame_time = 2 * repeat_time - frame_time;
            }
            x = (widget_width - logical.width() as f32) * frame_time as f32 / repeat_time as f32;
        } else {
            x = ((xalign * (widget_width - logical.width() as f32)) - logical.x() as f32).floor();
        }

        let baseline = self.allocated_baseline();
        if baseline != -1 {
            let layout_baseline = self.layout().unwrap().baseline() / pango::SCALE;
            y = baseline as f32 - layout_baseline as f32;
        } else if self.layout().unwrap().is_ellipsized() {
            y = 0.0;
        } else {
            let f = ((widget_height - logical.height() as f32) * self.yalign()).floor();
            if f < 0.0 {
                y = 0.0;
            } else {
                y = f;
            }
        }

        (x, y)
    }
}
