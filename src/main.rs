#![warn(clippy::redundant_else)]

#[macro_use]
mod macros;
mod config;
#[allow(dead_code)]
mod gettext;
#[allow(dead_code)]
mod globals;
mod ui;
mod util;

use crate::ui::application::WarpApplication;
use crate::util::error;
use gettextrs::gettext;
use gtk::glib;

fn setup_gresources() {
    #[cfg(all(debug_assertions, not(feature = "meson")))]
    let data = {
        log::info!("Loading GResource data from directory 'data/resources'");
        gvdb::gresource::GResourceBuilder::from_directory(
            "/app/drey/Warp",
            &std::path::PathBuf::from("data/resources"),
            true,
            true,
        )
        .map(|builder| glib::Bytes::from(&builder.build().unwrap()))
        .unwrap_or_else(|_| {
            log::warn!(
                "Loading fallback precompiled GResource data because the directory does not exist"
            );
            glib::Bytes::from(globals::GRESOURCE_DATA)
        })
    };
    #[cfg(any(not(debug_assertions), feature = "meson"))]
    let data = {
        log::debug!("Loading precompiled GResource data");
        glib::Bytes::from(globals::GRESOURCE_DATA)
    };

    let resource = gio::Resource::from_data(&data).expect("Error loading resource bundle");
    gio::resources_register(&resource);
}

fn main() {
    // Initialize logger
    pretty_env_logger::init();

    error::install_panic_hook();

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

    setup_gresources();

    let app = WarpApplication::new();
    app.run();
}
