#![windows_subsystem = "windows"]

extern crate cairo;
#[macro_use]
extern crate clap;
extern crate dirs as env_dirs;
extern crate env_logger;
extern crate gdk;
extern crate gdk_sys;
extern crate gio;
extern crate glib;
extern crate glib_sys as glib_ffi;
extern crate gobject_sys as gobject_ffi;
extern crate gtk;
extern crate gtk_sys;
extern crate htmlescape;
#[cfg(unix)]
extern crate unix_daemonize;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate neovim_lib;
extern crate pango;
extern crate pango_cairo_sys;
extern crate pango_sys;
extern crate pangocairo;
extern crate percent_encoding;
extern crate phf;
extern crate regex;
extern crate rmpv;
extern crate unicode_segmentation;
extern crate unicode_width;

extern crate atty;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate toml;

mod sys;

mod color;
mod dirs;
mod mode;
mod nvim_config;
mod theme;
mod ui_model;
mod value;
#[macro_use]
mod ui;
mod cmd_line;
mod cursor;
mod error;
mod file_browser;
mod input;
mod misc;
mod nvim;
mod plug_manager;
mod popup_menu;
mod project;
mod render;
mod settings;
mod shell;
mod shell_dlg;
mod subscriptions;
mod tabline;

use gio::prelude::*;
use std::cell::RefCell;
use std::io::Read;
#[cfg(unix)]
use unix_daemonize::{daemonize_redirect, ChdirMode};

use ui::Ui;

use clap::{App, Arg, ArgMatches};
use shell::ShellOptions;

fn main() {
    env_logger::init();

    let matches = App::new("NeovimGtk")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(misc::about_comments().as_str())
        .arg(Arg::with_name("no-fork")
             .long("no-fork")
             .help("Prevent detach from console"))
        .arg(Arg::with_name("disable-win-restore")
             .long("disable-win-restore")
             .help("Don't restore window size at start"))
        .arg(Arg::with_name("timeout")
             .long("timeout")
             .default_value("10")
             .help("Wait timeout in seconds. If nvim does not response in given time NvimGtk stops")
            .takes_value(true))
        .arg(
            Arg::with_name("enable-swap")
                .long("enable-swap")
                .help("Enable swap files"),
        ).arg(Arg::with_name("files").help("Files to open").multiple(true))
        .arg(
            Arg::with_name("nvim-bin-path")
                .long("nvim-bin-path")
                .help("Path to nvim binary")
                .takes_value(true),
        ).arg(
            Arg::with_name("nvim-args")
                .help("Args will be passed to nvim")
                .last(true)
                .multiple(true),
        ).get_matches();

    let input_data = RefCell::new(read_piped_input());

    #[cfg(unix)]
    {
        // fork to background by default
        if !matches.is_present("no-fork") {
            daemonize_redirect(
                Some("/tmp/nvim-gtk_stdout.log"),
                Some("/tmp/nvim-gtk_stderr.log"),
                ChdirMode::NoChdir,
            ).unwrap();
        }
    }

    let app_flags = gio::ApplicationFlags::HANDLES_OPEN | gio::ApplicationFlags::NON_UNIQUE;

    glib::set_program_name(Some("NeovimGtk"));

    let app = if cfg!(debug_assertions) {
        gtk::Application::new(Some("org.daa.NeovimGtkDebug"), app_flags)
    } else {
        gtk::Application::new(Some("org.daa.NeovimGtk"), app_flags)
    }.expect("Failed to initialize GTK application");

    let matches_copy = matches.clone();
    app.connect_activate(move |app| activate(app, &matches_copy, input_data.replace(None)));

    let matches_copy = matches.clone();
    app.connect_open(move |app, files, _| open(app, files, &matches_copy));

    let app_ref = app.clone();
    let matches_copy = matches.clone();
    let new_window_action = gio::SimpleAction::new("new-window", None);
    new_window_action.connect_activate(move |_, _| activate(&app_ref, &matches_copy, None));
    app.add_action(&new_window_action);

    gtk::Window::set_default_icon_name("org.daa.NeovimGtk");

    let app_exe = std::env::args().next().unwrap_or("nvim-gtk".to_owned());

    app.run(
        &std::iter::once(app_exe)
            .chain(
                matches
                    .values_of("files")
                    .unwrap_or(clap::Values::default())
                    .map(str::to_owned),
            ).collect::<Vec<String>>(),
    );
}

fn open(app: &gtk::Application, files: &[gio::File], matches: &ArgMatches) {
    let files_list: Vec<String> = files
        .into_iter()
        .filter_map(|f| f.get_path()?.to_str().map(str::to_owned))
        .collect();

    let mut ui = Ui::new(ShellOptions::new(matches, files_list, None));

    ui.init(app, !matches.is_present("disable-win-restore"));
}

fn activate(app: &gtk::Application, matches: &ArgMatches, input_data: Option<String>) {
    let mut ui = Ui::new(ShellOptions::new(matches, Vec::new(), input_data));

    ui.init(app, !matches.is_present("disable-win-restore"));
}

fn read_piped_input() -> Option<String> {
    if atty::isnt(atty::Stream::Stdin) {
        let mut buf = String::new();
        match std::io::stdin().read_to_string(&mut buf) {
            Ok(size) if size > 0 => Some(buf),
            Ok(_) => None,
            Err(err) => {
                error!("Error read stdin {}", err);
                None
            }
        }
    } else {
        None
    }
}
