use crate::gettext::gettextf;
use crate::ui::window::WarpApplicationWindow;
use custom_error::custom_error;
use futures::FutureExt;
use futures::{pin_mut, select};
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gio, glib, MessageType};
use std::future::Future;
use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, Ordering};
use wormhole::transfer::TransferError;
use wormhole::WormholeError;

custom_error! {pub UIError {msg: String} = "{msg}"}

#[allow(dead_code)]
impl UIError {
    pub fn new(msg: &str) -> Self {
        Self {
            msg: msg.to_string(),
        }
    }

    pub fn handle(self) {
        AppError::from(self).handle();
    }
}

custom_error! {pub AppError
    Canceled = "canceled",
    IO {source: std::io::Error} = "{source}",
    URL {source: url::ParseError} = "{source}",
    TRANSFER {source: TransferError} = "{source}",
    WORMHOLE {source: WormholeError} = "{source}",
    UI {source: UIError} = "{source}",
}

// Don't show more than one error dialog at the same time, it will get very annoying
static ERROR_DIALOG_ALREADY_SHOWING: AtomicBool = AtomicBool::new(false);

impl AppError {
    pub fn handle(&self) {
        if let AppError::Canceled = self {
            // Don't do anything here, the user canceled the operation
            return;
        }

        log::error!("{:?}", self);

        if gtk::is_initialized() {
            let window: Option<WarpApplicationWindow> = match gio::Application::default() {
                Some(app) => match app.downcast::<gtk::Application>() {
                    Ok(app) => {
                        let windows = app.windows();
                        windows
                            .get(0)
                            .cloned()
                            .and_then(|window| window.downcast().ok())
                    }
                    Err(_) => None,
                },
                _ => None,
            };

            let msg1 = "An error occurred";
            let msg2 = self.gettext_error();

            if let Some(window) = window {
                if window.is_visible() {
                    window.cancel_any_action();

                    let res = ERROR_DIALOG_ALREADY_SHOWING.compare_exchange(
                        false,
                        true,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    );

                    if res.is_ok() {
                        let dialog = gtk::builders::MessageDialogBuilder::new()
                            .message_type(MessageType::Error)
                            .text(msg1)
                            .secondary_text(&msg2)
                            .buttons(gtk::ButtonsType::Close)
                            .transient_for(&window)
                            .modal(true)
                            .build();

                        dialog.run_async(|obj, _| {
                            obj.close();
                            ERROR_DIALOG_ALREADY_SHOWING.store(false, Ordering::SeqCst);
                        });
                    }

                    return; // DON'T PANIC
                }
            }
        }

        // We can just panic if we can't display a proper error in the UI
        panic!(
            "An error occurred during application initialisation: {}",
            self
        );
    }

    fn gettext_error_wormhole(wormhole_error: &WormholeError) -> String {
        match wormhole_error {
            WormholeError::ProtocolJson(_) | WormholeError::Protocol(_) => {
                gettext("Corrupt or unexpected message received")
            }
            WormholeError::ServerError(_) => gettext("Error connecting to the rendezvous server.\nPlease check your network connection."),
            WormholeError::PakeFailed => gettext(
                "Encryption key confirmation failed. If you or your peer didn't mistype the code, this is a sign of an attacker guessing passwords. Please try again some time later.",
            ),
            WormholeError::Crypto => gettext("Cannot decrypt a received message"),
            _ => gettext("An unknown error occurred"),
        }
    }

    fn gettext_error_io(io_error: &std::io::Error) -> String {
        match io_error.kind() {
            ErrorKind::NotFound => gettext("File / Directory not found"),
            ErrorKind::PermissionDenied => gettext("Permission denied"),
            _ => io_error.to_string(),
        }
    }

    fn gettext_error(&self) -> String {
        match self {
            AppError::Canceled => "canceled".to_string(),
            AppError::IO { source } => Self::gettext_error_io(source),
            // TODO those should not appear publicly
            AppError::URL { source } => source.to_string(),
            AppError::TRANSFER { source } => match source {
                TransferError::AckError => gettext("Transfer was not acknowledged by peer"),
                TransferError::Checksum | TransferError::FilesystemSkew => {
                    gettext("The received file is corrupted")
                }
                TransferError::FileSize {
                    sent_size,
                    file_size,
                } => gettextf(
                    "The file contained a different amount of bytes than advertised! Sent {} \
                    bytes, but should have been {}",
                    &[sent_size, &file_size],
                ),
                TransferError::PeerError(msg) => {
                    if msg == "Task has been cancelled" {
                        gettext("The other side has cancelled the transfer")
                    } else {
                        gettextf("Something went wrong on the other side: {}", &[msg])
                    }
                }
                TransferError::UnsupportedOffer
                | TransferError::ProtocolJson(_)
                | TransferError::ProtocolMsgpack(_)
                | TransferError::Protocol(_)
                | TransferError::ProtocolUnexpectedMessage(_, _) => {
                    gettext("Corrupt or unexpected message received")
                }
                TransferError::Wormhole(source) => Self::gettext_error_wormhole(source),
                TransferError::TransitConnect(_) => {
                    gettext("Error while establishing file transfer connection")
                }
                TransferError::Transit(_) => gettext("Unknown file transfer error"),
                TransferError::IO(source) => Self::gettext_error_io(source),
                _ => gettext("An unknown error occurred"),
            },
            AppError::WORMHOLE { source } => Self::gettext_error_wormhole(source),
            // UIErrors are generated our code and already wrapped in gettext
            AppError::UI { source } => source.to_string(),
        }
    }
}

pub fn spawn_async<F>(func: F)
where
    F: Future<Output = Result<(), AppError>> + 'static + Send,
{
    smol::spawn(async move {
        match func.await {
            Ok(()) => (),
            Err(app_error) => app_error.handle(),
        }
    })
    .detach();
}

pub fn do_async<F>(func: F)
where
    F: Future<Output = Result<(), AppError>> + 'static,
{
    glib::MainContext::default().spawn_local(async move {
        match func.await {
            Ok(()) => (),
            Err(app_error) => app_error.handle(),
        }
    });
}

pub async fn cancelable_future<T>(
    future: impl Future<Output = T>,
    cancel_future: impl Future<Output = ()>,
) -> Result<T, AppError> {
    let future = future.fuse();
    let cancel_future = cancel_future.fuse();

    pin_mut!(future, cancel_future);

    select! {
        res = future => {
            Ok(res)
        },
        () = cancel_future => {
            drop(future);
            log::debug!("Future canceled");
            Err(AppError::Canceled)
        }
    }
}
