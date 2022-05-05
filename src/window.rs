// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::subclass::prelude::*;
use glib::{clone, closure_local, Receiver};
use gtk::{gdk, gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use crate::{
    audio::{AudioPlayer, RepeatMode, Song, WaveformGenerator},
    config::APPLICATION_ID,
    drag_overlay::DragOverlay,
    i18n::{i18n, i18n_f, ni18n_f},
    playback_control::PlaybackControl,
    playlist_view::PlaylistView,
    queue_row::QueueRow,
    song_details::SongDetails,
    utils,
    waveform_view::WaveformView,
};

pub enum WindowAction {
    Present,
}

mod imp {
    use glib::{ParamFlags, ParamSpec, ParamSpecBoolean, Value};
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(CompositeTemplate)]
    #[template(resource = "/io/bassi/Amberol/window.ui")]
    pub struct Window {
        // Template widgets
        #[template_child]
        pub drag_overlay: TemplateChild<DragOverlay>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub song_details: TemplateChild<SongDetails>,
        #[template_child]
        pub playback_control: TemplateChild<PlaybackControl>,
        #[template_child]
        pub queue_revealer: TemplateChild<adw::Flap>,
        #[template_child]
        pub playlist_view: TemplateChild<PlaylistView>,

        pub player: Rc<AudioPlayer>,
        pub provider: gtk::CssProvider,
        pub receiver: RefCell<Option<Receiver<WindowAction>>>,
        pub waveform: WaveformGenerator,
        pub settings: gio::Settings,

        pub playlist_shuffled: Cell<bool>,
        pub playlist_visible: Cell<bool>,
        pub playlist_selection: Cell<bool>,
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
            klass.install_property_action("queue.toggle", "playlist-visible");
            klass.install_property_action("queue.shuffle", "playlist-shuffled");
            klass.install_property_action("queue.select", "playlist-selection");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }

        fn new() -> Self {
            let (sender, r) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
            let receiver = RefCell::new(Some(r));

            Self {
                song_details: TemplateChild::default(),
                queue_revealer: TemplateChild::default(),
                toast_overlay: TemplateChild::default(),
                drag_overlay: TemplateChild::default(),
                playback_control: TemplateChild::default(),
                main_stack: TemplateChild::default(),
                status_page: TemplateChild::default(),
                playlist_view: TemplateChild::default(),
                playlist_shuffled: Cell::new(false),
                playlist_visible: Cell::new(true),
                playlist_selection: Cell::new(false),
                player: AudioPlayer::new(sender),
                waveform: WaveformGenerator::default(),
                provider: gtk::CssProvider::new(),
                settings: utils::settings_manager(),
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
            obj.setup_waveform();
            obj.set_initial_state();
            obj.bind_state();
            obj.bind_queue();
            obj.connect_signals();
            obj.setup_playlist();
            obj.setup_drop_target();
            obj.setup_provider();
            obj.restore_window_state();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecBoolean::new(
                        "playlist-shuffled",
                        "",
                        "",
                        false,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new("playlist-visible", "", "", false, ParamFlags::READWRITE),
                    ParamSpecBoolean::new(
                        "playlist-selection",
                        "",
                        "",
                        false,
                        ParamFlags::READWRITE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "playlist-shuffled" => obj.set_playlist_shuffled(value.get::<bool>().unwrap()),
                "playlist-visible" => obj.set_playlist_visible(value.get::<bool>().unwrap()),
                "playlist-selection" => obj.set_playlist_selection(value.get::<bool>().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "playlist-shuffled" => obj.playlist_shuffled().to_value(),
                "playlist-visible" => obj.playlist_visible().to_value(),
                "playlist-selection" => obj.playlist_selection().to_value(),
                _ => unimplemented!(),
            }
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

    fn setup_waveform(&self) {
        self.imp().waveform.connect_notify_local(
            Some("has-peaks"),
            clone!(@strong self as win => move |gen, _| {
                let peaks = gen.peaks();
                win.imp().playback_control.waveform_view().set_peaks(peaks);
            }),
        );
    }

    fn restore_window_state(&self) {
        // FIXME: https://gitlab.gnome.org/GNOME/gtk/-/issues/4136
        // let settings = utils::settings_manager();
        // let width = settings.int("window-width");
        // let height = settings.int("window-height");
        // self.set_default_size(width, height);
        self.set_default_size(600, -1);
    }

    fn clear_queue(&self) {
        self.set_playlist_visible(false);
        self.set_playlist_shuffled(false);
        self.set_playlist_selection(false);
        self.update_waveform(None);
        self.update_style(None);

        let player = &self.imp().player;
        let queue = player.queue();
        let state = player.state();

        player.stop();

        state.set_current_song(None);
        queue.clear();
    }

    fn playlist_visible(&self) -> bool {
        self.imp().playlist_visible.get()
    }

    fn set_playlist_visible(&self, visible: bool) {
        if visible != self.imp().playlist_visible.replace(visible) {
            self.imp().queue_revealer.set_reveal_flap(visible);
            self.notify("playlist-visible");
        }
    }

    fn playlist_shuffled(&self) -> bool {
        self.imp().playlist_shuffled.get()
    }

    fn set_playlist_shuffled(&self, shuffled: bool) {
        let imp = self.imp();

        if shuffled != imp.playlist_shuffled.replace(shuffled) {
            let queue = imp.player.queue();
            let state = imp.player.state();

            let reset_song = queue.is_first_song() && !state.playing();

            queue.set_shuffle(shuffled);

            if reset_song {
                imp.player.skip_to(0);
            }

            self.notify("playlist-shuffled");
        }
    }

    fn playlist_selection(&self) -> bool {
        self.imp().playlist_selection.get()
    }

    fn set_playlist_selection(&self, selection: bool) {
        let imp = self.imp();

        if selection != imp.playlist_selection.replace(selection) {
            if !selection {
                let queue = imp.player.queue();
                queue.unselect_all_songs();
            }

            self.imp()
                .playlist_view
                .queue_actionbar()
                .set_revealed(selection);

            self.notify("playlist-selection");
        }
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
        if let Ok(info) = file.query_info(
            "standard::*",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        ) {
            if info.file_type() != gio::FileType::Regular {
                let msg = i18n_f("Unrecognized file type for “{}”", &[&info.display_name()]);
                self.add_toast(msg);
                return;
            }
            if let Some(content_type) = info.content_type() {
                if !gio::content_type_is_a(&content_type, "audio/*") {
                    let msg = i18n_f(
                        "“{}” is not a supported audio file",
                        &[&info.display_name()],
                    );
                    self.add_toast(msg);
                    return;
                }
            }
        }

        let queue = self.imp().player.queue();
        let was_empty = queue.is_empty();

        let song = Song::new(&file.uri());
        queue.add_song(&song);

        if was_empty {
            self.imp().player.skip_to(0);
        }
    }

    pub fn add_folder_to_queue(&self, folder: &gio::File) {
        debug!("Adding the contents of {} to the queue", folder.uri());

        let mut files = utils::load_files_from_folder(folder, true).into_iter();
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
                win.set_playlist_selection(false);
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
                    let elapsed = state.position();
                    let remaining = state.duration().checked_sub(state.position()).unwrap_or_default();
                    win.imp().playback_control.set_elapsed(Some(elapsed));
                    win.imp().playback_control.set_remaining(Some(remaining));
                    let pos = state.position() as f64 / state.duration() as f64;
                    win.imp().playback_control.waveform_view().set_position(pos);
                } else {
                    win.imp().playback_control.set_elapsed(None);
                    win.imp().playback_control.set_remaining(None);
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
                    debug!("Updating waveform for {}", &current);
                    win.update_waveform(Some(&current));
                    debug!("Updating style for {}", &current);
                    win.update_style(Some(&current));
                } else {
                    win.update_waveform(None);
                    debug!("Reset album art");
                    win.remove_css_class("main-window");
                }
            }),
        );
        // Update the cover, if any is available
        state.connect_notify_local(
            Some("cover"),
            clone!(@weak self as win => move |state, _| {
                let song_details = win.imp().song_details.get();
                if let Some(cover) = state.cover() {
                    song_details.album_image().set_cover(Some(&cover));
                    song_details.show_cover_image(true);
                } else {
                    song_details.album_image().set_cover(None);
                    song_details.show_cover_image(false);
                }
            }),
        );
        // Bind the song properties to the UI
        state
            .bind_property("title", &imp.song_details.get().title_label(), "label")
            .flags(glib::BindingFlags::DEFAULT)
            .build();
        state
            .bind_property("artist", &imp.song_details.get().artist_label(), "label")
            .flags(glib::BindingFlags::DEFAULT)
            .build();
        state
            .bind_property("album", &imp.song_details.get().album_label(), "label")
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
                    win.imp().main_stack.get().set_visible_child_name("main-view");

                    win.action_set_enabled("queue.toggle", queue.n_songs() > 1);
                    win.action_set_enabled("queue.shuffle", queue.n_songs() > 1);

                    win.action_set_enabled("win.play", true);
                    win.action_set_enabled("win.previous", true);
                    win.action_set_enabled("win.next", queue.n_songs() > 1);
                }

                if queue.n_songs() == 1 {
                    win.imp().player.skip_next();
                }

                win.update_playlist_time();
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
                        repeat_button.set_tooltip_text(Some(&i18n("Enable repeat")));
                    },
                    RepeatMode::RepeatAll => {
                        repeat_button.set_icon_name("media-playlist-repeat-symbolic");
                        repeat_button.set_tooltip_text(Some(&i18n("Repeat all tracks")));
                    },
                    RepeatMode::RepeatOne => {
                        repeat_button.set_icon_name("media-playlist-repeat-song-symbolic");
                        repeat_button.set_tooltip_text(Some(&i18n("Repeat the current track")));
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
            Some("folded"),
            clone!(@weak self as win => move |flap, _| {
                win.set_playlist_visible(flap.reveals_flap());
                if flap.is_folded() {
                    win.imp().playlist_view.back_button().set_visible(win.playlist_visible());
                } else {
                    win.imp().playlist_view.back_button().set_visible(false);
                }
            }),
        );

        self.imp().queue_revealer.connect_notify_local(
            Some("reveal-flap"),
            clone!(@weak self as win => move |flap, _| {
                win.set_playlist_visible(flap.reveals_flap());
                if flap.is_folded() {
                    win.imp().playlist_view.back_button().set_visible(win.playlist_visible());
                } else {
                    win.imp().playlist_view.back_button().set_visible(false);
                }
            }),
        );

        let volume_control = self.imp().playback_control.volume_control();
        volume_control.connect_notify_local(
            Some("volume"),
            clone!(@weak self as win => move |control, _| {
                win.imp().player.set_volume(control.volume());
            }),
        );

        let waveform_view = self.imp().playback_control.waveform_view();
        waveform_view.connect_closure(
            "position-changed",
            false,
            closure_local!(@strong self as this => move |_view: WaveformView, position: f64| {
                debug!("New position: {}", position);
                this.imp().player.seek_position_rel(position);
                this.imp().player.play();
            }),
        );

        self.imp()
            .playlist_view
            .queue_remove_button()
            .connect_clicked(clone!(@weak self as win => move |_| {
                let queue = win.imp().player.queue();
                let mut remove_songs: Vec<Song> = Vec::new();
                // Collect all songs to be removed first, since we can't
                // remove objects from the model while we're iterating it
                for idx in 0..queue.n_songs() {
                    let song = queue.song_at(idx).unwrap();
                    if song.selected() {
                        remove_songs.push(song);
                    }
                }

                for song in remove_songs {
                    win.remove_song(&song);
                }
            }));

        self.imp().settings.connect_changed(
            Some("enable-recoloring"),
            clone!(@strong self as this => move |settings, _| {
                debug!("GSettings:enable-recoloring: {}", settings.boolean("enable-recoloring"));
                let state = this.imp().player.state();
                this.update_style(state.current_song().as_ref());
            }),
        );
        let _dummy = self.imp().settings.boolean("enable-recoloring");

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
        self.action_set_enabled("win.previous", false);
        self.action_set_enabled("win.next", false);

        self.action_set_enabled("queue.toggle", false);
        self.action_set_enabled("queue.shuffle", false);

        // Manually update the icon on the initial empty state
        // to avoid generating the UI definition file at build
        // time
        self.imp().status_page.set_icon_name(Some(APPLICATION_ID));
    }

    fn setup_playlist(&self) {
        let imp = self.imp();

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(clone!(@strong self as win => move |_, list_item| {
            let row = QueueRow::default();
            list_item.set_child(Some(&row));

            row.connect_notify_local(
                Some("selected"),
                clone!(@weak win => move |_, _| {
                    win.update_selected_count();
                }),
            );

            win
                .bind_property("playlist-selection", &row, "selection-mode")
                .flags(glib::BindingFlags::DEFAULT)
                .build();

            list_item
                .bind_property("item", &row, "song")
                .flags(glib::BindingFlags::DEFAULT)
                .build();

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
                .chain_property::<Song>("cover")
                .bind(&row, "song-cover", gtk::Widget::NONE);
            list_item
                .property_expression("item")
                .chain_property::<Song>("playing")
                .bind(&row, "playing", gtk::Widget::NONE);
            list_item
                .property_expression("item")
                .chain_property::<Song>("selected")
                .bind(&row, "selected", gtk::Widget::NONE);
        }));
        imp.playlist_view
            .queue_view()
            .set_factory(Some(&factory.upcast::<gtk::ListItemFactory>()));

        let queue = imp.player.queue();
        let selection = gtk::SingleSelection::new(Some(queue.model()));
        selection.set_can_unselect(false);
        selection.set_selected(gtk::INVALID_LIST_POSITION);
        imp.playlist_view
            .queue_view()
            .set_model(Some(&selection.upcast::<gtk::SelectionModel>()));
        imp.playlist_view.queue_view().connect_activate(
            clone!(@weak self as win => move |_, pos| {
                let queue = win.imp().player.queue();
                if win.playlist_selection() {
                    queue.select_song_at(pos);
                } else if queue.current_song_index() != Some(pos) {
                    win.imp().player.skip_to(pos);
                    win.imp().player.play();
                }
            }),
        );
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
                                let msg = i18n_f("Unrecognized file type for “{}”", &[&info.display_name()]);
                                win.add_toast(msg);
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
                // Translators: the '{}' must be left unmodified, and
                // it will be replaced by the number of minutes remaining
                // in the playlist
                "{} minute remaining",
                "{} minutes remaining",
                remaining_min,
                &[&remaining_min.to_string()],
            );
            self.imp()
                .playlist_view
                .queue_length_label()
                .set_label(remaining_str);
            self.imp().playlist_view.queue_length_label().show();
        } else {
            self.imp().playlist_view.queue_length_label().hide();
        }
    }

    fn scroll_playlist_to_song(&self) {
        let queue_view = self.imp().playlist_view.queue_view();
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

    fn update_style(&self, song: Option<&Song>) {
        let imp = self.imp();

        if !imp.settings.boolean("enable-recoloring") {
            imp.provider.load_from_data(&[]);
            imp.main_stack.remove_css_class("main-window");
            return;
        }

        if let Some(song) = song {
            if let Some(bg_colors) = song.cover_palette() {
                // The color chosen depends on the linear gradient we use in the
                // style, so remember to change this when changing the main-window
                // CSS class
                let fg_main_color =
                    if utils::is_color_dark(&bg_colors[0]) || utils::is_color_dark(&bg_colors[2]) {
                        gdk::RGBA::parse("#ffffff").unwrap()
                    } else {
                        gdk::RGBA::parse("rgba(0, 0, 0, 0.8)").unwrap()
                    };
                let fg_playlist_color = if utils::is_color_dark(&bg_colors[1]) {
                    gdk::RGBA::parse("#ffffff").unwrap()
                } else {
                    gdk::RGBA::parse("rgba(0, 0, 0, 0.8)").unwrap()
                };

                let mut css = String::new();

                let n_colors = bg_colors.len();
                for (i, color) in bg_colors.iter().enumerate().take(n_colors) {
                    let s = format!("@define-color background_color_{} {};", i, color);
                    css.push_str(&s);
                }

                for i in n_colors - 1..5 {
                    css.push_str(&format!(
                        "@define-color background_color_{} @window_bg_color;",
                        i
                    ));
                }

                // We compute the complementary of the dominant color in the palette; then we
                // try to find the closest color in the palette that we can use
                let complementary = utils::complementary_color(&bg_colors[0]);
                let mut near_color: Option<gdk::RGBA> = None;
                let mut min_near: f32 = f32::MAX;
                for color in bg_colors {
                    let delta_e = utils::color_distance(&color, &complementary);
                    if delta_e < min_near {
                        min_near = delta_e;
                        near_color = Some(color);
                    }
                }

                if let Some(near_color) = near_color {
                    css.push_str(&format!(
                        "@define-color complementary_color {};",
                        near_color
                    ));
                } else {
                    css.push_str(&format!(
                        "@define-color complementary_color {};",
                        complementary
                    ));
                }

                css.push_str(&format!(
                    "@define-color foreground_main_color {};",
                    fg_main_color
                ));
                css.push_str(&format!(
                    "@define-color foreground_playlist_color {};",
                    fg_playlist_color
                ));

                imp.provider.load_from_data(css.as_bytes());
                if !imp.main_stack.has_css_class("main-window") {
                    imp.main_stack.add_css_class("main-window");
                }

                return;
            }
        }

        imp.provider.load_from_data(&[]);
        imp.main_stack.remove_css_class("main-window");
    }

    fn update_waveform(&self, song: Option<&Song>) {
        let imp = self.imp();

        imp.playback_control.waveform_view().set_peaks(None);
        if let Some(song) = song {
            imp.waveform.set_uri(Some(song.uri()));
            if !imp.waveform.generate_peaks() {
                imp.waveform.set_uri(None);
            }
        } else {
            imp.waveform.set_uri(None);
        }
    }

    fn update_selected_count(&self) {
        let queue = self.imp().player.queue();
        let n_selected = queue.n_selected_songs();

        let selected_str = if n_selected == 0 {
            i18n("No song selected")
        } else {
            ni18n_f(
                // Translators: The '{}' must be left unmodified, and
                // it is expanded to the number of songs selected
                "{} song selected",
                "{} songs selected",
                n_selected,
                &[&n_selected.to_string()],
            )
        };

        self.imp()
            .playlist_view
            .queue_selected_label()
            .set_label(&selected_str);
    }

    pub fn open_file(&self, file: &gio::File) {
        self.add_file_to_queue(file);
        let queue = self.imp().player.queue();
        if queue.n_songs() == 1 {
            self.imp().player.play();
        }
    }

    pub fn remove_song(&self, song: &Song) {
        let imp = self.imp();
        if song.playing() {
            imp.player.skip_next();
        }
        let queue = imp.player.queue();
        queue.remove_song(song);

        self.update_selected_count();
    }

    pub fn add_toast(&self, msg: String) {
        let toast = adw::Toast::new(&msg);
        self.imp().toast_overlay.add_toast(&toast);
    }
}
