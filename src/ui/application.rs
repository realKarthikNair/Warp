use gettextrs::gettext;
use log::{debug, info};

use adw::subclass::prelude::*;
use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::globals;
use crate::ui::preferences::WarpPreferencesWindow;
use crate::ui::window::WarpApplicationWindow;
use crate::util::TransferDirection;

mod imp {
    use super::*;
    use crate::util::error::{AppError, UiError};
    use crate::util::extract_transmit_code;
    use crate::util::WormholeTransferURI;
    use gio::File;
    use glib::WeakRef;
    use once_cell::sync::OnceCell;
    use std::cell::Cell;

    #[derive(Debug, Default)]
    pub struct WarpApplication {
        pub window: OnceCell<WeakRef<WarpApplicationWindow>>,
        pub inhibit_cookie: Cell<Option<u32>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WarpApplication {
        const NAME: &'static str = "WarpApplication";
        type Type = super::WarpApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for WarpApplication {}

    impl ApplicationImpl for WarpApplication {
        fn activate(&self, app: &Self::Type) {
            debug!("GtkApplication<WarpApplication>::activate");
            self.parent_activate(app);

            if let Some(window) = self.window.get() {
                let window = window.upgrade().unwrap();
                window.present();
                return;
            }

            log::debug!("Create window");

            let window = WarpApplicationWindow::new(app);
            self.window
                .set(window.downgrade())
                .expect("Window already set.");

            app.main_window().present();
        }

        fn startup(&self, app: &Self::Type) {
            debug!("GtkApplication<WarpApplication>::startup");
            self.parent_startup(app);

            // Set icons for shell
            gtk::Window::set_default_icon_name(globals::APP_ID);

            app.setup_gresources();
            app.setup_css();
            app.setup_gactions();
            app.setup_accels();
        }

        fn open(&self, app: &Self::Type, files: &[File], _hint: &str) {
            self.activate(app);

            if !files.is_empty() {
                if app.main_window().transfer_in_progress() {
                    let err: AppError = UiError::new(&gettext(
                        "Unable to use transfer link: another transfer already in progress",
                    ))
                    .into();
                    err.show_error_dialog(&app.main_window());
                } else if let Some(code) = files[0]
                    .uri()
                    .strip_prefix("warp://recv/")
                    .and_then(extract_transmit_code)
                {
                    app.main_window()
                        .action_view()
                        .receive_file(wormhole::Code(code), app.main_window().config().app_cfg());
                } else {
                    match files[0].uri().parse::<WormholeTransferURI>() {
                        Ok(uri) => {
                            app.main_window().open_code_from_uri(uri);
                        }
                        Err(err) => {
                            let err: AppError = UiError::new(&err.to_string()).into();
                            err.show_error_dialog(&app.main_window());
                        }
                    }
                }
            }
        }
    }

    impl GtkApplicationImpl for WarpApplication {}
    impl AdwApplicationImpl for WarpApplication {}
}

glib::wrapper! {
    pub struct WarpApplication(ObjectSubclass<imp::WarpApplication>)
        @extends gio::Application, gtk::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl WarpApplication {
    pub fn new() -> Self {
        glib::Object::new(&[
            ("application-id", &Some(globals::APP_ID)),
            ("flags", &gio::ApplicationFlags::HANDLES_OPEN),
            ("resource-base-path", &Some("/app/drey/Warp/")),
        ])
        .expect("Application initialization failed...")
    }

    pub fn main_window(&self) -> WarpApplicationWindow {
        self.imp().window.get().unwrap().upgrade().unwrap()
    }

    pub fn setup_gresources(&self) {
        let data = glib::Bytes::from(globals::GRESOURCE_DATA);
        let resource = gio::Resource::from_data(&data).expect("Error loading resource bundle");
        gio::resources_register(&resource);
    }

    fn setup_gactions(&self) {
        // Help
        let action_help = gio::SimpleAction::new("help", None);
        action_help.connect_activate(clone!(@weak self as app => move |_, _| {
            gtk::show_uri(Some(&WarpApplicationWindow::default()), "help:warp", gtk::gdk::CURRENT_TIME);
        }));
        self.add_action(&action_help);

        // Preferences
        let action_preferences = gio::SimpleAction::new("preferences", None);
        action_preferences.connect_activate(clone!(@weak self as app => move |_, _| {
            WarpPreferencesWindow::new().show();
        }));
        self.add_action(&action_preferences);

        // Quit
        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(clone!(@weak self as app => move |_, _| {
            // This is needed to trigger the delete event and saving the window state
            app.main_window().close();
            app.quit();
        }));
        self.add_action(&action_quit);

        // About
        let action_about = gio::SimpleAction::new("about", None);
        action_about.connect_activate(clone!(@weak self as app => move |_, _| {
            app.show_about_dialog();
        }));
        self.add_action(&action_about);
    }

    // Sets up keyboard shortcuts
    fn setup_accels(&self) {
        self.set_accels_for_action("app.help", &["F1"]);
        self.set_accels_for_action("win.show-help-overlay", &["<Control>question"]);
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("win.open-file", &["<Control>o"]);
        self.set_accels_for_action("win.open-folder", &["<Control>d"]);
        self.set_accels_for_action("win.receive-file", &["<Control>r"]);
    }

    fn setup_css(&self) {
        let provider = gtk::CssProvider::new();
        provider.load_from_resource("/app/drey/Warp/style.css");
        if let Some(display) = gdk::Display::default() {
            gtk::StyleContext::add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    fn show_about_dialog(&self) {
        let dialog = gtk::AboutDialog::builder()
            .logo_icon_name(globals::APP_ID)
            .license_type(gtk::License::Gpl30)
            .website("https://gitlab.gnome.org/World/warp/")
            .version(globals::VERSION)
            .transient_for(&self.main_window())
            .translator_credits(&gettext("translator-credits"))
            .modal(true)
            .authors(vec!["Fina Wilke".into()])
            .artists(vec![gettext("Fina Wilke"), gettext("Sophie Herold")])
            .build();

        dialog.present();
    }

    pub fn run(&self) {
        info!("Warp ({})", globals::APP_ID);
        info!("Version: {}", globals::VERSION);
        info!("Datadir: {}", globals::PKGDATADIR);

        ApplicationExtManual::run(self);
    }

    pub fn inhibit_transfer(&self, transfer_direction: TransferDirection) {
        let inhibit_reason = match transfer_direction {
            TransferDirection::Send => gettext("Sending a File"),
            TransferDirection::Receive => gettext("Receiving a File"),
        };

        let inhibit = WarpApplication::default().inhibit(
            Some(&WarpApplicationWindow::default()),
            gtk::ApplicationInhibitFlags::LOGOUT | gtk::ApplicationInhibitFlags::SUSPEND,
            Some(&inhibit_reason),
        );

        self.imp().inhibit_cookie.set(Some(inhibit));
    }

    pub fn uninhibit_transfer(&self) {
        if let Some(cookie) = self.imp().inhibit_cookie.take() {
            self.uninhibit(cookie);
        }
    }

    pub fn send_notification_if_background(
        &self,
        id: Option<&str>,
        notification: &gio::Notification,
    ) {
        if let Some(window) = self.active_window() {
            if window.is_active() {
                return;
            }
        }

        self.send_notification(id, notification);
    }

    pub fn is_flatpak() -> bool {
        if let Ok(var) = std::env::var("FLATPAK_ID") {
            var == globals::APP_ID
        } else {
            false
        }
    }
}

impl Default for WarpApplication {
    fn default() -> Self {
        gio::Application::default()
            .expect("Application not initialized")
            .downcast::<WarpApplication>()
            .expect("Application is wrong subclass")
    }
}
