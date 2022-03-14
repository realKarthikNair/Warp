use crate::ui::action_view::ActionView;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, ResponseType};

use crate::config::PROFILE;
use crate::glib::clone;
use crate::ui::application::WarpApplication;

mod imp {
    use super::*;
    use adw::subclass::prelude::AdwApplicationWindowImpl;

    use crate::glib::clone;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/net/felinira/warp/ui/window.ui")]
    pub struct WarpApplicationWindow {
        #[template_child]
        pub headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub leaflet: TemplateChild<adw::Leaflet>,
        #[template_child]
        pub send_select_file_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub receive_button: TemplateChild<gtk::Button>,
        pub action_view: ActionView,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WarpApplicationWindow {
        const NAME: &'static str = "WarpApplicationWindow";
        type Type = super::WarpApplicationWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for WarpApplicationWindow {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // Devel Profile
            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            self.send_select_file_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.send_select_file_button();
                }));

            self.leaflet.append(&self.action_view);

            /*log::debug!("Starting wormhole");
            let reactor = TwistedReactor::new();
            if let Ok(reactor) = reactor {
                let wormhole = Wormhole::new(Arc::new(reactor));
                match wormhole {
                    Ok(wormhole) => {
                        wormhole.allocate_code();
                        log::info!("Get Code: {}", wormhole.get_code());
                        wormhole.wait_open();
                        let res = wormhole.send_text_message("Test Message");
                        if let Err(err) = res {
                            log::error!("Wormhole send message error: {}", err);
                        }

                        wormhole.close();
                    }
                    Err(err) => {
                        log::error!("Wormhole error: {}", err)
                    }
                }
            }*/
        }
    }

    impl WidgetImpl for WarpApplicationWindow {}
    impl WindowImpl for WarpApplicationWindow {
        // Save window state on delete event
        fn close_request(&self, window: &Self::Type) -> gtk::Inhibit {
            // Pass close request on to the parent
            self.parent_close_request(window)
        }
    }

    impl ApplicationWindowImpl for WarpApplicationWindow {}
    impl AdwApplicationWindowImpl for WarpApplicationWindow {}
}

glib::wrapper! {
    pub struct WarpApplicationWindow(ObjectSubclass<imp::WarpApplicationWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Root;
}

impl WarpApplicationWindow {
    pub fn new(app: &WarpApplication) -> Self {
        glib::Object::new(&[("application", app)]).expect("Failed to create WarpApplicationWindow")
    }

    pub fn send_select_file_button(&self) {
        let self_ = imp::WarpApplicationWindow::from_instance(self);
        let chooser = gtk::FileChooserDialog::new(
            Some("Select files / folders to send"),
            Some(self),
            gtk::FileChooserAction::Open,
            &[
                ("Cancel", gtk::ResponseType::Cancel),
                ("Open", gtk::ResponseType::Ok),
            ],
        );
        chooser.set_modal(true);
        chooser.connect_response(
            clone!(@strong self as instance => move |chooser, response| {
                let self_ = imp::WarpApplicationWindow::from_instance(&instance);
                match response {
                    ResponseType::Ok => {
                        if let Some(file) = chooser.file() {
                            if let Some(path) = file.path() {
                                if let Ok(path_str) = path.into_os_string().into_string() {
                                    log::debug!("Picked file: {}", path_str);
                                    self_.leaflet.navigate(adw::NavigationDirection::Forward);
                                }
                            }
                        }
                    }
                    ResponseType::Cancel => {
                        log::debug!("File Chooser Canceled");
                    }
                    _ => {}
                };

                chooser.close();
            }),
        );
        chooser.show();
    }

    pub fn navigate_back(&self) {
        let leaflet = WarpApplicationWindow::default().leaflet();
        leaflet.navigate(adw::NavigationDirection::Back);
    }

    pub fn leaflet(&self) -> adw::Leaflet {
        imp::WarpApplicationWindow::from_instance(self)
            .leaflet
            .clone()
    }
}

impl Default for WarpApplicationWindow {
    fn default() -> Self {
        WarpApplication::default().main_window()
    }
}
