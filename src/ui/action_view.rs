use super::fs;
use super::progress::FileTransferProgress;
use crate::gettext::gettextf;
use crate::glib::clone;
use crate::ui::fs::safe_persist_tempfile;
use crate::ui::window::WarpApplicationWindow;
use crate::util::error::*;
use crate::util::future::*;
use crate::util::{show_dir, TransferDirection, WormholeTransferURI};
use crate::{globals, WarpApplication};
use adw::gio::NotificationPriority;
use gettextrs::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, ResponseType};
use std::ffi::OsString;
use std::fmt::Debug;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use wormhole::transfer::AppVersion;
use wormhole::transit::TransitInfo;
use wormhole::{transfer, transit, AppConfig, Code, Wormhole};

// 5 seconds timeout
const TIMEOUT_MS: u64 = 5000;

#[derive(Debug)]
pub enum UIState {
    Initial,
    Archive(OsString),
    RequestCode,
    HasCode(WormholeTransferURI),
    Connected,
    AskConfirmation(String, u64),
    Transmitting(String, TransitInfo, SocketAddr),
    Done(OsString),
    Error(AppError),
}

/// We are only interested about the state, not the context information
impl PartialEq for UIState {
    fn eq(&self, other: &Self) -> bool {
        match self {
            UIState::Initial => matches!(other, UIState::Initial),
            UIState::Archive(_) => matches!(other, UIState::Archive(..)),
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

/// Mutable state for the `ActionView`
#[derive(Debug)]
pub struct UIContext {
    /// ID of the timer that runs the indeterminate progress
    pub progress_timeout_source_id: Option<glib::source::SourceId>,

    /// When sending a message to the cancel sender the whole process gets aborted
    pub cancel_sender: async_channel::Sender<()>,
    pub cancel_receiver: async_channel::Receiver<()>,

    /// We will send a message to this channel when the transfer was cancelled successfully
    pub cancellation_complete_sender: async_channel::Sender<()>,
    pub cancellation_complete_receiver: async_channel::Receiver<()>,

    /// Send a message to this sender to continue the process after the confirmation question
    pub continue_sender: async_channel::Sender<()>,
    pub continue_receiver: async_channel::Receiver<()>,

    /// User initiated cancel
    pub canceled: bool,

    /// Full path to the currently being received / sent file
    pub file_path: Option<PathBuf>,

    /// File path to the last file that was received successfully
    pub file_path_received_successfully: Option<PathBuf>,

    /// The user facing name of the file being received / send
    pub file_name: Option<OsString>,

    /// Whether we are currently sending or receiving
    pub direction: TransferDirection,

    /// The current UI mode
    pub ui_state: Rc<UIState>,

    /// Handle to the progress calculation
    pub progress: Option<FileTransferProgress>,

    /// The rendezvous url in use
    pub rendezvous_url: url::Url,

    /// The transit url in use
    pub transit_url: url::Url,
}

impl Default for UIContext {
    fn default() -> Self {
        let (cancel_sender, cancel_receiver) = async_channel::unbounded();
        let (continue_sender, continue_receiver) = async_channel::unbounded();
        let (cancellation_complete_sender, cancellation_complete_receiver) =
            async_channel::unbounded();

        Self {
            progress_timeout_source_id: None,
            cancel_sender,
            cancel_receiver,
            cancellation_complete_sender,
            cancellation_complete_receiver,
            continue_sender,
            continue_receiver,
            canceled: false,
            file_path: None,
            file_path_received_successfully: None,
            file_name: None,
            direction: TransferDirection::default(),
            ui_state: Rc::default(),
            progress: None,
            transit_url: globals::WORMHOLE_DEFAULT_TRANSIT_RELAY.clone(),
            rendezvous_url: globals::WORMHOLE_DEFAULT_RENDEZVOUS_SERVER.clone(),
        }
    }
}

static TRANSIT_ABILITIES: transit::Abilities = transit::Abilities::ALL_ABILITIES;

mod imp {
    use super::*;
    use gtk::gdk::AppLaunchContext;
    use std::cell::RefCell;

    use crate::glib::clone;
    //use crate::util::WormholeTransferURI;
    use gtk::gio::AppInfo;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/Warp/ui/action_view.ui")]
    pub struct ActionView {
        #[template_child]
        pub cancel_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub open_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub open_dir_button: TemplateChild<gtk::Button>,
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

        pub context: RefCell<UIContext>,
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
                        let continue_sender = obj.imp().context.borrow().continue_sender.clone();
                        continue_sender.send(()).await.unwrap();

                        Ok(())
                    });
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

                    let uri = WormholeTransferURI {
                        code: Code(code.to_string()),
                        version: 0,
                        rendezvous_server: obj.imp().context.borrow().rendezvous_url.clone(),
                        direction: TransferDirection::Receive,
                    };
                    clipboard.set_text(&uri.create_uri());

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
                        let msg = format!("{}", error);
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
                    if let Some(filename) = obj.imp().context.borrow_mut().file_path_received_successfully.clone() {
                        let uri = glib::filename_to_uri(filename.clone(), None);
                        if let Ok(uri) = uri {
                            log::debug!("Opening file with uri '{}'", uri);
                            let none: Option<&AppLaunchContext> = None;
                            let res = AppInfo::launch_default_for_uri(&uri.to_string(), none);
                            if let Err(err) = res {
                                log::error!("Error opening file: {}", err);
                                main_async_local_infallible(async move {
                                    let dialog = super::ActionView::no_registered_application_error_dialog(err.message());
                                    let answer = dialog.run_future().await;
                                    dialog.close();

                                    if answer == gtk::ResponseType::Ok {
                                        if let Err(err) = show_dir(&filename) {
                                            log::error!("Error opening file: {}", err);
                                            err.handle();
                                        }
                                    }
                                });
                            }
                        } else {
                            log::error!("Filename to open is not a valid uri");
                        }
                    } else {
                        log::error!("Open button clicked but no filename set");
                    };
                }));

            self.open_dir_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    if let Some(filename) = obj.imp().context.borrow_mut().file_path_received_successfully.clone() {
                        if let Err(err) = show_dir(&filename) {
                            err.handle();
                        }
                    };
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

    fn set_ui_state(&self, ui_state: UIState) {
        self.imp().context.borrow_mut().ui_state = Rc::new(ui_state);
        self.update_ui();
    }

    fn ui_state(&self) -> Rc<UIState> {
        self.imp().context.borrow().ui_state.clone()
    }

    fn set_transfer_direction(&self, direction: TransferDirection) {
        self.imp().context.borrow_mut().direction = direction;
    }

    fn transfer_direction(&self) -> TransferDirection {
        self.imp().context.borrow().direction
    }

    fn set_can_navigate_back(&self, can_navigate_back: bool) {
        self.imp().back_button.set_visible(can_navigate_back);
        WarpApplicationWindow::default()
            .leaflet()
            .set_can_navigate_back(can_navigate_back);
    }

    fn update_ui(&self) {
        let imp = self.imp();
        let direction = self.transfer_direction();
        let ui_state = self.ui_state();

        match &*ui_state {
            UIState::Initial => {
                imp.open_box.set_visible(false);
                imp.cancel_button.set_visible(true);
                imp.accept_transfer_button.set_visible(false);
                imp.code_box.set_visible(false);
                imp.progress_bar.set_visible(true);
                imp.progress_bar.set_show_text(false);
                imp.copy_error_button.set_visible(false);
                imp.status_page
                    .set_icon_name(Some("arrows-questionmark-symbolic"));

                self.set_can_navigate_back(false);
                self.show_progress_indeterminate(true);
            }
            UIState::Archive(filename) => match direction {
                TransferDirection::Send => {
                    imp.status_page.set_icon_name(Some("drawer-symbolic"));
                    // Translators: Title
                    imp.status_page.set_title(&gettext("Creating Archive"));
                    imp.status_page
                        // Translators: Description
                        .set_description(Some(&gettextf(
                            "Compressing folder “{}”",
                            &[&filename.to_string_lossy()],
                        )));
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
            UIState::HasCode(uri) => match direction {
                TransferDirection::Send => {
                    imp.status_page.set_icon_name(Some("code-symbolic"));
                    // Translators: Title, this is a noun
                    imp.status_page.set_title(&gettext("Your Transmit Code"));
                    //imp.status_page.set_paintable(Some(&uri.to_paintable_qr()));
                    //imp.status_page.add_css_class("qr");

                    let filename = imp
                        .context
                        .borrow()
                        .file_name
                        .clone()
                        .unwrap_or_else(|| "?".into());

                    if imp.context.borrow().rendezvous_url
                        == *globals::WORMHOLE_DEFAULT_RENDEZVOUS_SERVER
                    {
                        imp.status_page.set_description(Some(&gettextf(
                            // Translators: Description, Code in box below, argument is filename
                            "Ready to send “{}”\nThe receiver needs to enter this code to begin the file transfer.",
                            &[&filename.to_string_lossy()]
                        )));
                    } else {
                        imp.status_page.set_description(Some(&gettextf(
                            // Translators: Description, Code in box below, argument is filename
                            "Ready to send “{}”\nThe receiver needs to enter this code to begin the file transfer.\n\nYou have entered a custom rendezvous server URL in preferences. Please verify the receiver also uses the same rendezvous server.",
                            &[&filename.to_string_lossy()]
                        )));
                    }
                    imp.code_box.set_visible(true);
                    imp.code_entry.set_text(uri.code.as_ref());
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
                        &[&uri.code],
                    )));
                    imp.progress_bar.set_visible(true);
                }
            },
            UIState::Connected => {
                // Translators: Title
                imp.status_page.remove_css_class("qr");
                imp.status_page.set_title(&gettext("Connected to Peer"));
                imp.code_box.set_visible(false);
                imp.accept_transfer_button.set_visible(false);
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
            UIState::Done(filename) => {
                imp.status_page
                    // Translators: Title
                    .set_title(&gettext("File Transfer Successful"));
                imp.cancel_button.set_visible(false);
                imp.status_page
                    .set_icon_name(Some("checkmark-large-symbolic"));
                imp.progress_bar.set_text(None);
                imp.progress_bar.set_visible(false);

                self.set_can_navigate_back(true);

                let notification = gio::Notification::new(&gettext("File Transfer Complete"));

                notification.set_priority(NotificationPriority::High);
                notification.set_category(Some("transfer.complete"));

                if direction == TransferDirection::Send {
                    let description = gettextf(
                        // Translators: Description, Filename
                        "Successfully sent file “{}”",
                        &[&filename.to_string_lossy()],
                    );

                    imp.status_page.set_description(Some(&description));
                    notification.set_body(Some(&description));
                } else {
                    let description = gettextf(
                        // Translators: Description, Filename
                        "File has been saved to the Downloads folder as “{}”",
                        &[&filename.to_string_lossy()],
                    );

                    imp.status_page.set_description(Some(&description));
                    imp.open_box.set_visible(true);
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
                imp.cancel_button.set_visible(false);
                imp.status_page
                    .set_icon_name(Some("horizontal-arrows-one-way-symbolic"));
                imp.progress_bar.set_text(None);
                imp.progress_bar.set_visible(false);
                imp.accept_transfer_button.set_visible(false);

                self.set_can_navigate_back(true);

                let notification = gio::Notification::new(&gettext("File Transfer Failed"));
                notification.set_body(Some(&gettextf(
                    "The file transfer failed: {}",
                    &[&error.gettext_error()],
                )));
                notification.set_priority(NotificationPriority::High);
                notification.set_category(Some("transfer.error"));
                WarpApplication::default()
                    .send_notification_if_background(Some("transfer-error"), &notification);

                let peer_canceled = if let AppError::Transfer {
                    source: wormhole::transfer::TransferError::PeerError(msg),
                } = error
                {
                    msg == "Task has been cancelled"
                } else {
                    false
                };

                if !matches!(error, AppError::Canceled) && !peer_canceled {
                    imp.copy_error_button.set_visible(true);
                }
            }
        }
    }

    /// This will ask whether the transfer should be cancelled. If
    pub async fn cancel_request(&self) -> bool {
        if matches!(
            &*self.imp().context.borrow().ui_state,
            UIState::AskConfirmation(..) | UIState::Done(..) | UIState::Error(..)
        ) {
            self.cancel().await;
            return true;
        }

        let dialog = Self::ask_abort_dialog();
        let answer = dialog.run_future().await;
        dialog.close();

        if answer == gtk::ResponseType::Cancel {
            self.cancel().await;
            true
        } else {
            false
        }
    }

    async fn wait_for_cancellation_future(&self) {
        let channel = self
            .imp()
            .context
            .borrow()
            .cancellation_complete_receiver
            .clone();
        if let Err(err) = channel.recv().await {
            log::error!("Error when waiting for cancellation future {:?}", err);
        }
    }

    pub async fn cancel(&self) {
        log::info!("Cancelling transfer");
        self.imp().context.borrow_mut().canceled = true;

        let imp = self.imp();
        log::debug!("Sending cancel signal");
        let cancel_sender = imp.context.borrow().cancel_sender.clone();
        cancel_sender.send(()).await.unwrap();
        self.wait_for_cancellation_future().await;
        WarpApplicationWindow::default().navigate_back();
    }

    fn show_progress_indeterminate(&self, pulse: bool) {
        let imp = self.imp();
        if let Some(source_id) = imp.context.borrow_mut().progress_timeout_source_id.take() {
            source_id.remove();
        }

        if pulse {
            // 50 ms was mainly chosen for performance of the progress bar
            imp.context.borrow_mut().progress_timeout_source_id = Some(glib::timeout_add_local(
                Duration::from_millis(50),
                clone!(@strong self as obj => move || {
                    obj.imp().progress_bar.pulse();

                    Continue(true)
                }),
            ));
        }
    }

    async fn prepare_and_open_file(
        &self,
        path: &Path,
    ) -> Result<(smol::fs::File, PathBuf, OsString), AppError> {
        let mut filename = if let Some(filename) = path.file_name() {
            filename.to_os_string()
        } else {
            return Err(std::io::Error::from(std::io::ErrorKind::NotFound).into());
        };

        let (file, path) = if path.is_dir() {
            self.set_ui_state(UIState::Archive(filename.clone()));
            filename.push(".zip");

            let temp_file = fs::compress_folder_cancelable(path, Self::cancel_future()).await?;
            (
                smol::fs::File::from(temp_file.reopen()?),
                temp_file.path().to_path_buf(),
            )
        } else if path.is_file() {
            (
                smol::fs::OpenOptions::new().read(true).open(&*path).await?,
                path.to_path_buf(),
            )
        } else {
            // Translators: When opening a file
            return Err(UiError::new(&gettext("Specified file / directory does not exist")).into());
        };

        Ok((file, path, filename))
    }

    fn prepare_transmit(&self, direction: TransferDirection) -> Result<(), AppError> {
        self.reset();
        WarpApplication::default().inhibit_transfer(direction);
        self.set_transfer_direction(direction);
        self.set_ui_state(UIState::Initial);

        let rendezvous_url = WarpApplicationWindow::default()
            .config()
            .rendezvous_server_url()
            .map_err(|_| {
                UiError::new(&gettext(
                "Error parsing rendezvous server URL. An invalid URL was entered in the settings.",
            ))
            })?;
        self.imp().context.borrow_mut().rendezvous_url = rendezvous_url;

        let transit_url = WarpApplicationWindow::default()
            .config()
            .transit_server_url()
            .map_err(|_| {
                UiError::new(&gettext(
                    "Error parsing transit URL. An invalid URL was entered in the settings.",
                ))
            })?;
        self.imp().context.borrow_mut().transit_url = transit_url;

        WarpApplicationWindow::default().show_action_view();
        Ok(())
    }

    async fn transmit_receive(
        &self,
        download_path: PathBuf,
        code: Code,
        app_cfg: AppConfig<AppVersion>,
    ) -> Result<(), AppError> {
        self.prepare_transmit(TransferDirection::Receive)?;
        let uri = WormholeTransferURI::from_app_cfg_with_code_direction(
            &app_cfg,
            &code,
            TransferDirection::Send,
        );
        self.set_ui_state(UIState::HasCode(uri));

        WarpApplicationWindow::default().add_code(&code);

        let (_welcome, connection) = spawn_async(cancelable_future(
            Wormhole::connect_with_code(app_cfg, code),
            Self::cancel_future(),
        ))
        .await??;

        self.set_ui_state(UIState::Connected);

        let relay_url = self.imp().context.borrow().transit_url.clone();

        let request = spawn_async(async move {
            Ok(transfer::request_file(
                connection,
                relay_url,
                TRANSIT_ABILITIES,
                Self::cancel_future(),
            )
            .await?)
        })
        .await?
        .ok_or(AppError::Canceled)?;

        // Only use the last filename component otherwise the other side can overwrite
        // files in different directories
        let filename = if let Some(file_name) = request.filename.file_name() {
            PathBuf::from(file_name)
        } else {
            // This shouldn't happen realistically
            PathBuf::from("Unknown Filename.bin")
        };

        self.set_ui_state(UIState::AskConfirmation(
            filename.to_string_lossy().to_string(),
            request.filesize,
        ));

        // Continue or cancel
        let res = self.ask_confirmation_future().await;
        if res.is_err() {
            smol::spawn(async move {
                let _ = request.reject().await;
            })
            .await;

            return res;
        }

        self.set_ui_state(UIState::Connected);

        WarpApplication::default().withdraw_notification("receive-ready");

        let mut tempfile_prefix = filename.as_os_str().to_os_string();
        tempfile_prefix.push(".");

        let temp_file = tempfile::Builder::new()
            .prefix(&tempfile_prefix)
            .suffix(&".warpdownload")
            .tempfile_in(download_path)?;
        let file = smol::fs::File::from(temp_file.reopen()?);

        self.imp().context.borrow_mut().file_name = Some(filename.as_os_str().to_os_string());

        cancelable_future(
            spawn_async(async move {
                log::info!(
                    "Downloading file to {:?}",
                    temp_file.path().to_string_lossy()
                );

                let mut file = file;
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
                    .context
                    .borrow()
                    .canceled
                {
                    return Err(AppError::Canceled);
                }

                // Windows requires the file to be closed before renaming it
                file.sync_all().await?;
                drop(file);

                // Rename the file to its final name
                let path = safe_persist_tempfile(temp_file, &filename)?;
                let obj = WarpApplicationWindow::default().action_view();
                obj.imp().context.borrow_mut().file_name =
                    Some(path.file_name().unwrap().to_os_string());
                obj.imp()
                    .context
                    .borrow_mut()
                    .file_path_received_successfully = Some(path);

                Self::transmit_success_main();

                Ok(())
            }),
            Self::cancel_timeout_future(TIMEOUT_MS),
        )
        .await??;

        Ok(())
    }

    async fn transmit_send(
        &self,
        path: PathBuf,
        app_cfg: AppConfig<AppVersion>,
    ) -> Result<(), AppError> {
        self.prepare_transmit(TransferDirection::Send)?;
        self.set_ui_state(UIState::RequestCode);

        let window = WarpApplicationWindow::default();

        let (mut file, path, filename) = self.prepare_and_open_file(&path).await?;
        self.imp().context.borrow_mut().file_name = Some(filename.clone());
        let code_length = window.config().code_length_or_default();

        let res = spawn_async(cancelable_future(
            Wormhole::connect_without_code(app_cfg.clone(), code_length),
            Self::cancel_future(),
        ))
        .await?;

        let (welcome, connection) = match res {
            Ok(tuple) => tuple,
            Err(err) => {
                return Err(err.into());
            }
        };

        window.add_code(&welcome.code);
        let uri = WormholeTransferURI::from_app_cfg_with_code_direction(
            &app_cfg,
            &welcome.code,
            TransferDirection::Receive,
        );
        self.set_ui_state(UIState::HasCode(uri));

        let connection =
            spawn_async(cancelable_future(connection, Self::cancel_future())).await??;
        self.set_ui_state(UIState::Connected);

        self.imp().context.borrow_mut().file_path = Some(path);
        let transit_url = self.imp().context.borrow().transit_url.clone();

        cancelable_future(
            spawn_async(async move {
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
                    .context
                    .borrow()
                    .canceled
                {
                    return Err(AppError::Canceled);
                }

                Self::transmit_success_main();

                Ok(())
            }),
            Self::cancel_timeout_future(TIMEOUT_MS),
        )
        .await??;

        Ok(())
    }

    /// Wrapper to handle waiting on a channel that receives ()
    async fn receiver_future(receiver: async_channel::Receiver<()>) {
        let res = receiver.recv().await;
        match res {
            Ok(()) => {
                log::debug!("Canceled transfer");
            }
            Err(err) => {
                panic!("{:?}", err);
            }
        }
    }

    /// This future will finish when a message is received in the cancellation channel
    fn cancel_future() -> impl Future<Output = ()> {
        let obj = ActionView::default();
        let cancel_receiver = obj.imp().context.borrow().cancel_receiver.clone();
        Self::receiver_future(cancel_receiver)
    }

    /// This future is for any wormhole calls that have proper cancellation but no timeout handling
    ///
    /// We will wait until a cancellation event is received. Then we give the wormhole code
    /// `timeout_ms` milliseconds to properly respond. When there is no response the future will
    /// be finished
    fn cancel_timeout_future(timeout_ms: u64) -> impl Future<Output = ()> {
        let (sender, receiver) = async_channel::unbounded();
        async move {
            // Wait for a cancellation event
            Self::cancel_future().await;

            // Then do a timeout
            glib::timeout_add_once(Duration::from_millis(timeout_ms), move || {
                log::debug!("Cancellation timeout");
                if let Err(err) = sender.try_send(()) {
                    log::error!("Error when sending cancellation timeout message: {:?}", err);
                }
            });

            Self::receiver_future(receiver).await;
        }
    }

    /// Callback with information about the currently running transfer
    fn transit_handler(info: TransitInfo, peer_ip: SocketAddr) {
        glib::MainContext::default().invoke(move || {
            let obj = ActionView::default();
            let imp = obj.imp();

            let filename = imp
                .context
                .borrow()
                .file_name
                .as_ref()
                .map_or_else(|| "?".to_owned(), |s| s.to_string_lossy().to_string());

            obj.set_ui_state(UIState::Transmitting(filename, info, peer_ip));
        });
    }

    /// Handles progress information updates
    fn progress_handler(sent: u64, total: u64) {
        glib::MainContext::default().invoke(move || {
            let obj = ActionView::default();
            let imp = obj.imp();

            if imp.context.borrow().progress.is_none() {
                imp.context.borrow_mut().progress =
                    Some(FileTransferProgress::begin(total as usize));
            }

            let mut update_progress = false;
            let progress_str = imp
                .context
                .borrow_mut()
                .progress
                .as_mut()
                .and_then(|progress| {
                    update_progress = progress.set_progress(sent as usize);
                    progress.get_pretty_time_remaining()
                })
                .unwrap_or_else(|| "".to_owned());

            if update_progress {
                imp.progress_bar.set_fraction(sent as f64 / total as f64);
                imp.progress_bar.set_text(Some(&progress_str));
            }
        });
    }

    async fn ask_confirmation_future(&self) -> Result<(), AppError> {
        let continue_receiver = self.imp().context.borrow().continue_receiver.clone();
        cancelable_future(continue_receiver.recv(), Self::cancel_future()).await??;
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

    fn no_registered_application_error_dialog(msg: &str) -> gtk::MessageDialog {
        let dialog = gtk::builders::MessageDialogBuilder::new()
            // Translators: File receive confirmation message dialog title
            .text(&gettext("Unable to Open File"))
            .secondary_text(msg)
            .message_type(gtk::MessageType::Question)
            .buttons(gtk::ButtonsType::None)
            .transient_for(&WarpApplicationWindow::default())
            .modal(true)
            .build();
        let _close_button = dialog.add_button(&gettext("Close"), ResponseType::Close);
        let _open_in_dir_button = dialog.add_button(&gettext("Show in Folder"), ResponseType::Ok);
        dialog
    }

    /// Any post-transfer cleanup operations that are shared between success and failure states
    pub fn transmit_cleanup(&self) {
        log::debug!("Transmit cleanup");
        WarpApplication::default().uninhibit_transfer();

        if self.imp().context.borrow().canceled {
            // Send the cancellation complete message
            if let Err(err) = self
                .imp()
                .context
                .borrow()
                .cancellation_complete_sender
                .try_send(())
            {
                log::error!("Error sending cancellation complete message: {:?}", err);
            }
        }
    }

    /// Resets the view to be ready for the next transfer
    pub fn reset(&self) {
        log::debug!("Reset");
        let imp = self.imp();
        self.show_progress_indeterminate(false);

        imp.context.replace(UIContext::default());

        // Deletes any temporary files if required
        imp.context.borrow_mut().file_path = None;
    }

    fn transmit_success_main() {
        glib::MainContext::default().invoke(move || {
            WarpApplicationWindow::default()
                .action_view()
                .transmit_success();
        });
    }

    fn transmit_success(&self) {
        log::debug!("Transmit success");

        self.imp().progress_bar.set_fraction(1.0);

        let file_name = self
            .imp()
            .context
            .borrow()
            .file_name
            .clone()
            .unwrap_or_else(|| OsString::from("?"));

        self.set_ui_state(UIState::Done(file_name));

        self.transmit_cleanup();
    }

    pub fn transmit_error(&self, error: AppError) {
        log::debug!("Transmit error");

        if *self.ui_state() == UIState::Initial {
            error.handle();
        } else if !matches!(error, AppError::Canceled) {
            self.set_ui_state(UIState::Error(error));
        }

        self.transmit_cleanup();
    }

    pub fn transmit_error_handler(error: AppError) {
        WarpApplicationWindow::default()
            .action_view()
            .transmit_error(error);
    }

    pub fn send_file(&self, path: PathBuf, app_cfg: AppConfig<AppVersion>) {
        log::info!("Sending file: {}", path.display());
        let obj = self.clone();

        main_async_local(Self::transmit_error_handler, async move {
            obj.transmit_send(path, app_cfg).await?;
            Ok(())
        });
    }

    fn receive_file_impl(
        &self,
        code: Code,
        app_cfg: AppConfig<AppVersion>,
    ) -> Result<(), AppError> {
        let path = if let Some(downloads) = glib::user_special_dir(glib::UserDirectory::Downloads) {
            downloads
        } else {
            return Err(UiError::new(&gettext(
                "Downloads dir missing. Please set XDG_DOWNLOAD_DIR",
            ))
            .into());
        };

        let obj = self.clone();

        main_async_local(Self::transmit_error_handler, async move {
            obj.transmit_receive(path, code, app_cfg).await?;
            Ok(())
        });

        Ok(())
    }

    pub fn receive_file(&self, code: Code, app_cfg: AppConfig<AppVersion>) {
        log::info!("Receiving file with code '{}'", code);
        if let Err(err) = self.receive_file_impl(code, app_cfg) {
            self.transmit_error(err);
        }
    }

    pub fn transfer_in_progress(&self) -> bool {
        !self.imp().context.borrow().canceled
            && !matches!(&*self.ui_state(), UIState::Done(..) | UIState::Error(..))
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
