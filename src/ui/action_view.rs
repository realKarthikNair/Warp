use crate::glib::clone;
use crate::globals;
use crate::service::wormhole::{Wormhole, WormholeState};
use crate::ui::util;
use crate::ui::window::WarpApplicationWindow;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use pyo3::{PyResult, Python};
use std::path::PathBuf;
use std::time::Duration;

mod imp {
    use super::*;
    use std::cell::RefCell;

    use crate::glib::clone;
    use crate::service::wormhole::Wormhole;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/net/felinira/warp/ui/action_view.ui")]
    pub struct ActionView {
        #[template_child]
        pub cancel_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        pub wormhole: RefCell<Option<Wormhole>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ActionView {
        const NAME: &'static str = "ActionView";
        type Type = super::ActionView;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ActionView {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.cancel_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.back_clicked();
                }));

            self.progress_bar.set_pulse_step(0.05);
        }
    }

    impl WidgetImpl for ActionView {}
    impl BoxImpl for ActionView {}
}

glib::wrapper! {
    pub struct ActionView(ObjectSubclass<imp::ActionView>)
        @extends gtk::Widget, gtk::Box;
}

impl ActionView {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create ActionView")
    }

    pub fn back_clicked(&self) {
        WarpApplicationWindow::default().navigate_back();
    }

    pub fn send_file(&self, path: PathBuf) {
        if let Ok(path_str) = path.into_os_string().into_string() {
            log::debug!("Picked file: {}", path_str);
            let self_ = imp::ActionView::from_instance(self);
            self_.progress_bar.set_visible(true);
            self_.status_page.set_title("Waiting for code");
            self_
                .status_page
                .set_description(Some("Warp code is being requested"));
            WarpApplicationWindow::default()
                .leaflet()
                .navigate(adw::NavigationDirection::Forward);

            util::do_async(clone!(@strong self as obj => async move {
                let obj_ = imp::ActionView::from_instance(&obj);
                let wormhole = Wormhole::new().await?;
                wormhole.allocate_code()?;
                obj_.wormhole.replace(Some(wormhole));

                // 50 ms was mainly chosen for performance of the progress bar
                glib::timeout_add_local(
                    Duration::from_millis(50),
                    clone!(@strong obj => move|| {
                        let obj_ = imp::ActionView::from_instance(&obj);

                        if let Some(wormhole) = &*obj_.wormhole.borrow() {
                            let state = wormhole.poll_state();
                            match state {
                                WormholeState::Initialized => {
                                    obj_.progress_bar.pulse();
                                    Continue(true)
                                }
                                WormholeState::CodePresent => {
                                    obj_.status_page.set_title("Please send the code to the receiver");
                                    obj_.status_page.set_description(Some(&wormhole.get_code().unwrap()));
                                    obj_.progress_bar.set_visible(false);
                                    Continue(false)
                                }
                                _ => Continue(false)
                            }
                        } else {
                            Continue(false)
                        }
                    }),
                );

                Ok(())
            }));
        }
    }
}

impl Default for ActionView {
    fn default() -> Self {
        Self::new()
    }
}
