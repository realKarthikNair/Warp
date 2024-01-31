use crate::globals;
use crate::ui::preferences::WarpPreferencesWindow;
use crate::ui::window::WarpApplicationWindow;
use crate::util::future::main_async_local;
use crate::util::{show_dir, TransferDirection};
use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use glib::clone;
use std::path::PathBuf;

mod imp {
    use super::*;
    use crate::util::error::{AppError, UiError};
    use crate::util::WormholeTransferURI;
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
        fn activate(&self) {
            log::debug!("GtkApplication<WarpApplication>::activate");
            self.parent_activate();

            if let Some(window) = self.window.get() {
                let window = window.upgrade().unwrap();
                window.present();
                return;
            }

            log::debug!("Create window");

            let window = WarpApplicationWindow::new(&self.obj());
            self.window
                .set(window.downgrade())
                .expect("Window already set.");

            self.obj().main_window().present();
        }

        fn open(&self, files: &[gio::File], _hint: &str) {
            self.activate();
            let app = self.obj();

            if !files.is_empty() {
                if app.main_window().transfer_in_progress() {
                    let err: AppError = UiError::new(&gettext(
                        "Unable to use transfer link: another transfer already in progress",
                    ))
                    .into();
                    err.show_error_dialog(&app.main_window());
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

        fn startup(&self) {
            log::debug!("GtkApplication<WarpApplication>::startup");
            let app = self.obj();
            self.parent_startup();

            // Set icons for shell
            gtk::Window::set_default_icon_name(globals::APP_ID);

            app.cleanup_cache();
            app.setup_gactions();
            app.setup_accels();
        }
    }

    impl GtkApplicationImpl for WarpApplication {}
    impl AdwApplicationImpl for WarpApplication {}
}

glib::wrapper! {
    pub struct WarpApplication(ObjectSubclass<imp::WarpApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl WarpApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", Some(globals::APP_ID))
            .property("flags", gio::ApplicationFlags::HANDLES_OPEN)
            .property("resource-base-path", Some("/app/drey/Warp/"))
            .build()
    }

    pub fn main_window(&self) -> WarpApplicationWindow {
        self.imp().window.get().unwrap().upgrade().unwrap()
    }

    pub fn cleanup_cache(&self) {
        log::debug!("Cleaning up cache dir: {}", globals::CACHE_DIR.display());
        let res = std::fs::remove_dir_all(&*globals::CACHE_DIR);
        if let Err(err) = res {
            if err.kind() != std::io::ErrorKind::NotFound {
                log::error!(
                    "Error cleaning cache dir '{}': {}",
                    globals::CACHE_DIR.display(),
                    err
                );
            }
        }
    }

    fn setup_gactions(&self) {
        // Help
        let action_help = gio::SimpleAction::new("help", None);
        action_help.connect_activate(clone!(@weak self as app => move |_, _| {
            /* `help:` URIs are a Linux specific thing and won't work on Windows. There, we'll just open the path to the
             * respective HTML files and hope that it launches a browser …
             */
            let help_uri = if cfg!(not(windows)) {
                "help:warp".into()
            } else {
                let mut uri = globals::WINDOWS_BASE_PATH.clone();
                /* Hardcode the "C" language for now, so no translated help files *sigh*
                 *
                 * The problem is that gettext is a mess and does not provide us with a good way
                 * to query the currenty used language. In theory it can do that, but the values
                 * it returned on Windows did not work as they should.
                 */
                uri.push("share\\help\\C\\warp\\index.html");
                /* People with non-UTF-8 paths will at least get a good error message */
                let mut uri = uri.to_string_lossy().into_owned();
                uri.insert_str(0, "file:///");
                uri
            };

            log::debug!("Opening '{}' to show help", help_uri);
            let context = app
                .active_window()
                .map(|w| gtk::prelude::WidgetExt::display(&w).app_launch_context());

            glib::MainContext::default().spawn_local(async move {
                if let Err(err) = gio::AppInfo::launch_default_for_uri_future(&help_uri, context.as_ref()).await
                {
                    log::error!("Error launching help: {err:?}");
                }
            });
        }));
        self.add_action(&action_help);

        // Preferences
        let action_preferences = gio::SimpleAction::new("preferences", None);
        action_preferences.connect_activate(clone!(@weak self as app => move |_, _| {
            WarpPreferencesWindow::new().present();
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

        // Show received file in file browser (called from notification)
        let action_show_file =
            gio::SimpleAction::new("show-file", Some(&PathBuf::static_variant_type()));
        action_show_file.connect_activate(clone!(@weak self as app => move |_action, data| {
            if let Some(data) = data {
                let path = PathBuf::from_variant(data);
                if let Some(filename) = path {
                    main_async_local(crate::util::error::AppError::handle, async move {show_dir(&filename).await});
                }
            }
        }));

        self.add_action(&action_show_file);
    }

    // Sets up keyboard shortcuts
    fn setup_accels(&self) {
        self.set_accels_for_action("app.help", &["F1"]);
        self.set_accels_for_action("win.show-help-overlay", &["<Control>question"]);
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("win.open-file", &["<Control>o"]);
        self.set_accels_for_action("win.open-folder", &["<Control>d"]);
        self.set_accels_for_action("win.receive-file", &["<Control>r"]);
        self.set_accels_for_action("app.preferences", &["<Control>comma"]);
    }

    fn show_about_dialog(&self) {
        let dialog =
            adw::AboutDialog::from_appdata("app/drey/Warp/metainfo.xml", Some(globals::VERSION));

        dialog.set_developers(&[&gettext("Fina Wilke")]);
        dialog.set_artists(&[&gettext("Tobias Bernard"), &gettext("Sophie Herold")]);
        dialog.set_translator_credits(&gettext("translator-credits"));

        dialog.present(&self.main_window());
    }

    pub fn run(&self) {
        log::info!("Warp ({})", globals::APP_ID);
        log::info!("Version: {}", globals::VERSION);
        log::info!("Datadir: {}", globals::PKGDATADIR);

        ApplicationExtManual::run(self);
    }

    pub fn inhibit_transfer(&self, transfer_direction: TransferDirection) {
        let inhibit_reason = match transfer_direction {
            TransferDirection::Send => gettext("Sending a File"),
            TransferDirection::Receive => gettext("Receiving a File"),
        };

        let inhibit = self.inhibit(
            Some(&self.main_window()),
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
        assert!(
            gtk::is_initialized_main_thread(),
            "Calling gio::Application::default from non-main thread"
        );

        gio::Application::default()
            .expect("Application not initialized")
            .downcast::<WarpApplication>()
            .expect("Application is wrong subclass")
    }
}
