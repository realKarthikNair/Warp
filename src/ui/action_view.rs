use crate::glib::clone;
use crate::globals;
use crate::service::wormhole::{Wormhole, WormholeState};
use crate::ui::window::WarpApplicationWindow;
use crate::util;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use pyo3::{PyResult, Python};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};

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
        #[template_child]
        pub code_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub code_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub code_copy_button: TemplateChild<gtk::Button>,
        pub wormhole: RefCell<Option<Rc<Wormhole>>>,
        pub progress_timeout_source_id: RefCell<Option<glib::source::SourceId>>,
        pub cancel: Cell<bool>,
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

            self.code_copy_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    let obj_ = imp::ActionView::from_instance(&obj);
                    let code = obj_.code_entry.text();
                    let window = WarpApplicationWindow::default();
                    let clipboard = window.display().clipboard();
                    clipboard.set_text(&code);
                    let toast = adw::Toast::new("Copied code to clipboard");
                    toast.set_timeout(3);
                    toast.set_priority(adw::ToastPriority::Normal);
                    window.toast_overlay().add_toast(&toast);
                }));
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
        let self_ = imp::ActionView::from_instance(self);
        self_.wormhole.borrow().iter().for_each(|w| {
            let res = w.close();
            if let Err(err) = res {
                log::error!("{}", err);
            }

            ()
        });
        self_.wormhole.replace(None);
        self_.cancel.set(true);
        self.show_progress_indeterminate(false);
        WarpApplicationWindow::default().navigate_back();
    }

    pub fn show_progress_indeterminate(&self, pulse: bool) {
        let self_ = imp::ActionView::from_instance(self);
        if let Some(source_id) = self_.progress_timeout_source_id.take() {
            source_id.remove();
        }

        if pulse {
            // 50 ms was mainly chosen for performance of the progress bar
            self_
                .progress_timeout_source_id
                .replace(Some(glib::timeout_add_local(
                    Duration::from_millis(50),
                    clone!(@strong self as obj => move || {
                        let obj_ = imp::ActionView::from_instance(&obj);
                        obj_.progress_bar.pulse();

                        Continue(true)
                    }),
                )));
        }
    }

    pub fn send_file(&self, path: PathBuf) {
        if let Ok(path_str) = path.into_os_string().into_string() {
            log::debug!("Picked file: {}", path_str);
            let self_ = imp::ActionView::from_instance(self);
            self_.cancel.set(false);
            self_.code_box.set_visible(false);
            self_.progress_bar.set_visible(true);
            self_.status_page.set_title("Waiting for code");
            self_
                .status_page
                .set_description(Some("Warp code is being requested"));
            WarpApplicationWindow::default()
                .leaflet()
                .navigate(adw::NavigationDirection::Forward);
            self.show_progress_indeterminate(true);

            util::do_async_local(clone!(@strong self as obj => async move {
                let obj_ = imp::ActionView::from_instance(&obj);
                let wormhole = Rc::new(Wormhole::new().await?);
                obj_.wormhole.replace(Some(wormhole.clone()));
                wormhole.allocate_code()?;
                if obj_.cancel.get() {
                    wormhole.close();
                    return Ok(())
                }

                loop {
                    let state = wormhole.async_state().await;
                    match state {
                        WormholeState::Initialized => continue,
                        WormholeState::CodePresent => {
                            obj_.status_page.set_title("Please send the code to the receiver");
                            obj_.status_page.set_description(None);
                            obj_.code_box.set_visible(true);
                            obj_.code_entry.set_text(&wormhole.get_code().unwrap());
                            obj_.progress_bar.set_visible(false);
                        },
                        WormholeState::Connected => {
                            obj_.status_page.set_title("Connected to peer");
                            obj_.status_page.set_description(Some("Preparing to send file"));
                            obj_.code_box.set_visible(false);
                            obj_.progress_bar.set_visible(true);
                        }
                        _ => break,
                    }
                }

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
