use super::util;
use crate::glib::clone;
use crate::globals;
use crate::ui::window::WarpApplicationWindow;
use crate::util::{cancelable_future, do_async, spawn_async, AppError, UIError};
use gettextrs::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, ResponseType};
use scopeguard::ScopeGuard;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;
use wormhole::transfer::TransferError;
use wormhole::{transfer, transit, Code, Wormhole};

#[derive(Clone, Debug, PartialEq)]
pub enum UIState {
    Initial,
    Archive,
    RequestCode,
    HasCode(Code),
    Connected,
    Transmitting,
    Done(PathBuf),
}

impl Default for UIState {
    fn default() -> Self {
        Self::Initial
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransferDirection {
    Send,
    Receive,
}

impl Default for TransferDirection {
    fn default() -> Self {
        Self::Send
    }
}

mod imp {
    use super::*;
    use gtk::gdk::AppLaunchContext;
    use std::cell::RefCell;

    use crate::glib::clone;
    use gtk::gio::AppInfo;
    use gtk::CompositeTemplate;
    use once_cell::sync::OnceCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/net/felinira/warp/ui/action_view.ui")]
    pub struct ActionView {
        #[template_child]
        pub cancel_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
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
        pub cancel_sender: OnceCell<async_channel::Sender<()>>,
        pub cancel_receiver: OnceCell<async_channel::Receiver<()>>,
        pub filename: RefCell<Option<PathBuf>>,
        pub direction: RefCell<TransferDirection>,
        pub ui_state: RefCell<UIState>,
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
                    obj.cancel();
                }));

            self.back_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    WarpApplicationWindow::default().navigate_back();
                }));

            self.progress_bar.set_pulse_step(0.05);

            self.code_copy_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    let obj_ = imp::ActionView::from_instance(&obj);
                    let code = obj_.code_entry.text();
                    let window = WarpApplicationWindow::default();
                    let clipboard = window.display().clipboard();
                    clipboard.set_text(&code);
                    let toast = adw::Toast::new(&gettext("Copied code to clipboard"));
                    toast.set_timeout(3);
                    toast.set_priority(adw::ToastPriority::Normal);
                    window.toast_overlay().add_toast(&toast);
                }));

            self.code_entry.connect_has_focus_notify(|entry| {
                // Select all text when entry is focused
                if entry.has_focus() {
                    entry.select_region(0, -1);
                }
            });

            self.open_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    let obj_ = imp::ActionView::from_instance(&obj);
                    if let Some(filename) = obj_.filename.borrow().clone() {
                        let uri = glib::filename_to_uri(filename, None);
                        if let Ok(uri) = uri {
                            let none: Option<&AppLaunchContext> = None;
                            let _ = AppInfo::launch_default_for_uri(&uri.to_string(), none);
                        }
                    };
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

    fn set_ui_state(&self, ui_state: UIState) {
        imp::ActionView::from_instance(self)
            .ui_state
            .replace(ui_state);
        self.update_ui();
    }

    fn ui_state(&self) -> UIState {
        imp::ActionView::from_instance(self)
            .ui_state
            .borrow()
            .clone()
    }

    fn set_direction(&self, direction: TransferDirection) {
        imp::ActionView::from_instance(self)
            .direction
            .replace(direction);
    }

    fn direction(&self) -> TransferDirection {
        *imp::ActionView::from_instance(self).direction.borrow()
    }

    fn update_ui(&self) {
        let self_ = imp::ActionView::from_instance(self);
        let direction = self.direction();
        let ui_state = self.ui_state();

        match ui_state {
            UIState::Initial => {
                self_.filename.replace(None);
                self_.open_button.set_visible(false);
                self_.cancel_button.set_visible(true);
                self_.back_button.set_visible(false);
                self_.code_box.set_visible(false);
                self_.progress_bar.set_visible(true);
                self_.progress_bar.set_show_text(false);
                self_
                    .status_page
                    .set_icon_name(Some("arrows-questionmark-symbolic"));
                self.show_progress_indeterminate(true);
            }
            UIState::Archive => match direction {
                TransferDirection::Send => {
                    self_.status_page.set_icon_name(Some("drawer-symbolic"));
                    self_
                        .status_page
                        .set_title(&gettext("Creating archive for folder"));
                    self_
                        .status_page
                        .set_description(Some(&gettext("Compressing folder")));
                }
                TransferDirection::Receive => {
                    // We don't create archives here
                }
            },
            UIState::RequestCode => match direction {
                TransferDirection::Send => {
                    self_
                        .status_page
                        .set_icon_name(Some("arrows-questionmark-symbolic"));
                    self_.status_page.set_title(&gettext("Waiting for code"));
                    self_
                        .status_page
                        .set_description(Some(&gettext("Code is being requested")));
                }
                TransferDirection::Receive => {}
            },
            UIState::HasCode(code) => match direction {
                TransferDirection::Send => {
                    self_.status_page.set_icon_name(Some("code-symbolic"));
                    self_
                        .status_page
                        .set_title(&gettext("Please send the code to the receiver"));
                    self_.status_page.set_description(None);
                    self_.code_box.set_visible(true);
                    self_.code_entry.set_text(&code);
                    self_.progress_bar.set_visible(false);
                }
                TransferDirection::Receive => {
                    self_
                        .status_page
                        .set_icon_name(Some("arrows-questionmark-symbolic"));
                    self_
                        .status_page
                        .set_title(&gettext("Waiting for connection"));
                    self_
                        .status_page
                        .set_description(Some(&gettext!("Connecting to peer with code {}", code)));
                    self_.progress_bar.set_visible(true);
                }
            },
            UIState::Connected => {
                self_.status_page.set_title(&gettext("Connected to peer"));
                self_.code_box.set_visible(false);
                self_.progress_bar.set_visible(true);

                match direction {
                    TransferDirection::Send => {
                        self_
                            .status_page
                            .set_description(Some(&gettext("Preparing to send file")));
                        self_
                            .status_page
                            .set_icon_name(Some("horizontal-arrows-left-symbolic"));
                    }
                    TransferDirection::Receive => {
                        self_
                            .status_page
                            .set_description(Some(&gettext("Preparing to receive file")));
                        self_
                            .status_page
                            .set_icon_name(Some("horizontal-arrows-right-symbolic"));
                    }
                }
            }
            UIState::Transmitting => {
                self.show_progress_indeterminate(false);
                self_.progress_bar.set_show_text(true);
                if direction == TransferDirection::Send {
                    self_
                        .status_page
                        .set_description(Some(&gettext("Sending file")));
                } else {
                    self_
                        .status_page
                        .set_description(Some(&gettext("Receiving file")));
                }
            }
            UIState::Done(path) => {
                self_
                    .status_page
                    .set_title(&gettext("File transfer successful"));
                self_.back_button.set_visible(true);
                self_.cancel_button.set_visible(false);
                self_
                    .status_page
                    .set_icon_name(Some("checkmark-large-symbolic"));

                if direction == TransferDirection::Send {
                    self_
                        .status_page
                        .set_description(Some(&gettext("Successfully sent file")));
                } else {
                    let filename = path.file_name().unwrap();
                    self_.status_page.set_description(Some(&gettext!(
                        "File has been saved to the Downloads folder as {}",
                        filename.to_string_lossy()
                    )));
                    self_.open_button.set_visible(true);
                }
            }
        }
    }

    pub fn cancel(&self) {
        log::info!("Cancelling transfer");

        do_async(clone!(@strong self as obj => async move {
            let obj_ = imp::ActionView::from_instance(&obj);
            obj_.cancel_sender.get().unwrap().send(()).await.unwrap();

            if let Some(path) = obj_.filename.borrow().clone() {
                log::info!("Removing partially downloaded file '{}'", path.display());
                if let Err(err) = std::fs::remove_file(&path) {
                    log::error!("Error removing {}: {}", path.display(), err);
                }
            }
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

    async fn prepare_and_open_file(
        &self,
        path: &Path,
    ) -> Result<Option<(async_std::fs::File, ScopeGuard<PathBuf, fn(PathBuf)>)>, AppError> {
        let mut is_temp = false;
        let file_path = if path.is_dir() {
            self.set_ui_state(UIState::Archive);
            is_temp = true;
            util::compress_folder_cancelable(path, Self::cancel_future()).await?
        } else if path.is_file() {
            Some(path.to_path_buf())
        } else {
            return Err(UIError::new(&gettext("Specified file / directory does not exist")).into());
        };

        if let Some(file_path) = file_path {
            let file = async_std::fs::OpenOptions::new()
                .read(true)
                .open(&file_path)
                .await?;

            let guard: ScopeGuard<PathBuf, fn(PathBuf)> = if is_temp {
                scopeguard::guard(file_path, |path| {
                    log::debug!("Removing residual temporary file {}", path.display());
                    let _ignore = std::fs::remove_file(path);
                })
            } else {
                scopeguard::guard(file_path, |path| {
                    log::debug!("Dropping file_path {}", path.display());
                })
            };

            Ok(Some((file, guard)))
        } else {
            Ok(None)
        }
    }

    fn transmit(
        &self,
        path: PathBuf,
        code: Option<Code>,
        direction: TransferDirection,
    ) -> Result<(), AppError> {
        let path = path;
        self.set_direction(direction);
        self.set_ui_state(UIState::Initial);

        WarpApplicationWindow::default().show_action_view();

        if direction == TransferDirection::Receive {
            let code = code.as_ref().unwrap().clone();
            self.set_ui_state(UIState::HasCode(code));
        }

        do_async(
            clone!(@strong self as obj => @default-return Ok(()), async move {
                let obj_ = imp::ActionView::from_instance(&obj);

                // Drain cancel receiver from any previous transfers
                while obj_.cancel_receiver.get().unwrap().try_recv().is_ok() {}

                let file_tuple = if direction == TransferDirection::Send {
                    obj.prepare_and_open_file(&path).await?
                } else {
                    None
                };

                let wormhole = if direction == TransferDirection::Send {
                    obj.set_ui_state(UIState::RequestCode);
                    let res = cancelable_future(Wormhole::connect_without_code(globals::WORMHOLE_APPCFG.clone(), 4), Self::cancel_future()).await?;

                    let (welcome, connection)= match res {
                        Ok(tuple) => tuple,
                        Err(err) => {
                            return Err(err.into());
                        }
                    };

                    obj.set_ui_state(UIState::HasCode(welcome.code.clone()));
                    let connection = cancelable_future(connection, Self::cancel_future()).await??;

                    log::debug!("Connected to wormhole");
                    connection
                } else {
                    // Method invariant
                    let code = code.unwrap();
                    let (_welcome, connection) = cancelable_future(
                        Wormhole::connect_with_code(
                            globals::WORMHOLE_APPCFG.clone(),
                            code
                        ), Self::cancel_future()).await??;

                    connection
                };

                obj.set_ui_state(UIState::Connected);

                let transit_abilities = transit::Abilities::ALL_ABILITIES;
                let transit_url = url::Url::parse(globals::WORMHOLE_TRANSIT_RELAY)?;

                if direction == TransferDirection::Send {
                    spawn_async(async move {
                        if let Some((mut file, path)) = file_tuple {
                            let metadata = file.metadata().await?;
                            let filename = if let Some(filename) = path.file_name() {
                                filename
                            } else {
                                return Err(std::io::Error::from(std::io::ErrorKind::NotFound).into());
                            };

                            let res = transfer::send_file(wormhole,
                                transit_url,
                                &mut file,
                                &filename,
                                metadata.len(),
                                transit_abilities,
                                Self::progress_handler,
                                Self::cancel_future()
                            ).await;

                            Self::handle_transfer_result(res, &path);
                        }

                        Ok(())
                    });
                } else {
                    // receive
                    let request = transfer::request_file(wormhole,
                        transit_url,
                        transit_abilities,
                        Self::cancel_future()
                    ).await?;

                    let request = if let Some(request) = request {
                        request
                    } else {
                        return Err(AppError::Canceled);
                    };

                    // Only use the last filename component otherwise the other side can overwrite
                    // files in different directories
                    let filename = if let Some(file_name) = request.filename.file_name() {
                        PathBuf::from(file_name)
                    } else {
                        PathBuf::from("Unknown File.bin")
                    };

                    let dialog = Self::save_file_dialog(&filename, request.filesize);
                    let answer = dialog.run_future().await;
                    dialog.close();

                    if answer == gtk::ResponseType::Cancel {
                        async_std::task::spawn(async move {
                            let _ = request.reject().await;
                        });

                        obj.cancel();
                        return Err(AppError::Canceled);
                    }

                    let path = path.join(&filename);

                    let (file_res, path) = util::open_file_find_new_filename_if_exists(&path).await;
                    obj_.filename.replace(Some(path.clone()));

                    spawn_async(async move {
                        log::info!("Downloading file to {:?}", path.to_str());

                        let mut file = file_res?;

                        let res = request.accept(Self::progress_handler, &mut file, Self::cancel_future()).await;
                        Self::handle_transfer_result(res, &path);
                        Ok(())
                    });
                }

                Ok(())
            }),
        );

        Ok(())
    }

    fn cancel_future() -> impl Future<Output = ()> {
        let obj = WarpApplicationWindow::default().action_view();
        let obj_ = imp::ActionView::from_instance(&obj);
        let cancel_receiver = obj_.cancel_receiver.get().unwrap().clone();

        async move {
            loop {
                let res = cancel_receiver.recv().await;
                match res {
                    Ok(()) => {
                        break;
                    }
                    Err(err) => {
                        panic!("{:?}", err);
                    }
                }
            }

            log::debug!("Canceled transfer");
        }
    }

    fn progress_handler(sent: u64, total: u64) {
        glib::MainContext::default().invoke(move || {
            let obj = WarpApplicationWindow::default().action_view();
            let obj_ = imp::ActionView::from_instance(&obj);

            if *obj_.ui_state.borrow() != UIState::Transmitting {
                obj.set_ui_state(UIState::Transmitting);
            }

            obj_.progress_bar.set_fraction(sent as f64 / total as f64);
            obj_.progress_bar.set_text(Some(&format!(
                "{} / {}",
                pretty_bytes::converter::convert(sent as f64),
                pretty_bytes::converter::convert(total as f64)
            )));
        });
    }

    fn save_file_dialog(filename: &Path, size: u64) -> gtk::MessageDialog {
        let dialog = gtk::builders::MessageDialogBuilder::new()
            .text(&gettext("Accept file transfer?"))
            .secondary_text(&gettext!(
                "Your peer wants to send you the file “{}” (Size: {}). Do you want to download this file to your Downloads folder?",
                filename.display(),
                pretty_bytes::converter::convert(size as f64)
            ))
            .message_type(gtk::MessageType::Question)
            .buttons(gtk::ButtonsType::None)
            .transient_for(&WarpApplicationWindow::default())
            .modal(true)
            .build();
        dialog.add_buttons(&[
            ("Cancel", ResponseType::Cancel),
            ("Download", ResponseType::Ok),
        ]);
        dialog
    }

    fn handle_transfer_result(res: Result<(), TransferError>, path: &Path) {
        let path = path.to_path_buf();

        glib::MainContext::default().invoke(move || {
            let obj = WarpApplicationWindow::default().action_view();
            let obj_ = imp::ActionView::from_instance(&obj);
            obj.show_progress_indeterminate(false);
            obj_.progress_bar.set_fraction(1.0);

            match res {
                Ok(_) => obj.set_ui_state(UIState::Done(path)),
                Err(err) => {
                    obj.cancel();
                    AppError::from(err).handle();
                }
            }
        });
    }

    pub fn send_file(&self, path: PathBuf) {
        log::info!("Sending file: {}", path.display());
        if let Err(err) = self.transmit(path, None, TransferDirection::Send) {
            err.handle();
        }
    }

    fn receive_file_impl(&self, code: String) -> Result<(), AppError> {
        let path = if let Some(downloads) = glib::user_special_dir(glib::UserDirectory::Downloads) {
            downloads
        } else {
            return Err(UIError::new(&gettext(
                "Downloads dir missing. Please set XDG_DOWNLOADS_DIR",
            ))
            .into());
        };

        self.transmit(path, Some(Code(code)), TransferDirection::Receive)
    }

    pub fn receive_file(&self, code: String) {
        log::info!("Receiving file with code '{}'", code);
        if let Err(err) = self.receive_file_impl(code) {
            err.handle();
        }
    }
}

impl Default for ActionView {
    fn default() -> Self {
        Self::new()
    }
}
