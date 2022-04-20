// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::RefCell;

use adw::subclass::prelude::*;
use glib::clone;
use gtk::{gdk, gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use crate::{audio::Song, cover_picture::CoverPicture, window::Window};

mod imp {
    use glib::{ParamFlags, ParamSpec, ParamSpecBoolean, ParamSpecObject, ParamSpecString, Value};
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/bassi/Amberol/queue-row.ui")]
    pub struct QueueRow {
        // Template widgets
        #[template_child]
        pub row_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub song_cover_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub song_cover_image: TemplateChild<CoverPicture>,
        #[template_child]
        pub song_title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub song_artist_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub controls_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub move_controls_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub move_up_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub move_down_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub remove_button: TemplateChild<gtk::Button>,

        pub song: RefCell<Option<Song>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for QueueRow {
        const NAME: &'static str = "AmberolQueueRow";
        type Type = super::QueueRow;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.set_layout_manager_type::<gtk::BoxLayout>();
            klass.set_css_name("queuerow");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for QueueRow {
        fn dispose(&self, _obj: &Self::Type) {
            self.row_stack.unparent();
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.init_controllers();
            obj.init_widgets();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "song",
                        "",
                        "",
                        Song::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecString::new("song-artist", "", "", None, ParamFlags::READWRITE),
                    ParamSpecString::new("song-title", "", "", None, ParamFlags::READWRITE),
                    ParamSpecObject::new(
                        "song-cover",
                        "",
                        "",
                        gdk::Texture::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new("playing", "", "", false, ParamFlags::READWRITE),
                    ParamSpecBoolean::new("move-controls", "", "", true, ParamFlags::READWRITE),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "song" => {
                    let song = value.get::<Option<Song>>().unwrap();
                    self.song.replace(song);
                }
                "song-artist" => {
                    let p = value.get::<&str>().expect("The value needs to be a string");
                    self.song_artist_label.set_label(p);
                }
                "song-title" => {
                    let p = value.get::<&str>().expect("The value needs to be a string");
                    self.song_title_label.set_label(p);
                }
                "song-cover" => {
                    let p = value.get::<gdk::Texture>().ok();
                    if let Some(texture) = p {
                        self.song_cover_image.set_cover(Some(&texture));
                        self.song_cover_stack.set_visible_child_name("cover");
                    } else {
                        self.song_cover_image.set_cover(None);
                        self.song_cover_stack.set_visible_child_name("no-cover");
                    }
                }
                "playing" => {
                    let p = value
                        .get::<bool>()
                        .expect("The value needs to be a boolean");
                    if p {
                        self.row_stack.set_visible_child_name("currently-playing");
                    } else {
                        self.row_stack.set_visible_child_name("song-details");
                    }
                }
                "move-controls" => {
                    let p = value
                        .get::<bool>()
                        .expect("The value needs to be a boolean");
                    debug!("Move controls for queue row visible: {}", p);
                    self.move_controls_box.set_visible(p);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "song" => self.song.borrow().to_value(),
                "song-artist" => self.song_artist_label.label().to_value(),
                "song-title" => self.song_title_label.label().to_value(),
                "song-cover" => self.song_cover_image.cover().to_value(),
                "playing" => {
                    let visible_child = self.row_stack.visible_child_name().unwrap();
                    let v = matches!(visible_child.as_str(), "currently-playing");
                    v.to_value()
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for QueueRow {}
}

glib::wrapper! {
    pub struct QueueRow(ObjectSubclass<imp::QueueRow>)
        @extends gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for QueueRow {
    fn default() -> Self {
        glib::Object::new(&[]).expect("Failed to create QueueRow")
    }
}

impl QueueRow {
    pub fn new() -> Self {
        Self::default()
    }

    fn init_controllers(&self) {
        let controller = gtk::EventControllerMotion::new();
        controller.set_name("queuerow motion");
        controller.connect_contains_pointer_notify(clone!(@strong self as this => move |c| {
            this.imp().controls_box.set_visible(c.contains_pointer());
        }));
        self.add_controller(&controller);
    }

    fn init_widgets(&self) {
        self.imp()
            .remove_button
            .connect_clicked(clone!(@strong self as this => move |_| {
                let app = gio::Application::default()
                    .expect("Failed to retrieve application singleton")
                    .downcast::<gtk::Application>()
                    .unwrap();
                let win = app
                    .active_window()
                    .unwrap()
                    .downcast::<Window>()
                    .unwrap();
                if let Some(song) = this.song() {
                    win.remove_song(&song);
                }
            }));

        self.imp()
            .move_up_button
            .connect_clicked(clone!(@strong self as this => move |_| {
                let app = gio::Application::default()
                    .expect("Failed to retrieve application singleton")
                    .downcast::<gtk::Application>()
                    .unwrap();
                let win = app
                    .active_window()
                    .unwrap()
                    .downcast::<Window>()
                    .unwrap();
                if let Some(song) = this.song() {
                    win.move_song_up(&song);
                }
            }));
        self.imp()
            .move_down_button
            .connect_clicked(clone!(@strong self as this => move |_| {
                let app = gio::Application::default()
                    .expect("Failed to retrieve application singleton")
                    .downcast::<gtk::Application>()
                    .unwrap();
                let win = app
                    .active_window()
                    .unwrap()
                    .downcast::<Window>()
                    .unwrap();
                if let Some(song) = this.song() {
                    win.move_song_down(&song);
                }
            }));
    }

    pub fn set_song_title(&self, title: String) {
        let imp = self.imp();
        imp.song_title_label.set_label(&title);
    }

    pub fn set_song_artist(&self, artist: String) {
        let imp = self.imp();
        imp.song_artist_label.set_label(&artist);
    }

    pub fn set_playing(&self, playing: bool) {
        let imp = self.imp();
        if playing {
            imp.row_stack.set_visible_child_name("currently-playing");
        } else {
            imp.row_stack.set_visible_child_name("song-details");
        }
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
    }
}
