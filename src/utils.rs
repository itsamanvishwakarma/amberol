// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(dead_code)]

use color_thief::{get_palette, ColorFormat};
use gtk::{gdk, gio, glib, prelude::*};

use crate::{audio::Song, config::APPLICATION_ID};

pub fn settings_manager() -> gio::Settings {
    // We ship a single schema for both default and development profiles
    let app_id = APPLICATION_ID.trim_end_matches(".Devel");
    gio::Settings::new(app_id)
}

pub fn format_time(seconds: u64, total: u64) -> String {
    format!(
        "{}:{:02} / {}:{:02}",
        (seconds - (seconds % 60)) / 60,
        seconds % 60,
        (total - (total % 60)) / 60,
        total % 60
    )
}

pub fn is_color_dark(color: &gdk::RGBA) -> bool {
    let lum = color.red() * 0.2126 + color.green() * 0.7152 + color.blue() * 0.072;

    lum < 0.5
}

pub fn load_cover_texture(buffer: &glib::Bytes) -> Option<gdk_pixbuf::Pixbuf> {
    let stream = gio::MemoryInputStream::from_bytes(buffer);
    match gdk_pixbuf::Pixbuf::from_stream_at_scale(&stream, 256, 256, true, gio::Cancellable::NONE)
    {
        Ok(pixbuf) => Some(pixbuf),
        Err(_) => None,
    }
}

fn color_format(has_alpha: bool) -> ColorFormat {
    if has_alpha {
        ColorFormat::Rgba
    } else {
        ColorFormat::Rgb
    }
}

pub fn load_palette(pixbuf: &gdk_pixbuf::Pixbuf) -> Option<Vec<gdk::RGBA>> {
    if let Ok(palette) = get_palette(
        pixbuf.pixel_bytes().unwrap().as_ref(),
        color_format(pixbuf.has_alpha()),
        5,
        4,
    ) {
        let colors: Vec<gdk::RGBA> = palette
            .iter()
            .map(|c| {
                gdk::RGBA::new(
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    1.0,
                )
            })
            .collect();

        return Some(colors);
    }

    None
}

pub fn load_files_from_folder(folder: &gio::File, recursive: bool) -> Vec<gio::File> {
    let mut enumerator = folder
        .enumerate_children(
            "standard::*",
            gio::FileQueryInfoFlags::NONE,
            None::<&gio::Cancellable>,
        )
        .expect("Unable to enumerate");

    let mut files = Vec::new();
    while let Some(info) = enumerator.next().and_then(|s| s.ok()) {
        let child = enumerator.child(&info);
        if recursive && info.file_type() == gio::FileType::Directory {
            let mut res = load_files_from_folder(&child, recursive);
            files.append(&mut res);
        } else if info.file_type() == gio::FileType::Regular {
            if let Some(content_type) = info.content_type() {
                if gio::content_type_is_a(&content_type, "audio/*") {
                    let child = enumerator.child(&info);
                    debug!("Adding {} to the queue", child.uri());
                    files.push(child.clone());
                }
            }
        }
    }

    // gio::FileEnumerator has no guaranteed order, so we should
    // rely on the basename being formatted in a way that gives us an
    // implicit order; if anything, this will queue songs in the same
    // order in which they appear in the directory when browsing its
    // contents
    files.sort_by(|a, b| {
        let parent_a = a.parent().unwrap();
        let parent_b = b.parent().unwrap();
        let parent_basename_a = parent_a.basename().unwrap();
        let parent_basename_b = parent_b.basename().unwrap();
        let basename_a = a.basename().unwrap();
        let basename_b = b.basename().unwrap();
        let key_a = format!(
            "{}-{}",
            parent_basename_a.to_string_lossy(),
            basename_a.to_string_lossy()
        );
        let key_b = format!(
            "{}-{}",
            parent_basename_b.to_string_lossy(),
            basename_b.to_string_lossy()
        );
        key_a.partial_cmp(&key_b).unwrap()
    });

    files
}

pub fn load_songs_from_folder(folder: &gio::File) -> Vec<Song> {
    let files = load_files_from_folder(folder, false);

    let songs: Vec<Song> = files
        .iter()
        .map(|f| Song::new(f.uri().as_str()))
        .filter(|s| !s.equals(&Song::default()))
        .collect();

    songs
}
