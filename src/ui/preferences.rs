use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;

use crate::ui::window::WarpApplicationWindow;

mod imp {
    use super::*;
    use std::cell::RefCell;

    use crate::glib::signal::Inhibit;
    use crate::glib::Value;
    use crate::globals;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/Warp/ui/preferences.ui")]
    pub struct WarpPreferencesWindow {
        #[template_child]
        pub rendezvous_server_url_entry: TemplateChild<gtk::Entry>,

        #[template_child]
        pub transit_server_url_entry: TemplateChild<gtk::Entry>,

        pub rendezvous_server_url: RefCell<String>,
        pub transit_server_url: RefCell<String>,
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
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> Value {
            match pspec.name() {
                "rendezvous-server-url" => obj.rendezvous_server_url().to_value(),
                "transit-server-url" => obj.transit_server_url().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let window = WarpApplicationWindow::default();
            obj.set_rendezvous_server_url(
                window
                    .config()
                    .rendezvous_server_url
                    .clone()
                    .unwrap_or_else(|| "".to_string()),
            );
            obj.set_transit_server_url(
                window
                    .config()
                    .transit_server_url
                    .clone()
                    .unwrap_or_else(|| "".to_string()),
            );

            self.rendezvous_server_url_entry
                .set_placeholder_text(Some(globals::WORMHOLE_RENDEZVOUS_RELAY_DEFAULT));
            self.transit_server_url_entry
                .set_placeholder_text(Some(globals::WORMHOLE_TRANSIT_RELAY_DEFAULT));
        }
    }

    impl WidgetImpl for WarpPreferencesWindow {}
    impl WindowImpl for WarpPreferencesWindow {
        fn close_request(&self, _window: &Self::Type) -> Inhibit {
            let rendezvous_url = &*self.rendezvous_server_url.borrow();
            WarpApplicationWindow::default()
                .config()
                .rendezvous_server_url = if rendezvous_url != "" {
                Some(rendezvous_url.clone())
            } else {
                None
            };

            let transit_url = &*self.transit_server_url.borrow();
            WarpApplicationWindow::default().config().transit_server_url = if transit_url != "" {
                Some(transit_url.clone())
            } else {
                None
            };

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
        self.imp().rendezvous_server_url.replace(url.clone());
        self.notify("rendezvous-server-url");
    }

    pub fn rendezvous_server_url(&self) -> String {
        self.imp().rendezvous_server_url.borrow().to_string()
    }

    pub fn set_transit_server_url(&self, url: String) {
        self.imp().transit_server_url.replace(url.clone());
        self.notify("transit-server-url");
    }

    pub fn transit_server_url(&self) -> String {
        self.imp().transit_server_url.borrow().to_string()
    }
}

impl Default for WarpPreferencesWindow {
    fn default() -> Self {
        Self::new()
    }
}
