// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{cell::RefCell, rc::Rc};

use adw::subclass::prelude::*;
use glib::{clone, Receiver};
use gtk::{gdk, gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use crate::{
    audio::{AudioPlayer, RepeatMode, Song},
    config::APPLICATION_ID,
    cover_picture::CoverPicture,
    drag_overlay::DragOverlay,
    i18n::{i18n, ni18n_f},
    playback_control::PlaybackControl,
    queue_row::QueueRow,
    utils,
};

pub enum WindowAction {
    Present,
}

mod imp {
    use super::*;

    #[derive(CompositeTemplate)]
    #[template(resource = "/io/bassi/Amberol/window.ui")]
    pub struct Window {
        // Template widgets
        #[template_child]
        pub song_title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub song_artist_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub song_album_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub song_time_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub queue_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub queue_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub cover_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub album_image: TemplateChild<CoverPicture>,
        #[template_child]
        pub queue_length_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub drag_overlay: TemplateChild<DragOverlay>,
        #[template_child]
        pub playback_control: TemplateChild<PlaybackControl>,

        pub player: Rc<AudioPlayer>,
        pub provider: gtk::CssProvider,
        pub receiver: RefCell<Option<Receiver<WindowAction>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "AmberolWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action("win.play", None, move |win, _, _| {
                debug!("Window::win.play()");
                win.imp().player.toggle_play();
            });
            klass.install_action("win.previous", None, move |win, _, _| {
                debug!("Window::win.previous()");
                win.imp().player.skip_previous();
            });
            klass.install_action("win.next", None, move |win, _, _| {
                debug!("Window::win.next()");
                win.imp().player.skip_next();
            });
            klass.install_action("win.seek-backwards", None, move |win, _, _| {
                debug!("Window::win.seek-backwards()");
                win.imp().player.seek_backwards();
            });
            klass.install_action("win.seek-forward", None, move |win, _, _| {
                debug!("Window::win.seek-forward()");
                win.imp().player.seek_forward();
            });
            klass.install_action("queue.repeat-mode", None, move |win, _, _| {
                debug!("Window::queue.repeat()");
                win.imp().player.toggle_repeat_mode();
            });
            klass.install_action("queue.add-song", None, move |win, _, _| {
                debug!("Window::win.add-song()");
                win.add_song();
            });
            klass.install_action("queue.add-folder", None, move |win, _, _| {
                debug!("Window::win.add-folder()");
                win.add_folder();
            });
            klass.install_action("queue.clear", None, move |win, _, _| {
                debug!("Window::queue.clear()");
                win.clear_queue();
            });
            klass.install_action("queue.toggle", None, move |win, _, _| {
                debug!("Window::queue.toggle()");
                win.toggle_queue();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }

        fn new() -> Self {
            let (sender, r) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
            let receiver = RefCell::new(Some(r));

            Self {
                song_title_label: TemplateChild::default(),
                song_artist_label: TemplateChild::default(),
                song_album_label: TemplateChild::default(),
                song_time_label: TemplateChild::default(),
                queue_revealer: TemplateChild::default(),
                queue_view: TemplateChild::default(),
                cover_stack: TemplateChild::default(),
                album_image: TemplateChild::default(),
                queue_length_label: TemplateChild::default(),
                drag_overlay: TemplateChild::default(),
                playback_control: TemplateChild::default(),
                player: AudioPlayer::new(sender),
                provider: gtk::CssProvider::new(),
                receiver,
            }
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            if APPLICATION_ID.ends_with("Devel") {
                obj.add_css_class("devel");
            }

            obj.setup_channel();
            obj.set_initial_state();
            obj.bind_state();
            obj.bind_queue();
            obj.connect_signals();
            obj.setup_playlist();
            obj.setup_drop_target();
            obj.setup_provider();
            obj.restore_window_state();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Window {
    pub fn new<P: glib::IsA<gtk::Application>>(application: &P) -> Self {
        glib::Object::new(&[("application", application)]).expect("Failed to create Window")
    }

    fn setup_channel(&self) {
        let receiver = self.imp().receiver.borrow_mut().take().unwrap();
        receiver.attach(
            None,
            clone!(@strong self as this => move |action| this.process_action(action)),
        );
    }

    fn process_action(&self, action: WindowAction) -> glib::Continue {
        match action {
            WindowAction::Present => self.present(),
            // _ => debug!("Received action {:?}", action),
        }

        glib::Continue(true)
    }

    fn restore_window_state(&self) {
        // FIXME: https://gitlab.gnome.org/GNOME/gtk/-/issues/4136
        // let settings = utils::settings_manager();
        // let width = settings.int("window-width");
        // let height = settings.int("window-height");
        // self.set_default_size(width, height);
        self.set_default_size(400, -1);
    }

    fn clear_queue(&self) {
        let player = &self.imp().player;
        player.stop();
        player.state().set_current_song(None);
        player.queue().clear();
        self.imp().queue_revealer.set_reveal_child(false);
    }

    fn toggle_queue(&self) {
        let visible = self.imp().queue_revealer.reveals_child();
        self.imp().queue_revealer.set_reveal_child(!visible);
    }

    fn add_song(&self) {
        let app = gio::Application::default()
            .expect("Failed to retrieve application singleton")
            .downcast::<gtk::Application>()
            .unwrap();
        let win = app
            .active_window()
            .unwrap()
            .downcast::<gtk::Window>()
            .unwrap();
        let dialog = gtk::FileChooserNative::builder()
            .accept_label(&i18n("_Add Song"))
            .cancel_label(&i18n("_Cancel"))
            .modal(true)
            .title("Open File")
            .action(gtk::FileChooserAction::Open)
            .transient_for(&win)
            .build();

        let filter = gtk::FileFilter::new();
        gtk::FileFilter::set_name(&filter, Some(&i18n("Audio files")));
        filter.add_mime_type("audio/*");
        dialog.add_filter(&filter);

        dialog.connect_response(
            clone!(@strong dialog, @weak self as win => move |_, response| {
                if response == gtk::ResponseType::Accept {
                    win.add_files_to_queue(&dialog.files());
                }
            }),
        );
        dialog.show();
    }

    fn add_folder(&self) {
        let app = gio::Application::default()
            .expect("Failed to retrieve application singleton")
            .downcast::<gtk::Application>()
            .unwrap();
        let win = app
            .active_window()
            .unwrap()
            .downcast::<gtk::Window>()
            .unwrap();
        let dialog = gtk::FileChooserNative::builder()
            .accept_label(&i18n("_Add Folder"))
            .cancel_label(&i18n("_Cancel"))
            .modal(true)
            .title("Open Folder")
            .action(gtk::FileChooserAction::SelectFolder)
            .transient_for(&win)
            .build();

        dialog.connect_response(
            clone!(@strong dialog, @weak self as win => move |_, response| {
                if response == gtk::ResponseType::Accept {
                    win.add_folders_to_queue(&dialog.files());
                }
            }),
        );
        dialog.show();
    }

    fn add_folders_to_queue(&self, folders: &gio::ListModel) {
        for pos in 0..folders.n_items() {
            let folder = folders.item(pos).unwrap().downcast::<gio::File>().unwrap();
            self.add_folder_to_queue(&folder);
        }
    }

    fn add_files_to_queue(&self, files: &gio::ListModel) {
        for pos in 0..files.n_items() {
            let file = files.item(pos).unwrap().downcast::<gio::File>().unwrap();
            debug!("Adding {} to the queue", file.uri());
            self.add_file_to_queue(&file);
        }
    }

    pub fn add_file_to_queue(&self, file: &gio::File) {
        let queue = self.imp().player.queue();
        let song = Song::new(&file.uri());
        queue.add_song(&song);
    }

    pub fn add_folder_to_queue(&self, folder: &gio::File) {
        debug!("Adding the contents of {} to the queue", folder.uri());

        let mut files = utils::load_files_from_folder(folder, false).into_iter();
        glib::idle_add_local(clone!(@strong self as win => move || {
            let queue = win.imp().player.queue();

            files.next()
                .map(|f| {
                    let s = Song::new(f.uri().as_str());
                    if !s.equals(&Song::default()) {
                        let was_empty = queue.is_empty();
                        queue.add_song(&s);
                        if was_empty {
                            win.imp().player.skip_to(0);
                        }
                    }
                })
                .map(|_| glib::Continue(true))
                .unwrap_or_else(|| {
                    glib::Continue(false)
                })
        }));
    }

    // Bind the PlayerState to the UI
    fn bind_state(&self) {
        let imp = self.imp();
        let state = imp.player.state();

        // Use the PlayerState:playing property to control the play/pause button
        state.connect_notify_local(
            Some("playing"),
            clone!(@weak self as win => move |state, _| {
                let play_button = win.imp().playback_control.play_button();
                if state.playing() {
                    play_button.set_icon_name("media-playback-pause-symbolic");
                } else {
                    play_button.set_icon_name("media-playback-start-symbolic");
                }
            }),
        );
        // Update the position label
        state.connect_notify_local(
            Some("position"),
            clone!(@weak self as win => move |state, _| {
                if state.current_song().is_some() {
                    let position = state.position();
                    let duration = state.duration();
                    let time = utils::format_time(position, duration);
                    win.imp().song_time_label.set_label(&time);
                } else {
                    win.imp().song_time_label.set_label("");
                }
            }),
        );
        // Update the playlist time
        state.connect_notify_local(
            Some("song"),
            clone!(@weak self as win => move |state, _| {
                win.scroll_playlist_to_song();
                win.update_playlist_time();
                if let Some(current) = state.current_song() {
                    debug!("Updating style for {:?}", current);
                    win.imp().cover_stack.set_visible_child_name("cover-image");
                    win.update_style(&current);
                } else {
                    debug!("Reset album art");
                    win.imp().cover_stack.set_visible_child_name("no-image");
                    win.remove_css_class("main-window");
                }
            }),
        );
        // Bind the song properties to the UI
        state
            .bind_property("title", &imp.song_title_label.get(), "label")
            .flags(glib::BindingFlags::DEFAULT)
            .build();
        state
            .bind_property("artist", &imp.song_artist_label.get(), "label")
            .flags(glib::BindingFlags::DEFAULT)
            .build();
        state
            .bind_property("album", &imp.song_album_label.get(), "label")
            .flags(glib::BindingFlags::DEFAULT)
            .build();
        state
            .bind_property("cover", &imp.album_image.get(), "cover")
            .flags(glib::BindingFlags::DEFAULT)
            .build();
        state
            .bind_property(
                "volume",
                &imp.playback_control.get().volume_control(),
                "volume",
            )
            .flags(glib::BindingFlags::DEFAULT)
            .build();
    }

    // Bind the Queue to the UI
    fn bind_queue(&self) {
        let queue = self.imp().player.queue();

        queue.connect_notify_local(
            Some("n-songs"),
            clone!(@weak self as win => move |queue, _| {
                if queue.is_empty() {
                    win.set_initial_state();
                } else {
                    win.action_set_enabled("queue.toggle", queue.n_songs() > 1);
                    win.imp().playback_control.get().shuffle_button().set_sensitive(queue.n_songs() > 1);

                    win.action_set_enabled("win.play", true);
                    win.action_set_enabled("win.seek-backwards", true);
                    win.action_set_enabled("win.seek-forward", true);
                    win.action_set_enabled("win.previous", true);
                    win.action_set_enabled("win.next", queue.n_songs() > 1);
                }

                if queue.n_songs() == 1 {
                    win.imp().player.skip_next();
                }

                win.update_playlist_time();

                if queue.n_songs() > 1 {
                    win.imp().queue_revealer.set_reveal_child(true);
                }
            }),
        );
        queue.connect_notify_local(
            Some("repeat-mode"),
            clone!(@weak self as win => move |queue, _| {
                let imp = win.imp();
                let repeat_button = imp.playback_control.repeat_button();
                match queue.repeat_mode() {
                    RepeatMode::Consecutive => {
                        repeat_button.set_icon_name("media-playlist-consecutive-symbolic");
                    },
                    RepeatMode::RepeatAll => {
                        repeat_button.set_icon_name("media-playlist-repeat-symbolic");
                    },
                    RepeatMode::RepeatOne => {
                        repeat_button.set_icon_name("media-playlist-repeat-song-symbolic");
                    },
                }
            }),
        );
        queue.connect_notify_local(
            Some("current"),
            clone!(@weak self as win => move |queue, _| {
                if queue.is_last_song() {
                    win.action_set_enabled("win.next", false);
                } else {
                    win.action_set_enabled("win.next", true);
                }
            }),
        );
    }

    fn connect_signals(&self) {
        self.imp().queue_revealer.connect_notify_local(
            Some("child-revealed"),
            clone!(@weak self as win => move |_, _| {
                let width = win.default_size().0;
                win.set_default_size(width, -1);
            }),
        );

        let shuffle_button = self.imp().playback_control.shuffle_button();
        shuffle_button.connect_toggled(clone!(@weak self as win => move |toggle_button| {
            let queue = win.imp().player.queue();
            queue.set_shuffle(toggle_button.is_active());
        }));

        let volume_control = self.imp().playback_control.volume_control();
        volume_control.connect_notify_local(
            Some("volume"),
            clone!(@weak self as win => move |control, _| {
                win.imp().player.set_volume(control.volume());
            }),
        );

        self.connect_close_request(move |window| {
            debug!("Saving window state");
            let width = window.default_size().0;
            let height = window.default_size().1;

            let settings = utils::settings_manager();
            settings
                .set_int("window-width", width)
                .expect("Unable to store window-width");
            settings
                .set_int("window-height", height)
                .expect("Unable to stop window-height");

            glib::signal::Inhibit(false)
        });
    }

    // The initial state of the playback actions
    fn set_initial_state(&self) {
        self.action_set_enabled("win.play", false);
        self.action_set_enabled("win.pause", false);
        self.action_set_enabled("win.previous", false);
        self.action_set_enabled("win.next", false);
        self.action_set_enabled("win.seek-backwards", false);
        self.action_set_enabled("win.seek-forward", false);
        self.action_set_enabled("queue.toggle", false);

        // Not an action, so we need direct access to the widget
        self.imp()
            .playback_control
            .get()
            .shuffle_button()
            .set_sensitive(false);
    }

    fn setup_playlist(&self) {
        let imp = self.imp();

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, list_item| {
            let row = QueueRow::default();
            list_item.set_child(Some(&row));

            list_item
                .property_expression("item")
                .chain_property::<Song>("artist")
                .bind(&row, "song-artist", gtk::Widget::NONE);
            list_item
                .property_expression("item")
                .chain_property::<Song>("title")
                .bind(&row, "song-title", gtk::Widget::NONE);
            list_item
                .property_expression("item")
                .chain_property::<Song>("playing")
                .bind(&row, "playing", gtk::Widget::NONE);
        });
        imp.queue_view
            .set_factory(Some(&factory.upcast::<gtk::ListItemFactory>()));

        let queue = imp.player.queue();
        let selection = gtk::SingleSelection::new(Some(queue.model()));
        selection.set_can_unselect(false);
        selection.set_selected(gtk::INVALID_LIST_POSITION);
        imp.queue_view
            .set_model(Some(&selection.upcast::<gtk::SelectionModel>()));
        imp.queue_view
            .connect_activate(clone!(@weak self as win => move |_, pos| {
                win.imp().player.skip_to(pos);
                win.imp().player.play();
            }));
    }

    fn setup_drop_target(&self) {
        let drop_target = gtk::DropTarget::builder()
            .name("file-drop-target")
            .actions(gdk::DragAction::COPY)
            .formats(&gdk::ContentFormats::for_type(gdk::FileList::static_type()))
            .build();

        drop_target.connect_drop(
            clone!(@weak self as win => @default-return false, move |_, value, _, _| {
                if let Ok(file_list) = value.get::<gdk::FileList>() {
                    for f in file_list.files() {
                        if let Ok(info) = f.query_info("standard::*", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE) {
                            if info.file_type() == gio::FileType::Regular {
                                win.add_file_to_queue(&f);
                            } else if info.file_type() == gio::FileType::Directory {
                                win.add_folder_to_queue(&f);
                            } else {
                                warn!("Unsupported file type {:?} for file '{}'", info.file_type(), f.uri());
                            }
                        }
                    }

                    return true;
                }

                false
            }),
        );

        self.imp().drag_overlay.set_drop_target(&drop_target);
    }

    fn update_playlist_time(&self) {
        let title = format!("<b>{}</b>", &i18n("Playlist"));

        let queue = self.imp().player.queue();
        let n_songs = queue.n_songs();
        if let Some(current) = queue.current_song_index() {
            let mut remaining_time = 0;
            for pos in 0..n_songs {
                let song = queue.song_at(pos).unwrap();
                if pos >= current {
                    remaining_time += song.duration();
                }
            }

            let remaining_min = (remaining_time / 60) as u32;
            let remaining_str = &ni18n_f(
                // Translators: The first '{}' is the word "Playlist";
                // the second '{}' is the number of minutes remaining
                // in the playlist
                "{} ({} minute remaining)",
                "{} ({} minutes remaining)",
                remaining_min,
                &[&title, &remaining_min.to_string()],
            );
            self.imp().queue_length_label.set_label(remaining_str);
        } else {
            self.imp().queue_length_label.set_label(&title);
        }
    }

    fn scroll_playlist_to_song(&self) {
        let queue_view = self.imp().queue_view.get();
        if let Some(current_idx) = self.imp().player.queue().current_song_index() {
            debug!("Scrolling playlist to {}", current_idx);
            queue_view
                .upcast_ref::<gtk::Widget>()
                .activate_action("list.scroll-to-item", Some(&current_idx.to_variant()))
                .expect("Failed to activate action");
        }
    }

    fn setup_provider(&self) {
        let imp = self.imp();
        if let Some(display) = gdk::Display::default() {
            gtk::StyleContext::add_provider_for_display(&display, &imp.provider, 400);
        }
    }

    fn update_style(&self, song: &Song) {
        let imp = self.imp();

        if let Some(bg_colors) = song.cover_palette() {
            // The color chosen depends on the linear gradient we use in the
            // style, so remember to change this when changing the main-window
            // CSS class
            let fg_color = if utils::is_color_dark(&bg_colors[1]) {
                gdk::RGBA::parse("#ffffff").unwrap()
            } else {
                gdk::RGBA::parse("rgba(0, 0, 0, 0.8)").unwrap()
            };

            let mut css = String::new();

            let n_colors = bg_colors.len();
            for (i, color) in bg_colors.iter().enumerate().take(n_colors) {
                css.push_str(&format!("@define-color background_color_{} {};", i, color));
            }

            css.push_str(&format!("@define-color foreground_color {};", fg_color));

            imp.provider.load_from_data(css.as_bytes());
            if !self.has_css_class("main-window") {
                self.add_css_class("main-window");
            }
        } else {
            self.remove_css_class("main-window");
        }
    }

    pub fn open_file(&self, file: &gio::File) {
        self.add_file_to_queue(file);
        let queue = self.imp().player.queue();
        if queue.n_songs() == 1 {
            self.imp().player.play();
        }
    }
}
