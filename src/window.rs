use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::application::WarpApplication;
use crate::config::{APP_ID, PROFILE};

mod imp {
    use super::*;
    use std::sync::Arc;

    use crate::wormhole::{TwistedReactor, Wormhole};
    use gtk::CompositeTemplate;
    use pyo3::PyResult;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/net/felinira/warp/ui/window.ui")]
    pub struct WarpApplicationWindow {
        #[template_child]
        pub headerbar: TemplateChild<gtk::HeaderBar>,
    }

    impl Default for WarpApplicationWindow {
        fn default() -> Self {
            Self {
                headerbar: TemplateChild::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WarpApplicationWindow {
        const NAME: &'static str = "WarpApplicationWindow";
        type Type = super::WarpApplicationWindow;
        type ParentType = gtk::ApplicationWindow;

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

            log::debug!("Starting wormhole");
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
            }
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
}
