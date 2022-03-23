#[macro_use]
mod macros;

mod config;
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
    let mut text_domain = gettextrs::TextDomain::new(globals::GETTEXT_PACKAGE);
    if let Some(localedir) = option_env!("LOCALEDIR") {
        text_domain = text_domain.prepend(localedir);
    }

    if let Err(err) = text_domain
        .locale_category(gettextrs::LocaleCategory::LcAll)
        .init()
    {
        log::info!("{}", err);
    }

    glib::set_application_name(&gettext("Warp"));

    let app = WarpApplication::new();
    app.run();
}
