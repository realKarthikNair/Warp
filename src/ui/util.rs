use crate::gettext::gettextf;
use crate::util::{AppError, UIError};
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
) -> Result<Option<PathBuf>, AppError> {
    let (tar_path_future, tar_path) = compress_folder(path).await?;
    let tar_path_future = tar_path_future.fuse();
    let cancel_future = cancel_future.fuse();

    pin_mut!(tar_path_future, cancel_future);

    let cancel_or_tar = select! {
        res = tar_path_future => {
            log::debug!("Created tar archive");
            Some(res)
        },
        () = cancel_future => {
            log::debug!("Tar creation canceled");
            None
        }
    };

    if let Some(tar_res) = cancel_or_tar {
        match tar_res {
            Ok(()) => Ok(Some(tar_path)),
            Err(err) => Err(err),
        }
    } else {
        // Canceled. We drop the smol::Task here which aborts it
        drop(tar_path_future);
        // Remove file if it already exists
        log::debug!(
            "Removing partially created tar archive: {}",
            tar_path.display()
        );
        let _ignore = smol::fs::remove_file(tar_path).await;
        Ok(None)
    }
}

pub async fn compress_folder(
    path: &Path,
) -> Result<(impl Future<Output = Result<(), AppError>>, PathBuf), AppError> {
    let path = path.to_path_buf();
    if !path.is_dir() {
        return Err(UIError::new("Wrong compress_folder invocation").into());
    }

    let outer_dir = path
        .parent()
        .ok_or_else(|| UIError::new("Archive parent folder not found"))?;
    let dirname = path.file_name();
    if let Some(dirname) = dirname {
        let temp_dir = glib::tmp_dir();
        let mut tar_name = dirname.to_owned();
        tar_name.push(".tgz");

        // We don't use set_extension here because it would remove any .something from dir name
        let tar_path = temp_dir.join(PathBuf::from(tar_name));

        let (_file, name) = open_file_find_new_filename_if_exists(&tar_path).await;
        let tar_path = name;

        let mut command = Command::new("tar");
        command
            .arg("-C")
            .arg(outer_dir)
            .arg("-czf")
            .arg(tar_path.as_os_str())
            .arg(dirname)
            .kill_on_drop(true);
        let future = async move {
            let res = command.spawn()?.status().await?;
            if let Some(code) = res.code() {
                if code == 0 {
                    Ok(())
                } else {
                    Err(UIError::new("Error creating tar archive").into())
                }
            } else {
                Err(UIError::new("Error creating tar archive").into())
            }
        };

        log::debug!("Creating tar archive: {}", tar_path.to_string_lossy());
        Ok((future, tar_path))
    } else {
        Err(UIError::new(&gettextf(
            "Path {} does not have a directory name",
            &[&path.display()],
        ))
        .into())
    }
}

pub async fn open_file_find_new_filename_if_exists(
    path: &Path,
) -> (std::io::Result<smol::fs::File>, PathBuf) {
    let mut file_stem: String = path
        .file_stem()
        .unwrap_or(&OsString::new())
        .to_string_lossy()
        .into();
    if file_stem.is_empty() {
        file_stem = "Downloaded file".to_string();
    }

    let orig_file_stem = file_stem.clone();

    let mut file_ext: String = path
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
    let dir = path.parent().unwrap_or(&PathBuf::from(".")).to_path_buf();
    let mut path;

    loop {
        filename = PathBuf::from(file_stem.clone());
        filename.set_extension(file_ext.clone());

        path = dir.join(filename);
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

    (file_res, path)
}
