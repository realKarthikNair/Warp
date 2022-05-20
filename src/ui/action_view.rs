use super::fs;
use super::progress::FileTransferProgress;
use crate::gettext::gettextf;
use crate::glib::clone;
use crate::ui::window::WarpApplicationWindow;
use crate::util::error::*;
use crate::util::future::*;
use crate::WarpApplication;
use adw::gio::NotificationPriority;
use gettextrs::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, ResponseType};
use scopeguard::ScopeGuard;
use std::cell::Ref;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;
use wormhole::transit::TransitInfo;
use wormhole::{transfer, transit, Code, Wormhole};

#[derive(Debug)]
pub enum UIState {
    Initial,
    Archive,
    RequestCode,
    HasCode(Code),
    Connected,
    AskConfirmation(String, u64),
    Transmitting(String, TransitInfo, SocketAddr),
    Done(PathBuf),
    Error(AppError),
}

/// We are only interested about the state, not the context information
impl PartialEq for UIState {
    fn eq(&self, other: &Self) -> bool {
        match self {
            UIState::Initial => matches!(other, UIState::Initial),
            UIState::Archive => matches!(other, UIState::Archive),
            UIState::RequestCode => matches!(other, UIState::RequestCode),
            UIState::HasCode(..) => matches!(other, UIState::HasCode(..)),
            UIState::Connected => matches!(other, UIState::Connected),
            UIState::AskConfirmation(..) => matches!(other, UIState::AskConfirmation(..)),
            UIState::Transmitting(..) => matches!(other, UIState::Transmitting(..)),
            UIState::Done(_) => matches!(other, UIState::Done(..)),
            UIState::Error(_) => matches!(other, UIState::Error(..)),
        }
    }
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

static TRANSIT_ABILITIES: transit::Abilities = transit::Abilities::ALL_ABILITIES;

mod imp {
    use super::*;
    use gtk::gdk::AppLaunchContext;
    use std::cell::{Cell, RefCell};

    use crate::glib::clone;
    use crate::globals::TRANSMIT_URI_PREFIX;
    use gtk::gio::AppInfo;
    use gtk::CompositeTemplate;
    use once_cell::sync::OnceCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/Warp/ui/action_view.ui")]
    pub struct ActionView {
        #[template_child]
        pub cancel_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub accept_transfer_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub code_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub code_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub link_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub code_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub copy_error_button: TemplateChild<gtk::Button>,

        // ID of the timer that runs the indeterminate progress
        pub progress_timeout_source_id: RefCell<Option<glib::source::SourceId>>,

        // User initiated cancel
        pub canceled: Cell<bool>,

        // When sending a message to the cancel sender the whole process gets aborted
        pub cancel_sender: OnceCell<async_channel::Sender<()>>,
        pub cancel_receiver: OnceCell<async_channel::Receiver<()>>,

        // Send a message to this sender to continue the process after the confirmation question
        pub continue_sender: OnceCell<async_channel::Sender<()>>,
        pub continue_receiver: OnceCell<async_channel::Receiver<()>>,

        // Full path to the received/sent file
        pub filename: RefCell<Option<PathBuf>>,

        // Whether we are currently sending or receiving
        pub direction: RefCell<TransferDirection>,

        // The current UI mode
        pub ui_state: RefCell<UIState>,

        // Handle to the progress calculation
        pub progress: RefCell<Option<FileTransferProgress>>,

        // The transit url in use
        pub transit_url: RefCell<Option<url::Url>>,
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
                    main_async_local(super::ActionView::transmit_error_handler, async move {
                        obj.cancel_request().await;
                        Ok(())
                    });
                }));

            self.back_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    WarpApplicationWindow::default().navigate_back();
                }));

            self.accept_transfer_button
                .connect_clicked(clone!(@weak obj => move |_|
                    main_async_local(super::ActionView::transmit_error_handler, async move {
                        if let Some(continue_sender) = obj.imp().continue_sender.get() {
                            continue_sender.send(()).await.unwrap();
                        }

                        Ok(())
                    }, );
                ));

            self.progress_bar.set_pulse_step(0.05);

            self.code_copy_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    let code = obj.imp().code_entry.text();
                    let window = WarpApplicationWindow::default();
                    let clipboard = window.display().clipboard();

                    clipboard.set_text(&code);

                    // Translators: Notification when clicking on "Copy Code to Clipboard" button
                    let toast = adw::Toast::new(&gettext("Copied Code to Clipboard"));
                    toast.set_timeout(3);
                    toast.set_priority(adw::ToastPriority::Normal);
                    window.toast_overlay().add_toast(&toast);
                }));

            self.link_copy_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    let code = obj.imp().code_entry.text();
                    let window = WarpApplicationWindow::default();
                    let clipboard = window.display().clipboard();

                    let link = format!("{}{}", TRANSMIT_URI_PREFIX, code);
                    clipboard.set_text(&link);

                    // Translators: Notification when clicking on "Copy Link to Clipboard" button
                    let toast = adw::Toast::new(&gettext("Copied Link to Clipboard"));
                    toast.set_timeout(3);
                    toast.set_priority(adw::ToastPriority::Normal);
                    window.toast_overlay().add_toast(&toast);
                }));

            self.copy_error_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    let window = WarpApplicationWindow::default();

                    let toast = if let UIState::Error(error) = &*obj.ui_state() {
                        let msg = format!("{:?}", error);
                        window.display().clipboard().set_text(&msg);

                        adw::Toast::new("Copied Error to Clipboard")
                    } else {
                        adw::Toast::new(&gettext("No error available"))
                    };

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
                    if let Some(filename) = obj.imp().filename.borrow().clone() {
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

            let (continue_sender, continue_receiver) = async_channel::unbounded();
            self.continue_sender.get_or_init(|| continue_sender);
            self.continue_receiver.get_or_init(|| continue_receiver);
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
        self.imp().ui_state.replace(ui_state);
        self.update_ui();
    }

    fn ui_state(&self) -> Ref<UIState> {
        self.imp().ui_state.borrow()
    }

    fn set_direction(&self, direction: TransferDirection) {
        self.imp().direction.replace(direction);
    }

    fn direction(&self) -> TransferDirection {
        *self.imp().direction.borrow()
    }

    fn update_ui(&self) {
        let imp = self.imp();
        let direction = self.direction();
        let ui_state = self.ui_state();

        match &*ui_state {
            UIState::Initial => {
                imp.canceled.set(false);
                imp.filename.replace(None);
                imp.progress.replace(None);
                imp.open_button.set_visible(false);
                imp.cancel_button.set_visible(true);
                imp.accept_transfer_button.set_visible(false);
                imp.back_button.set_visible(false);
                imp.code_box.set_visible(false);
                imp.progress_bar.set_visible(true);
                imp.progress_bar.set_show_text(false);
                imp.copy_error_button.set_visible(false);
                imp.status_page
                    .set_icon_name(Some("arrows-questionmark-symbolic"));
                self.show_progress_indeterminate(true);
            }
            UIState::Archive => match direction {
                TransferDirection::Send => {
                    imp.status_page.set_icon_name(Some("drawer-symbolic"));
                    // Translators: Title
                    imp.status_page.set_title(&gettext("Creating Archive"));
                    imp.status_page
                        // Translators: Description
                        .set_description(Some(&gettext("Compressing folder")));
                }
                TransferDirection::Receive => {
                    // We don't create archives here
                }
            },
            UIState::RequestCode => match direction {
                TransferDirection::Send => {
                    imp.status_page
                        .set_icon_name(Some("arrows-questionmark-symbolic"));
                    // Translators: Title
                    imp.status_page.set_title(&gettext("Connecting"));
                    imp.status_page
                        // Translators: Description, Filename
                        .set_description(Some(&gettext("Requesting file transfer")));
                }
                TransferDirection::Receive => {}
            },
            UIState::HasCode(code) => match direction {
                TransferDirection::Send => {
                    imp.status_page.set_icon_name(Some("code-symbolic"));
                    // Translators: Title
                    imp.status_page.set_title(&gettext("Your Transfer Code"));
                    imp.status_page.set_description(Some(&gettext(
                        // Translators: Description, Code in box below
                        "The receiver needs to enter this code to begin the file transfer",
                    )));
                    imp.code_box.set_visible(true);
                    imp.code_entry.set_text(code);
                    imp.progress_bar.set_visible(false);
                }
                TransferDirection::Receive => {
                    imp.status_page
                        .set_icon_name(Some("arrows-questionmark-symbolic"));
                    // Translators: Title
                    imp.status_page.set_title(&gettext("Connecting"));
                    imp.status_page.set_description(Some(&gettextf(
                        // Translators: Description, Transfer Code
                        "Connecting to peer with code “{}”",
                        &[&code],
                    )));
                    imp.progress_bar.set_visible(true);
                }
            },
            UIState::Connected => {
                // Translators: Title
                imp.status_page.set_title(&gettext("Connected to Peer"));
                imp.code_box.set_visible(false);
                imp.progress_bar.set_visible(true);

                match direction {
                    TransferDirection::Send => {
                        imp.status_page
                            // Translators: Description
                            .set_description(Some(&gettext("Preparing to send file")));
                        imp.status_page.set_icon_name(Some("send-to-symbolic"));
                    }
                    TransferDirection::Receive => {
                        imp.status_page
                            // Translators: Description
                            .set_description(Some(&gettext("Preparing to receive file")));
                        imp.status_page
                            .set_icon_name(Some("folder-download-symbolic"));
                    }
                }
            }
            UIState::AskConfirmation(filename, size) => {
                self.show_progress_indeterminate(false);
                imp.accept_transfer_button.set_visible(true);
                imp.progress_bar.set_visible(false);

                imp.status_page.set_icon_name(Some("paper-filled-symbolic"));
                imp.status_page.set_title(&gettext("Accept File Transfer?"));
                imp.status_page.set_description(Some(&gettextf(
                    // Translators: File receive confirmation message dialog; Filename, File size
                    "Your peer wants to send you “{0}” (Size: {1}).\nDo you want to download this file to your Downloads folder?",
                    &[&filename,
                        &glib::format_size(*size)]
                )));

                let notification = gio::Notification::new(&gettext("Ready to Receive File"));
                notification.set_body(Some(&gettext(
                    "A file is ready to be transferred. The transfer needs to be acknowledged.",
                )));
                notification.set_priority(NotificationPriority::Urgent);
                notification.set_category(Some("transfer"));
                WarpApplication::default()
                    .send_notification_if_background(Some("receive-ready"), &notification);
            }
            UIState::Transmitting(filename, info, peer_addr) => {
                self.show_progress_indeterminate(false);
                imp.accept_transfer_button.set_visible(false);
                imp.progress_bar.set_visible(true);
                imp.progress_bar.set_show_text(true);

                let mut ip = peer_addr.ip();
                // We convert ipv4 mapped ipv6 addresses because the gio code can't tell if they are
                // local or not
                if let IpAddr::V6(ipv6) = ip {
                    if let Some(ipv4) = ipv6.to_ipv4() {
                        ip = IpAddr::from(ipv4);
                    }
                }

                let gio_addr = gio::InetAddress::from(ip);
                let is_site_local = gio_addr.is_site_local();

                let description = match info {
                    TransitInfo::Direct => {
                        if is_site_local {
                            // Translators: Description, During transfer
                            gettextf("File “{}” via local network direct transfer", &[&filename])
                        } else {
                            // Translators: Description, During transfer
                            gettextf("File “{}” via direct transfer", &[&filename])
                        }
                    }
                    TransitInfo::Relay { name } => {
                        if let Some(name) = name {
                            // Translators: Description, During transfer
                            gettextf("File “{0}” via relay {1}", &[&filename, &name])
                        } else {
                            // Translators: Description, During transfer
                            gettextf("File “{}” via relay", &[&filename])
                        }
                    }
                    // Translators: Description, During transfer
                    _ => gettextf("File “{}” via Unknown connection method", &[&filename]),
                };

                imp.status_page.set_description(Some(&description));

                if direction == TransferDirection::Send {
                    // Translators: Title
                    imp.status_page.set_title(&gettext("Sending File"));
                    imp.status_page.set_icon_name(Some("send-to-symbolic"));
                } else {
                    // Translators: Title
                    imp.status_page.set_title(&gettext("Receiving File"));
                    imp.status_page
                        .set_icon_name(Some("folder-download-symbolic"));
                }
            }
            UIState::Done(path) => {
                imp.status_page
                    // Translators: Title
                    .set_title(&gettext("File Transfer Successful"));
                imp.back_button.set_visible(true);
                imp.cancel_button.set_visible(false);
                imp.status_page
                    .set_icon_name(Some("checkmark-large-symbolic"));
                imp.progress_bar.set_text(None);
                imp.progress_bar.set_visible(false);

                let notification = gio::Notification::new(&gettext("File Transfer Complete"));

                notification.set_priority(NotificationPriority::High);
                notification.set_category(Some("transfer.complete"));

                let filename = path.file_name().unwrap().to_string_lossy();
                if direction == TransferDirection::Send {
                    let description = gettextf(
                        // Translators: Description, Filename
                        "Successfully sent file “{}”",
                        &[&filename],
                    );

                    imp.status_page.set_description(Some(&description));
                    notification.set_body(Some(&description));
                } else {
                    let description = gettextf(
                        // Translators: Description, Filename
                        "File has been saved to the Downloads folder as “{}”",
                        &[&filename],
                    );

                    imp.status_page.set_description(Some(&description));
                    imp.open_button.set_visible(true);
                    notification.set_body(Some(&description));
                }

                WarpApplication::default()
                    .send_notification_if_background(Some("transfer-complete"), &notification);
            }
            UIState::Error(error) => {
                imp.status_page
                    // Translators: Title
                    .set_title(&gettext("File Transfer Failed"));
                imp.status_page
                    .set_description(Some(&error.gettext_error()));
                imp.back_button.set_visible(true);
                imp.cancel_button.set_visible(false);
                imp.status_page
                    .set_icon_name(Some("horizontal-arrows-one-way-symbolic"));
                imp.progress_bar.set_text(None);
                imp.progress_bar.set_visible(false);

                let notification = gio::Notification::new(&gettext("File Transfer Failed"));
                notification.set_body(Some(&gettextf(
                    "The file transfer failed: {}",
                    &[&error.gettext_error()],
                )));
                notification.set_priority(NotificationPriority::High);
                notification.set_category(Some("transfer.error"));
                WarpApplication::default()
                    .send_notification_if_background(Some("transfer-error"), &notification);

                if !matches!(error, AppError::Canceled) {
                    imp.copy_error_button.set_visible(true);
                }
            }
        }
    }

    pub async fn cancel_request(&self) -> bool {
        if matches!(
            &*self.imp().ui_state.borrow(),
            UIState::AskConfirmation(..) | UIState::Done(..) | UIState::Error(..)
        ) {
            self.cancel();
            return true;
        }

        let dialog = Self::ask_abort_dialog();
        let answer = dialog.run_future().await;
        dialog.close();

        if answer == gtk::ResponseType::Cancel {
            self.cancel();
            true
        } else {
            false
        }
    }

    pub fn cancel(&self) {
        log::info!("Cancelling transfer");
        self.imp().canceled.set(true);

        main_async_local_infallible(clone!(@strong self as obj => async move {
            let imp = obj.imp();
            imp.cancel_sender.get().unwrap().send(()).await.unwrap();
        }));

        self.transmit_cleanup();
        WarpApplicationWindow::default().navigate_back();
    }

    pub fn show_progress_indeterminate(&self, pulse: bool) {
        let imp = self.imp();
        if let Some(source_id) = imp.progress_timeout_source_id.take() {
            source_id.remove();
        }

        if pulse {
            // 50 ms was mainly chosen for performance of the progress bar
            imp.progress_timeout_source_id
                .replace(Some(glib::timeout_add_local(
                    Duration::from_millis(50),
                    clone!(@strong self as obj => move || {
                        obj.imp().progress_bar.pulse();

                        Continue(true)
                    }),
                )));
        }
    }

    async fn prepare_and_open_file(
        &self,
        path: &Path,
    ) -> Result<(smol::fs::File, ScopeGuard<PathBuf, fn(PathBuf)>), AppError> {
        let mut is_temp = false;
        let file_path = if path.is_dir() {
            self.set_ui_state(UIState::Archive);
            is_temp = true;
            fs::compress_folder_cancelable(path, Self::cancel_future()).await?
        } else if path.is_file() {
            path.to_path_buf()
        } else {
            // Translators: When opening a file
            return Err(UIError::new(&gettext("Specified file / directory does not exist")).into());
        };

        let file = smol::fs::OpenOptions::new()
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

        Ok((file, guard))
    }

    fn prepare_transmit(&self, direction: TransferDirection) -> Result<(), AppError> {
        WarpApplication::default().inhibit_transfer(direction);
        self.set_direction(direction);
        self.set_ui_state(UIState::Initial);

        let _rendezvous_url = url::Url::parse(
            WarpApplicationWindow::default()
                .config()
                .rendezvous_server_url_or_default(),
        )
        .map_err(|_| {
            UIError::new(&gettext(
                "Error parsing rendezvous server URL. An invalid URL was entered in the settings.",
            ))
        })?;

        let transit_url = url::Url::parse(
            WarpApplicationWindow::default()
                .config()
                .transit_server_url_or_default(),
        )
        .map_err(|_| {
            UIError::new(&gettext(
                "Error parsing transit URL. An invalid URL was entered in the settings.",
            ))
        })?;
        self.imp().transit_url.replace(Some(transit_url));

        WarpApplicationWindow::default().show_action_view();
        Ok(())
    }

    async fn transmit_receive(&self, download_path: PathBuf, code: Code) -> Result<(), AppError> {
        self.prepare_transmit(TransferDirection::Receive)?;
        self.set_ui_state(UIState::HasCode(code.clone()));

        WarpApplicationWindow::default().add_code(code.clone());
        let app_cfg = WarpApplicationWindow::default().config().app_cfg();

        let (_welcome, connection) = cancelable_future(
            Wormhole::connect_with_code(app_cfg, code),
            Self::cancel_future(),
        )
        .await??;
        self.set_ui_state(UIState::Connected);

        let relay_url = self.imp().transit_url.borrow().clone().unwrap();

        let request = transfer::request_file(
            connection,
            relay_url,
            TRANSIT_ABILITIES,
            Self::cancel_future(),
        )
        .await?
        .ok_or(AppError::Canceled)?;

        // Only use the last filename component otherwise the other side can overwrite
        // files in different directories
        let filename = if let Some(file_name) = request.filename.file_name() {
            PathBuf::from(file_name)
        } else {
            PathBuf::from("Unknown File.bin")
        };

        self.set_ui_state(UIState::AskConfirmation(
            filename.to_string_lossy().to_string(),
            request.filesize,
        ));

        // Continue or cancel
        let res = self.ask_confirmation_future().await;
        if res.is_err() {
            spawn_async_infallible(async move {
                let _ = request.reject().await;
            });

            return res;
        }

        WarpApplication::default().withdraw_notification("receive-ready");

        let path = download_path.join(&filename);

        let (file_res, path) = fs::open_file_find_new_filename_if_exists(&path).await;
        self.imp().filename.replace(Some(path.clone()));

        spawn_async(Self::transmit_error_handler, async move {
            log::info!("Downloading file to {:?}", path.to_str());

            let mut file = file_res?;
            request
                .accept(
                    Self::transit_handler,
                    Self::progress_handler,
                    &mut file,
                    Self::cancel_future(),
                )
                .await?;

            if WarpApplicationWindow::default()
                .action_view()
                .imp()
                .canceled
                .get()
            {
                return Err(AppError::Canceled);
            }

            Self::transmit_success_main(path);

            Ok(())
        });

        Ok(())
    }

    async fn transmit_send(&self, path: PathBuf) -> Result<(), AppError> {
        self.prepare_transmit(TransferDirection::Send)?;
        self.set_ui_state(UIState::RequestCode);

        let (mut file, path) = self.prepare_and_open_file(&path).await?;
        let app_cfg = WarpApplicationWindow::default().config().app_cfg();

        let res = cancelable_future(
            Wormhole::connect_without_code(app_cfg, 4),
            Self::cancel_future(),
        )
        .await?;

        let (welcome, connection) = match res {
            Ok(tuple) => tuple,
            Err(err) => {
                return Err(err.into());
            }
        };

        WarpApplicationWindow::default().add_code(welcome.code.clone());
        self.set_ui_state(UIState::HasCode(welcome.code.clone()));
        let connection = cancelable_future(connection, Self::cancel_future()).await??;
        self.set_ui_state(UIState::Connected);

        self.imp().filename.replace(Some((*path).to_path_buf()));
        let transit_url = self.imp().transit_url.borrow().clone().unwrap();

        spawn_async(Self::transmit_error_handler, async move {
            let filename = if let Some(filename) = path.file_name() {
                filename
            } else {
                return Err(std::io::Error::from(std::io::ErrorKind::NotFound).into());
            };
            let metadata = file.metadata().await?;

            transfer::send_file(
                connection,
                transit_url,
                &mut file,
                &filename,
                metadata.len(),
                TRANSIT_ABILITIES,
                Self::transit_handler,
                Self::progress_handler,
                Self::cancel_future(),
            )
            .await?;

            if WarpApplicationWindow::default()
                .action_view()
                .imp()
                .canceled
                .get()
            {
                return Err(AppError::Canceled);
            }

            // We can drop the path now, we don't need the temp file anymore
            Self::transmit_success_main(path.clone());

            Ok(())
        });

        Ok(())
    }

    fn cancel_future() -> impl Future<Output = ()> {
        let obj = ActionView::default();
        let cancel_receiver = obj.imp().cancel_receiver.get().unwrap().clone();

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

    fn transit_handler(info: TransitInfo, peer_ip: SocketAddr) {
        glib::MainContext::default().invoke(move || {
            let obj = ActionView::default();
            let imp = obj.imp();

            let filename = imp
                .filename
                .borrow()
                .as_ref()
                .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into()))
                .unwrap_or_else(|| "".to_string());

            obj.set_ui_state(UIState::Transmitting(filename, info, peer_ip));
        });
    }

    fn progress_handler(sent: u64, total: u64) {
        glib::MainContext::default().invoke(move || {
            let obj = ActionView::default();
            let imp = obj.imp();

            if imp.progress.borrow().is_none() {
                imp.progress
                    .replace(Some(FileTransferProgress::begin(total as usize)));
            }

            let mut update_progress = false;
            let progress_str = imp
                .progress
                .borrow_mut()
                .as_mut()
                .and_then(|progress| {
                    update_progress = progress.set_progress(sent as usize);
                    progress.get_pretty_time_remaining()
                })
                .unwrap_or_else(|| "".to_string());

            if update_progress {
                imp.progress_bar.set_fraction(sent as f64 / total as f64);
                imp.progress_bar.set_text(Some(&progress_str));
            }
        });
    }

    async fn ask_confirmation_future(&self) -> Result<(), AppError> {
        cancelable_future(
            self.imp().continue_receiver.get().unwrap().recv(),
            Self::cancel_future(),
        )
        .await??;
        Ok(())
    }

    fn ask_abort_dialog() -> gtk::MessageDialog {
        let dialog = gtk::builders::MessageDialogBuilder::new()
            // Translators: File receive confirmation message dialog title
            .text(&gettext("Abort file transfer?"))
            .secondary_text(&gettext("Do you want to abort the current file transfer?"))
            .message_type(gtk::MessageType::Question)
            .buttons(gtk::ButtonsType::None)
            .transient_for(&WarpApplicationWindow::default())
            .modal(true)
            .build();
        let _continue_button = dialog.add_button(&gettext("Continue"), ResponseType::Close);
        let abort_button = dialog.add_button(&gettext("Abort"), ResponseType::Cancel);
        abort_button.add_css_class("destructive-action");
        dialog
    }

    /// Any post-transfer cleanup operations that are shared between success and failure states
    fn transmit_cleanup(&self) {
        log::debug!("Transmit cleanup");

        let imp = self.imp();
        WarpApplication::default().uninhibit_transfer();
        self.show_progress_indeterminate(false);

        // Drain cancel and continue receiver from any previous transfers
        while imp.cancel_receiver.get().unwrap().try_recv().is_ok() {}
        while imp.continue_receiver.get().unwrap().try_recv().is_ok() {}

        if let Some(path) = imp.filename.borrow().clone() {
            if *imp.direction.borrow() == TransferDirection::Receive
                && !matches!(*imp.ui_state.borrow(), UIState::Done(..))
            {
                log::info!("Removing partially downloaded file '{}'", path.display());
                if let Err(err) = std::fs::remove_file(&path) {
                    log::error!("Error removing {0}: {1}", path.display(), err);
                }
            }
        }
    }

    fn transmit_success_main(path: PathBuf) {
        glib::MainContext::default().invoke(move || {
            WarpApplicationWindow::default()
                .action_view()
                .transmit_success(&path)
        });
    }

    fn transmit_success(&self, path: &Path) {
        log::debug!("Transmit success");

        let path = path.to_path_buf();

        self.imp().progress_bar.set_fraction(1.0);

        self.set_ui_state(UIState::Done(path));
        self.transmit_cleanup();
    }

    pub fn transmit_error(&self, error: AppError) {
        log::debug!("Transmit error");

        if *self.ui_state() != UIState::Initial {
            if matches!(error, AppError::Canceled) {
                // Canceled is initiated intentionally by the user
                return;
            }

            self.set_ui_state(UIState::Error(error));
        } else {
            error.handle();
        }

        self.transmit_cleanup();
    }

    pub fn transmit_error_handler(error: AppError) {
        WarpApplicationWindow::default()
            .action_view()
            .transmit_error(error);
    }

    pub fn send_file(&self, path: PathBuf) {
        log::info!("Sending file: {}", path.display());
        let obj = self.clone();

        main_async_local(Self::transmit_error_handler, async move {
            obj.transmit_send(path).await?;
            Ok(())
        });
    }

    fn receive_file_impl(&self, code: Code) -> Result<(), AppError> {
        let path = if let Some(downloads) = glib::user_special_dir(glib::UserDirectory::Downloads) {
            downloads
        } else {
            return Err(UIError::new(&gettext(
                "Downloads dir missing. Please set XDG_DOWNLOADS_DIR",
            ))
            .into());
        };

        let obj = self.clone();

        main_async_local(Self::transmit_error_handler, async move {
            obj.transmit_receive(path, code).await?;
            Ok(())
        });

        Ok(())
    }

    pub fn receive_file(&self, code: Code) {
        log::info!("Receiving file with code '{}'", code);
        if let Err(err) = self.receive_file_impl(code) {
            self.transmit_error(err);
        }
    }

    pub fn transfer_in_progress(&self) -> bool {
        !matches!(&*self.ui_state(), UIState::Done(..) | UIState::Error(..))
    }

    pub fn should_handle_error_inline(&self) -> bool {
        !matches!(
            &*self.ui_state(),
            UIState::Initial | UIState::Done(..) | UIState::Error(..)
        )
    }
}

impl Default for ActionView {
    fn default() -> Self {
        WarpApplicationWindow::default().action_view()
    }
}
