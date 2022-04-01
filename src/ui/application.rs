use gettextrs::gettext;
use log::{debug, info};

use adw::subclass::prelude::*;
use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::globals;
use crate::ui::action_view::TransferDirection;
use crate::ui::window::WarpApplicationWindow;

mod imp {
    use super::*;
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
            ("flags", &gio::ApplicationFlags::empty()),
            ("resource-base-path", &Some("/net/felinira/warp/")),
        ])
        .expect("Application initialization failed...")
    }

    pub fn main_window(&self) -> WarpApplicationWindow {
        self.imp().window.get().unwrap().upgrade().unwrap()
    }

    pub fn setup_gresources(&self) {
        let res_bytes = include_cargo_output_path_bytes!("resources.gresource");
        let data = glib::Bytes::from(&res_bytes[..]);
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
        provider.load_from_resource("/net/felinira/warp/style.css");
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
            .website("https://gitlab.gnome.org/felinira/warp/")
            .version(globals::VERSION)
            .transient_for(&self.main_window())
            .translator_credits(&gettext("translator-credits"))
            .modal(true)
            .authors(vec!["Fina Wilke".into()])
            .artists(vec![
                "Fina Wilke".into(),
                gettext("App Icon by <a href=\"https://svgrepo.com\">svgrepo.com</a>"),
                gettext(
                    "Symbolic icon made from <a href=\"http://www.onlinewebfonts.com/icon\">Icon Fonts</a> is licensed by CC BY 3.0",
                ),
            ])
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
}

impl Default for WarpApplication {
    fn default() -> Self {
        gio::Application::default()
            .expect("Application not initialized")
            .downcast::<WarpApplication>()
            .expect("Application is wrong subclass")
    }
}
