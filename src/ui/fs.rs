use crate::globals;
use crate::util::error::AppError;
use futures::FutureExt;
use futures::{pin_mut, select};
use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use zip::ZipWriter;
use zip_extensions::write::ZipWriterExtensions;

pub async fn compress_folder_cancelable(
    path: &Path,
    cancel_future: impl Future<Output = ()>,
) -> Result<tempfile::NamedTempFile, AppError> {
    let tar_path_future = compress_folder(path);
    let tar_path_future = tar_path_future.fuse();
    let cancel_future = cancel_future.fuse();

    pin_mut!(tar_path_future, cancel_future);

    select! {
        res = tar_path_future => {
            if res.is_ok() {
                log::debug!("Created archive");
            }

            res
        },
        () = cancel_future => {
            log::debug!("Archive creation canceled");
            // Canceled / Error: We drop the smol::Task at the end of this function which aborts it
            // The dropped NamedTempFile will be deleted as well
            Err(AppError::Canceled)
        }
    }
}

pub async fn compress_folder(path: &Path) -> Result<tempfile::NamedTempFile, AppError> {
    let path = path.to_path_buf();
    assert!(path.is_dir(), "Wrong compress_folder invocation");

    let tmp_dir = &*globals::CACHE_DIR;
    std::fs::create_dir_all(tmp_dir)?;

    let mut zip_file = tempfile::Builder::new()
        .prefix("warp_archive_")
        .suffix(".zip")
        .tempfile_in(tmp_dir)?;

    log::debug!("Creating archive: {}", zip_file.path().display());
    smol::spawn(async move {
        let mut zip = ZipWriter::new(&mut zip_file);
        zip.create_from_directory(&path)?;
        drop(zip);
        Ok(zip_file)
    })
    .await
}

pub fn safe_persist_tempfile(
    temp_file: tempfile::NamedTempFile,
    filename: &Path,
) -> std::io::Result<PathBuf> {
    let mut temp_path = temp_file.into_temp_path();
    let mut file_stem: String = filename
        .file_stem()
        .unwrap_or(&OsString::new())
        .to_string_lossy()
        .into();
    if file_stem.is_empty() {
        file_stem = "Downloaded file".to_owned();
    }

    let orig_file_stem = file_stem.clone();

    let mut file_ext: String = filename
        .extension()
        .unwrap_or(&OsString::new())
        .to_string_lossy()
        .into();
    if file_ext.is_empty() {
        file_ext = "bin".to_owned();
    }

    let mut i = 1;
    let mut filename;
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
        let persist_res = temp_path.persist_noclobber(&path);
        if let Err(err) = persist_res {
            if err.error.kind() != std::io::ErrorKind::AlreadyExists {
                log::error!("Error creating file '{}': {}", path.display(), err);
                return Err(err.error);
            }

            file_stem = format!("{orig_file_stem} ({i})");
            i += 1;

            temp_path = err.path;
        } else {
            return Ok(path);
        }
    }
}
