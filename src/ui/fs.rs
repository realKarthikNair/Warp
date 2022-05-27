use crate::gettext::gettextf;
use crate::util::error::{AppError, UiError};
use futures::FutureExt;
use futures::{pin_mut, select};
use gtk::glib;
use smol::process::Command;
use std::ffi::OsString;
use std::future::Future;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub async fn compress_folder_cancelable(
    path: &Path,
    cancel_future: impl Future<Output = ()>,
) -> Result<tempfile::TempPath, AppError> {
    let tar_path_future = compress_folder(path)?;
    let tar_path_future = tar_path_future.fuse();
    let cancel_future = cancel_future.fuse();

    pin_mut!(tar_path_future, cancel_future);

    select! {
        res = tar_path_future => {
            if res.is_ok() {
                log::debug!("Created tar archive");
            }

            res
        },
        () = cancel_future => {
            log::debug!("Tar creation canceled");
            // Canceled / Error: We drop the smol::Task at the end of this function which aborts it
            // The dropped TempPath will be deleted as well
            Err(AppError::Canceled)
        }
    }
}

pub fn compress_folder(
    path: &Path,
) -> Result<impl Future<Output = Result<tempfile::TempPath, AppError>>, AppError> {
    let path = path.to_path_buf();
    if !path.is_dir() {
        panic!("Wrong compress_folder invocation");
    }

    let tmp_dir = glib::tmp_dir();
    let outer_dir = path.parent().unwrap_or(&tmp_dir);

    let dirname = path.file_name();
    if let Some(dirname) = dirname {
        let temp_dir = glib::tmp_dir();
        let tar_path = tempfile::NamedTempFile::new_in(temp_dir)?.into_temp_path();

        let mut command = Command::new("tar");
        command
            .arg("-C")
            .arg(outer_dir)
            .arg("-czf")
            .arg(tar_path.as_os_str())
            .arg(dirname)
            .kill_on_drop(true);

        log::debug!("Creating tar archive: {}", tar_path.to_string_lossy());
        let future = async move {
            let res = command.spawn()?.status().await?;
            if let Some(code) = res.code() {
                if code == 0 {
                    Ok(tar_path)
                } else {
                    Err(UiError::new("Error creating tar archive").into())
                }
            } else {
                Err(UiError::new("Error creating tar archive").into())
            }
        };

        Ok(future)
    } else {
        Err(UiError::new(&gettextf(
            "Path {} does not have a directory name",
            &[&path.display()],
        ))
        .into())
    }
}

pub async fn safe_persist_tempfile(
    temp_path: tempfile::TempPath,
    filename: &Path,
) -> std::io::Result<PathBuf> {
    let mut file_stem: String = filename
        .file_stem()
        .unwrap_or(&OsString::new())
        .to_string_lossy()
        .into();
    if file_stem.is_empty() {
        file_stem = "Downloaded file".to_string();
    }

    let orig_file_stem = file_stem.clone();

    let mut file_ext: String = filename
        .extension()
        .unwrap_or(&OsString::new())
        .to_string_lossy()
        .into();
    if file_ext.is_empty() {
        file_ext = "bin".to_string();
    }

    let mut i = 1;
    let mut filename;
    let mut file_res;
    let dir = temp_path
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .to_path_buf();
    let mut path;

    loop {
        let mut filename_str = file_stem.clone();
        filename_str.push('.');
        filename_str.push_str(&file_ext);
        filename = PathBuf::from(filename_str);

        path = dir.join(filename.clone());
        file_res = smol::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .truncate(true)
            .open(&path)
            .await;
        if let Err(err) = &file_res {
            if err.kind() != ErrorKind::AlreadyExists {
                break;
            }

            file_stem = format!("{} ({})", orig_file_stem, i);
            i += 1;
        } else {
            break;
        }
    }

    temp_path.persist(&path)?;

    Ok(path)
}
