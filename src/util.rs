use custom_error::custom_error;
use gtk::prelude::*;
use gtk::{gio, glib};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};

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
    PY {source: pyo3::PyErr} = "Python error: {source}",
    UI {source: UIError} = "{source}",
}

// Don't show more than one error dialog at the same time, it will get very annoying
static ERROR_DIALOG_ALREADY_SHOWING: AtomicBool = AtomicBool::new(false);

impl AppError {
    pub fn handle(&self) {
        log::error!("{}", self);

        if gtk::is_initialized() {
            let window = match gio::Application::default() {
                Some(app) => match app.downcast::<gtk::Application>() {
                    Ok(app) => {
                        let windows = app.windows();
                        windows.get(0).cloned()
                    }
                    Err(_) => None,
                },
                _ => None,
            };

            let msg1 = "An error occurred";
            let msg2 = format!("{}", self);

            if let Some(window) = window {
                if window.is_visible() {
                    let res = ERROR_DIALOG_ALREADY_SHOWING.compare_exchange(
                        false,
                        true,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    );

                    if res.is_ok() {
                        let dialog = gtk::builders::MessageDialogBuilder::new()
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
}

pub fn do_async_local<F>(func: F)
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

pub fn do_async<F>(func: F)
where
    F: Future<Output = Result<(), AppError>> + Send + 'static,
{
    glib::MainContext::default().spawn(async move {
        match func.await {
            Ok(()) => (),
            Err(app_error) => app_error.handle(),
        }
    });
}
