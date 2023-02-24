use adw::prelude::*;
use adw::subclass::prelude::*;

use super::application::WarpApplication;

const CODE_LENGTH_MIN: i32 = 2;
const CODE_LENGTH_MAX: i32 = 8;

mod imp {
    use super::*;
    use crate::gettext::gettextf;
    use crate::globals;
    use glib::signal::Inhibit;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(file = "preferences.ui")]
    pub struct WarpPreferencesWindow {
        #[template_child]
        pub preferences_group: TemplateChild<adw::PreferencesGroup>,

        #[template_child]
        pub rendezvous_server_url_entry_row: TemplateChild<adw::EntryRow>,

        #[template_child]
        pub transit_server_url_entry_row: TemplateChild<adw::EntryRow>,

        #[template_child]
        pub code_length_spin_button: TemplateChild<gtk::SpinButton>,

        pub rendezvous_server_url: RefCell<String>,
        pub transit_server_url: RefCell<String>,
        pub code_length: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WarpPreferencesWindow {
        const NAME: &'static str = "WarpPreferencesWindow";
        type Type = super::WarpPreferencesWindow;
        type ParentType = adw::PreferencesWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for WarpPreferencesWindow {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::new(
                        "rendezvous-server-url",
                        "",
                        "",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
                        "transit-server-url",
                        "",
                        "",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecInt::new(
                        "code-length",
                        "",
                        "",
                        CODE_LENGTH_MIN,
                        CODE_LENGTH_MAX,
                        4,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();
            match pspec.name() {
                "rendezvous-server-url" => obj.set_rendezvous_server_url(value.get().unwrap()),
                "transit-server-url" => obj.set_transit_server_url(value.get().unwrap()),
                "code-length" => obj.set_code_length(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();
            match pspec.name() {
                "rendezvous-server-url" => obj.rendezvous_server_url().to_value(),
                "transit-server-url" => obj.transit_server_url().to_value(),
                "code-length" => obj.code_length().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            let window = self.obj().app().main_window();
            obj.set_transient_for(Some(&window));
            obj.set_rendezvous_server_url(
                window
                    .config()
                    .rendezvous_server_url
                    .clone()
                    .unwrap_or_default(),
            );
            obj.set_transit_server_url(
                window
                    .config()
                    .transit_server_url
                    .clone()
                    .unwrap_or_default(),
            );

            self.code_length_spin_button
                .set_adjustment(&gtk::Adjustment::new(4f64, 2f64, 8f64, 1f64, 0f64, 0f64));

            obj.set_code_length(window.config().code_length_or_default() as i32);

            self.preferences_group.set_description(Some(&gettextf(
                "Changing the rendezvous server URL needs to be done on both sides of the \
transfer. Only enter a server URL you can trust.\n\
\n\
Leaving these entries empty will use the app defaults:\n\
Rendezvous Server: “{0}”\n\
Transit Server: “{1}”",
                &[
                    &globals::WORMHOLE_DEFAULT_RENDEZVOUS_SERVER_STR,
                    &globals::WORMHOLE_DEFAULT_TRANSIT_RELAY_URL_STR,
                ],
            )));
        }
    }

    impl WidgetImpl for WarpPreferencesWindow {}
    impl WindowImpl for WarpPreferencesWindow {
        fn close_request(&self) -> Inhibit {
            let window = self.obj().app().main_window();

            let rendezvous_url = &*self.rendezvous_server_url.borrow();
            window.config().rendezvous_server_url =
                (!rendezvous_url.is_empty()).then(|| rendezvous_url.clone());

            let transit_url = &*self.transit_server_url.borrow();
            window.config().transit_server_url =
                (!transit_url.is_empty()).then(|| transit_url.clone());

            let code_length = self.code_length.get();
            window.config().code_length = Some(code_length as usize);

            window.save_config();

            Inhibit(false)
        }
    }

    impl AdwWindowImpl for WarpPreferencesWindow {}
    impl PreferencesWindowImpl for WarpPreferencesWindow {}
}

glib::wrapper! {
    pub struct WarpPreferencesWindow(ObjectSubclass<imp::WarpPreferencesWindow>)
        @extends gtk::Widget, gtk::Window, adw::Window, adw::PreferencesWindow;
}

impl WarpPreferencesWindow {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn app(&self) -> WarpApplication {
        WarpApplication::default()
    }

    pub fn set_rendezvous_server_url(&self, url: String) {
        let is_valid_url = url::Url::parse(&url).is_ok();

        if is_valid_url && !url.is_empty() {
            self.imp()
                .rendezvous_server_url_entry_row
                .add_css_class("success");
        } else {
            self.imp()
                .rendezvous_server_url_entry_row
                .remove_css_class("success");
        }

        if url.is_empty() || is_valid_url {
            self.imp()
                .rendezvous_server_url_entry_row
                .remove_css_class("error");
            self.imp().rendezvous_server_url.replace(url);
        } else {
            self.imp()
                .rendezvous_server_url_entry_row
                .add_css_class("error");
            self.imp().rendezvous_server_url.replace(String::new());
        }

        self.notify("rendezvous-server-url");
    }

    pub fn rendezvous_server_url(&self) -> String {
        self.imp().rendezvous_server_url.borrow().to_string()
    }

    pub fn set_transit_server_url(&self, url: String) {
        let url_res = url::Url::parse(&url);
        let is_valid_hint = if let Ok(url) = url_res {
            wormhole::transit::RelayHint::from_urls(None, [url]).is_ok()
        } else {
            false
        };

        if is_valid_hint && !url.is_empty() {
            self.imp()
                .transit_server_url_entry_row
                .add_css_class("success");
        } else {
            self.imp()
                .transit_server_url_entry_row
                .remove_css_class("success");
        }

        if url.is_empty() || is_valid_hint {
            self.imp()
                .transit_server_url_entry_row
                .remove_css_class("error");
            self.imp().transit_server_url.replace(url);
        } else {
            self.imp()
                .transit_server_url_entry_row
                .add_css_class("error");
            self.imp().transit_server_url.replace(String::new());
        }

        self.notify("transit-server-url");
    }

    pub fn transit_server_url(&self) -> String {
        self.imp().transit_server_url.borrow().to_string()
    }

    pub fn set_code_length(&self, length: i32) {
        self.imp().code_length.set(length);
        self.notify("code-length");
    }

    pub fn code_length(&self) -> i32 {
        self.imp().code_length.get()
    }
}

impl Default for WarpPreferencesWindow {
    fn default() -> Self {
        Self::new()
    }
}
