#[macro_use]
mod macros;

#[rustfmt::skip]
mod config;
mod globals;
mod service;
mod ui;

use crate::ui::application::WarpApplication;
use gettextrs::{gettext, LocaleCategory};
use gtk::glib;
use once_cell::sync::Lazy;
use std::thread;

use self::config::{GETTEXT_PACKAGE, LOCALEDIR};

fn main() {
    // Initialize logger
    pretty_env_logger::init();

    // Initialized Twisted in separate thread
    thread::spawn(move || Lazy::force(&globals::TWISTED_REACTOR));

    // Prepare i18n
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Warp"));

    let app = WarpApplication::new();
    app.run();
}
