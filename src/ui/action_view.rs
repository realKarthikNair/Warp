use crate::glib::clone;
use crate::globals;
use crate::ui::window::WarpApplicationWindow;
use crate::util;
use crate::util::{do_async_local, AppError, UIError};
use futures::StreamExt;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use wormhole::{transfer, transit, Code, Wormhole};

mod imp {
    use super::*;
    use futures::channel::mpsc;
    use std::cell::{Cell, RefCell};

    use crate::glib::clone;
    use gtk::CompositeTemplate;
    use once_cell::sync::OnceCell;

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
        pub progress_timeout_source_id: RefCell<Option<glib::source::SourceId>>,
        pub cancel: Cell<bool>,
        pub cancel_sender: OnceCell<async_channel::Sender<bool>>,
        pub cancel_receiver: OnceCell<async_channel::Receiver<bool>>,
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

            let (cancel_sender, cancel_receiver) = async_channel::unbounded();
            self.cancel_sender.get_or_init(|| cancel_sender);
            self.cancel_receiver.get_or_init(|| cancel_receiver);
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

        self_.cancel.set(true);

        do_async_local(clone!(@strong self as obj => async move {
            imp::ActionView::from_instance(&obj).cancel_sender.get().unwrap().send(true).await.unwrap();
            Ok(())
        }));

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

    pub fn transmit(&self, path: Option<PathBuf>, code: Option<Code>, send: bool) {
        let self_ = imp::ActionView::from_instance(self);

        if send {
            let path = path.as_ref().unwrap();
            if let Ok(path_str) = path.clone().into_os_string().into_string() {
                log::debug!("Picked file: {}", path_str);
                self_.status_page.set_title("Waiting for code");
                self_
                    .status_page
                    .set_description(Some("Code is being requested"));
            } else {
                log::error!("Path not convertible to string");
                return;
            }
        } else {
            log::debug!("Receiving file");
            let code = code.as_ref().unwrap().clone();
            self_.status_page.set_title("Waiting for connection");
            self_
                .status_page
                .set_description(Some(&format!("Connecting to peer with code {}", code)));
        }

        self_.code_box.set_visible(false);
        self_.cancel.set(false);
        self_.progress_bar.set_visible(true);
        self_.progress_bar.set_show_text(false);
        self.show_progress_indeterminate(true);
        WarpApplicationWindow::default()
            .leaflet()
            .navigate(adw::NavigationDirection::Forward);

        util::do_async_local(
            clone!(@strong self as obj => @default-return Ok(()), async move {
                let obj_ = imp::ActionView::from_instance(&obj);

                let wormhole;

                if send {
                    let res = Wormhole::connect_without_code(globals::WORMHOLE_APPCFG.clone(), 4).await;
                    let (welcome, connection)= match res {
                        Ok(tuple) => tuple,
                        Err(err) => {
                            return Err(err)?;
                        }
                    };
                    obj_.status_page.set_title("Please send the code to the receiver");
                    obj_.status_page.set_description(None);
                    obj_.code_box.set_visible(true);
                    obj_.code_entry.set_text(&welcome.code);
                    obj_.progress_bar.set_visible(false);

                    wormhole = connection.await?;
                    obj_.status_page.set_title("Connected to peer");
                    obj_.status_page.set_description(Some("Preparing to send file"));
                    obj_.code_box.set_visible(false);
                    obj_.progress_bar.set_visible(true);
                } else {
                    let code = code.unwrap();
                    let (welcome, connection) = Wormhole::connect_with_code(globals::WORMHOLE_APPCFG.clone(), code).await?;
                    wormhole = connection;
                    obj_.status_page.set_title("Connected to peer");
                    obj_.status_page.set_description(Some("Preparing to receive file"));
                }

                // Handle delayed cancel that happens before wormhole creation
                if obj_.cancel.get() {
                    wormhole.close().await?;
                    return Ok(());
                }


                let transit_abilities = transit::Abilities::ALL_ABILITIES;
                let progress_handler = |sent: u64, total: u64| {
                    glib::MainContext::default().invoke(move ||{
                        let obj = WarpApplicationWindow::default().action_view();
                        let obj_ = imp::ActionView::from_instance(&obj);
                        if sent == 0 {
                            obj.show_progress_indeterminate(false);
                            obj_.progress_bar.set_show_text(true);
                        }

                        obj_.progress_bar.set_fraction(sent as f64 / total as f64);
                        obj_.progress_bar.set_text(Some(&format!("{} / {}", pretty_bytes::converter::convert(sent as f64), pretty_bytes::converter::convert(total as f64))));
                    });
                };

                let transit_url = url::Url::parse(globals::WORMHOLE_TRANSIT_RELAY)?;
                let cancel_receiver = obj_.cancel_receiver.get().unwrap().clone();

                if send {
                    let path = path.as_ref().unwrap().clone();
                    let filename = PathBuf::from(&path.file_name().ok_or(UIError::new("Path error"))?);

                    let mutex_obj = Mutex::new(obj);

                    async_std::task::spawn(async move {
                        let cancel_future = async move {
                            cancel_receiver.recv().await;
                            log::debug!("Canceled transfer");
                        };

                        let res = transfer::send_file_or_folder(wormhole, transit_url, &path, &filename, transit_abilities, progress_handler, cancel_future).await;

                        glib::MainContext::default().invoke(move || {

                            let obj = WarpApplicationWindow::default().action_view();
                            let obj_ = imp::ActionView::from_instance(&obj);
                            obj.show_progress_indeterminate(false);
                            obj_.progress_bar.set_fraction(1 as f64);

                            match res {
                                Ok(_) => {
                                    obj_.status_page.set_title("File transfer successful");
                                    obj_.status_page.set_description(Some("Successfully sent file to peer"));
                                },
                                Err(err) => {
                                    obj.back_clicked();
                                    AppError::from(err).handle();
                                }
                            }
                        });
                    });
                }

                Ok(())
            }),
        );
    }

    pub fn send_file(&self, path: PathBuf) {
        self.transmit(Some(path), None, true);
    }

    pub fn receive_file(&self, code: String) {
        self.transmit(None, Some(Code(code)), false);
    }
}

impl Default for ActionView {
    fn default() -> Self {
        Self::new()
    }
}
