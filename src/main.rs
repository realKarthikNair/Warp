#[macro_use]
mod macros;

mod application;
#[rustfmt::skip]
mod config;
mod globals;
mod window;
mod wormhole;

use gettextrs::{gettext, LocaleCategory};
use gtk::{gio, glib};

use self::application::WarpApplication;
use self::config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};

fn main() {
    // Initialize logger
    pretty_env_logger::init();

    // Prepare i18n
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Warp"));

    let app = WarpApplication::new();
    app.run();
}
