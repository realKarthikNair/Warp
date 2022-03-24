#[macro_use]
mod macros;

mod config;
#[allow(dead_code)]
mod globals;
mod ui;
mod util;

use crate::ui::application::WarpApplication;
use gettextrs::gettext;
use gtk::glib;

fn main() {
    // Initialize logger
    pretty_env_logger::init();

    // Prepare i18n
    gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    let localedir = option_env!("LOCALEDIR").unwrap_or(globals::DEFAULT_LOCALEDIR);
    match gettextrs::bindtextdomain(globals::GETTEXT_PACKAGE, localedir) {
        Ok(path) => log::debug!("Bound text domain for path: {}", path.display()),
        Err(err) => log::error!("Error binding text domain: {}", err),
    }

    if let Err(err) = gettextrs::textdomain(globals::GETTEXT_PACKAGE) {
        log::error!("Unable to switch to the text domain: {}", err);
    }

    glib::set_application_name(&gettext("Warp"));

    let app = WarpApplication::new();
    app.run();
}
