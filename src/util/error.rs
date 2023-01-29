use crate::gettext::gettextf;
use crate::globals;
use crate::ui::window::WarpApplicationWindow;
use adw::prelude::*;
use gettextrs::gettext;
use std::fmt::{Debug, Display, Formatter, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use wormhole::rendezvous::RendezvousError;
use wormhole::transfer::TransferError;
use wormhole::transit::RelayHintParseError;
use wormhole::WormholeError;
use zip::result::ZipError;

#[derive(thiserror::Error, Debug)]
pub struct UiError {
    msg: String,
}

#[allow(dead_code)]
impl UiError {
    pub fn new(msg: &impl ToString) -> Self {
        Self {
            msg: msg.to_string(),
        }
    }

    pub fn handle(self) {
        AppError::from(self).handle();
    }
}

impl Display for UiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    Canceled,
    Io {
        #[from]
        source: std::io::Error,
    },
    Url {
        #[from]
        source: url::ParseError,
    },
    Transfer {
        #[from]
        source: TransferError,
    },
    Wormhole {
        #[from]
        source: WormholeError,
    },
    WormholeRelayHintParseError {
        #[from]
        source: RelayHintParseError,
    },
    Ui {
        #[from]
        source: UiError,
    },
    AsyncChannelRecvError {
        #[from]
        source: async_channel::RecvError,
    },
    AsyncBroadcastRecvError {
        #[from]
        source: async_broadcast::RecvError,
    },
    Zip {
        #[from]
        source: ZipError,
    },
    Panic {
        msg: String,
    },
    Glib {
        #[from]
        source: glib::Error,
    },
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Canceled => write!(f, "Operation was canceled by the user"),
            AppError::Io { source } => write!(f, "I/O Error: {source}"),
            AppError::Url { source } => write!(f, "URL ParseError: {source}"),
            AppError::Transfer { source } => write!(f, "TransferError: {source:?}"),
            AppError::Wormhole { source } => write!(f, "WormholeError: {source:?}"),
            AppError::WormholeRelayHintParseError { source } => {
                write!(f, "WormholeRelayHintParseError: {source:?}")
            }
            AppError::Ui { source } => write!(f, "UiError: {source}"),
            AppError::AsyncChannelRecvError { source } => {
                write!(f, "AsyncChannelRecvError: {source}")
            }
            AppError::AsyncBroadcastRecvError { source } => {
                write!(f, "AsyncBroadcastRecvError: {source}")
            }
            AppError::Zip { source } => write!(f, "ZipError: {source}"),
            AppError::Panic { msg } => write!(f, "Panic: {msg}"),
            AppError::Glib { source } => write!(f, "Glib: {source}"),
        }
    }
}

// Don't show more than one error dialog at the same time, it will get very annoying
static ERROR_DIALOG_ALREADY_SHOWING: AtomicBool = AtomicBool::new(false);

impl AppError {
    pub fn handle(self) {
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

            if let Some(window) = window {
                if window.is_visible() {
                    self.show_error_dialog(&window);
                    return; // DON'T PANIC
                }
            }
        }

        // We can just panic if we can't display a proper error in the UI
        panic!("An error occurred during application initialisation: {self}",);
    }

    pub fn show_error_dialog(&self, window: &WarpApplicationWindow) {
        let msg1 = &gettext("An error occurred");
        let msg2 = self.gettext_error();

        let res = ERROR_DIALOG_ALREADY_SHOWING.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );

        if res.is_ok() {
            let dialog = adw::builders::MessageDialogBuilder::new()
                .heading(msg1)
                .body(&msg2)
                .close_response("close")
                .transient_for(window)
                .modal(true)
                .build();

            dialog.add_response("close", &gettext("_Close"));

            dialog.run_async(None, |obj, _| {
                obj.close();
                ERROR_DIALOG_ALREADY_SHOWING.store(false, Ordering::SeqCst);
            });
        }
    }

    fn gettext_error_wormhole(wormhole_error: &WormholeError) -> String {
        match wormhole_error {
            WormholeError::ProtocolJson(_) | WormholeError::Protocol(_) => {
                gettext("Corrupt or unexpected message received")
            }
            WormholeError::ServerError(err) => {
                if let RendezvousError::Server(msg) = err {
                    if &**msg == "crowded" {
                        gettext("The rendezvous server will not allow further connections for this code. A new code needs to be generated.")
                    } else if &**msg == "pruney" {
                        gettext("The rendezvous server removed the code due to inactivity. A new code needs to be generated.")
                    } else {
                        gettextf("The rendezvous server responded with an unknown message: {}", &[msg])
                    }
                } else if WarpApplicationWindow::default().config().rendezvous_server_url.is_some() {
                    gettext("Error connecting to the rendezvous server.\nYou have entered a custom rendezvous server URL in preferences. Please verify the URL is correct and the server is working.")
                } else {
                    gettext("Error connecting to the rendezvous server.\nPlease try again later / verify you are connected to the internet.")
                }
            },
            WormholeError::PakeFailed => gettext(
                "Encryption key confirmation failed. If you or your peer didn't mistype the code, this is a sign of an attacker guessing passwords. Please try again some time later.",
            ),
            WormholeError::Crypto => gettext("Cannot decrypt a received message"),
            _ => gettext("An unknown error occurred"),
        }
    }

    fn gettext_error_io(io_error: &std::io::Error) -> String {
        match io_error.kind() {
            std::io::ErrorKind::NotFound => gettext("File / Directory not found"),
            std::io::ErrorKind::PermissionDenied => gettext("Permission denied"),
            _ => io_error.to_string(),
        }
    }

    pub fn gettext_error(&self) -> String {
        match self {
            AppError::Canceled => "canceled".to_owned(),
            AppError::Io { source } => Self::gettext_error_io(source),
            // TODO those should not appear publicly
            AppError::Url { source } => source.to_string(),
            AppError::Transfer { source } => match source {
                TransferError::AckError => gettext("Transfer was not acknowledged by peer"),
                TransferError::Checksum | TransferError::FilesystemSkew => {
                    gettext("The received file is corrupted")
                }
                TransferError::FileSize {
                    sent_size,
                    file_size,
                } => gettextf(
                    "The file contained a different amount of bytes than advertised! Sent {} bytes, but should have been {}",
                    &[sent_size, &file_size],
                ),
                TransferError::PeerError(msg) => {
                    if msg == "Task has been cancelled" {
                        gettext("The other side has cancelled the transfer")
                    } else if msg == "transfer rejected" {
                        gettext("The other side has rejected the transfer")
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
                TransferError::Wormhole(source) => {
                    log::error!("Wormhole error: {:?}", source);
                    Self::gettext_error_wormhole(source)
                },
                TransferError::TransitConnect(_) => {
                    gettext("Error while establishing file transfer connection")
                }
                TransferError::Transit(_) => gettext("Unknown file transfer error"),
                TransferError::IO(source) => Self::gettext_error_io(source),
                _ => gettext("An unknown error occurred"),
            },
            AppError::Wormhole { source } => Self::gettext_error_wormhole(source),
            AppError::WormholeRelayHintParseError { source } => source.to_string(),
            // UIErrors are generated by our code and already wrapped in gettext
            AppError::Ui { source } => source.to_string(),
            AppError::AsyncChannelRecvError { .. } | AppError::AsyncBroadcastRecvError { .. } => gettext("An unknown error occurred"),
            AppError::Zip { source } => match source {
                ZipError::Io(err) => Self::gettext_error_io(err),
                _ => gettext("An unknown error occurred"),
            },
            AppError::Panic { .. } => gettext("An unexpected error occurred. Please report an issue with the error message."),
            AppError::Glib { source } => source.to_string(),
        }
    }
}

pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("<unnamed>");
        let location = panic_info.location().unwrap();
        let msg = match panic_info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<dyn Any>",
            },
        }
        .to_owned();

        let backtrace = backtrace::Backtrace::new();
        let info_msg = format!("thread '{name}' panicked at '{msg}', {location}\n{backtrace:?}");

        globals::PANIC_BACKTRACES.lock().unwrap().push(info_msg);
        default_hook(panic_info);
    }));
}

pub fn error_for_panic() -> AppError {
    let mut msg = String::new();

    let mut backtrace_info = globals::PANIC_BACKTRACES.lock().unwrap();
    for backtrace_msg in backtrace_info.iter() {
        let _ignored = writeln!(msg, "{backtrace_msg}");
    }

    backtrace_info.clear();

    if msg.is_empty() {
        msg = "Unknown panic cause".to_owned();
    }

    AppError::Panic { msg }
}
