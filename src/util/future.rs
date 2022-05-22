use crate::globals;
use crate::util::error::AppError;
use futures::{pin_mut, select, FutureExt};
use gtk::glib;
use std::future::Future;

pub async fn spawn_async<F>(func: F) -> Result<(), AppError>
where
    F: Future<Output = Result<(), AppError>> + 'static + Send,
{
    let task = smol::spawn(async move { func.await });
    task.catch_unwind().await.map_err(|any| {
        let mut msg = if let Some(msg) = any.downcast_ref::<&str>() {
            msg.to_string()
        } else if let Some(msg) = any.downcast_ref::<String>() {
            msg.to_string()
        } else {
            "Unknown panic cause".to_string()
        };

        let backtrace = globals::PANIC_BACKTRACE.with(|b| b.take());

        if let Some(mut backtrace) = backtrace {
            msg = format!("{}\n{:?}", msg, backtrace);
        }

        AppError::Panic { msg }
    })?
}

pub fn main_async_local<F, E>(error_handler: E, func: F)
where
    F: Future<Output = Result<(), AppError>> + 'static,
    E: FnOnce(AppError) + 'static,
{
    glib::MainContext::default().spawn_local(async move {
        match func.await {
            Ok(()) => (),
            Err(app_error) => error_handler(app_error),
        }
    });
}

pub fn main_async_local_infallible<F>(func: F)
where
    F: Future<Output = ()> + 'static,
{
    glib::MainContext::default().spawn_local(func);
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
