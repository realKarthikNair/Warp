use crate::util::error::AppError;
use futures::{pin_mut, select, FutureExt};
use gtk::glib;
use std::future::Future;

pub fn spawn_async<F, E>(error_handler: E, func: F)
where
    F: Future<Output = Result<(), AppError>> + 'static + Send,
    E: FnOnce(AppError) + 'static + Send,
{
    smol::spawn(async move {
        match func.await {
            Ok(()) => (),
            Err(app_error) => {
                main_async(error_handler, async move { Err(app_error) });
            }
        }
    })
    .detach();
}

pub fn spawn_async_infallible<F>(func: F)
where
    F: Future<Output = ()> + 'static + Send,
{
    smol::spawn(func).detach();
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

pub fn main_async<F, E>(error_handler: E, func: F)
where
    F: Future<Output = Result<(), AppError>> + Send + 'static,
    E: FnOnce(AppError) + Send + 'static,
{
    glib::MainContext::default().spawn(async move {
        match func.await {
            Ok(()) => (),
            Err(app_error) => error_handler(app_error),
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
