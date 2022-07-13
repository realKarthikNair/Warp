use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;

use crate::ui::window::WarpApplicationWindow;

const CODE_LENGTH_MIN: i32 = 2;
const CODE_LENGTH_MAX: i32 = 8;

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};

    use crate::glib::signal::Inhibit;
    use crate::glib::Value;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/Warp/ui/preferences.ui")]
    pub struct WarpPreferencesWindow {
        #[template_child]
        pub rendezvous_server_url_entry_row: TemplateChild<adw::EntryRow>,

        #[template_child]
        pub transit_server_url_entry_row: TemplateChild<adw::EntryRow>,

        #[template_child]
        pub manual_code_switch: TemplateChild<gtk::Switch>,

        #[template_child]
        pub code_length_spin_button: TemplateChild<gtk::SpinButton>,

        pub rendezvous_server_url: RefCell<String>,
        pub transit_server_url: RefCell<String>,
        pub manual_code_entry: Cell<bool>,
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
                    glib::ParamSpecBoolean::new(
                        "manual-code-entry",
                        "",
                        "",
                        false,
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

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "rendezvous-server-url" => obj.set_rendezvous_server_url(value.get().unwrap()),
                "transit-server-url" => obj.set_transit_server_url(value.get().unwrap()),
                "manual-code-entry" => obj.set_manual_code_entry(value.get().unwrap()),
                "code-length" => obj.set_code_length(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> Value {
            match pspec.name() {
                "rendezvous-server-url" => obj.rendezvous_server_url().to_value(),
                "transit-server-url" => obj.transit_server_url().to_value(),
                "manual-code-entry" => obj.manual_code_entry().to_value(),
                "code-length" => obj.code_length().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let window = WarpApplicationWindow::default();

            obj.set_transient_for(Some(&window));
            obj.set_rendezvous_server_url(
                window
                    .config()
                    .rendezvous_server_url
                    .clone()
                    .unwrap_or_else(|| "".to_owned()),
            );
            obj.set_transit_server_url(
                window
                    .config()
                    .transit_server_url
                    .clone()
                    .unwrap_or_else(|| "".to_owned()),
            );

            obj.set_manual_code_entry(window.config().manual_code_entry);

            self.code_length_spin_button
                .set_adjustment(&gtk::Adjustment::new(4f64, 2f64, 8f64, 1f64, 0f64, 0f64));

            obj.set_code_length(window.config().code_length_or_default() as i32);
        }
    }

    impl WidgetImpl for WarpPreferencesWindow {}
    impl WindowImpl for WarpPreferencesWindow {
        fn close_request(&self, _window: &Self::Type) -> Inhibit {
            let window = WarpApplicationWindow::default();

            let rendezvous_url = &*self.rendezvous_server_url.borrow();
            window.config().rendezvous_server_url =
                (!rendezvous_url.is_empty()).then(|| rendezvous_url.clone());

            let transit_url = &*self.transit_server_url.borrow();
            window.config().transit_server_url =
                (!transit_url.is_empty()).then(|| transit_url.clone());

            window.config().manual_code_entry = self.manual_code_entry.get();

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
        glib::Object::new(&[]).expect("Failed to create WarpPreferencesWindow")
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
            self.imp().rendezvous_server_url.replace("".to_owned());
        }

        self.notify("rendezvous-server-url");
    }

    pub fn rendezvous_server_url(&self) -> String {
        self.imp().rendezvous_server_url.borrow().to_string()
    }

    pub fn set_transit_server_url(&self, url: String) {
        let is_valid_url = url::Url::parse(&url).is_ok();

        if is_valid_url && !url.is_empty() {
            self.imp()
                .transit_server_url_entry_row
                .add_css_class("success");
        } else {
            self.imp()
                .transit_server_url_entry_row
                .remove_css_class("success");
        }

        if url.is_empty() || is_valid_url {
            self.imp()
                .transit_server_url_entry_row
                .remove_css_class("error");
            self.imp().transit_server_url.replace(url);
        } else {
            self.imp()
                .transit_server_url_entry_row
                .add_css_class("error");
            self.imp().transit_server_url.replace("".to_owned());
        }

        self.notify("transit-server-url");
    }

    pub fn transit_server_url(&self) -> String {
        self.imp().transit_server_url.borrow().to_string()
    }

    pub fn set_manual_code_entry(&self, manual_code_entry: bool) {
        self.imp().manual_code_entry.set(manual_code_entry);
        self.notify("manual-code-entry");
    }

    pub fn manual_code_entry(&self) -> bool {
        self.imp().manual_code_entry.get()
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
