use crate::globals;
use crate::util::error::AppError;
use futures::FutureExt;
use futures::{pin_mut, select};
use std::ffi::OsString;
use std::future::Future;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use zip::ZipWriter;
use zip_extensions::write::ZipWriterExtensions;

pub async fn compress_folder_cancelable(
    path: &Path,
    cancel_future: impl Future<Output = ()>,
) -> Result<tempfile::TempPath, AppError> {
    let tar_path_future = compress_folder(path);
    let tar_path_future = tar_path_future.fuse();
    let cancel_future = cancel_future.fuse();

    pin_mut!(tar_path_future, cancel_future);

    select! {
        res = tar_path_future => {
            if res.is_ok() {
                log::debug!("Created archive");
            }

            res.map(|f| f.into_temp_path())
        },
        () = cancel_future => {
            log::debug!("Archive creation canceled");
            // Canceled / Error: We drop the smol::Task at the end of this function which aborts it
            // The dropped TempPath will be deleted as well
            Err(AppError::Canceled)
        }
    }
}

pub async fn compress_folder(path: &Path) -> Result<tempfile::NamedTempFile, AppError> {
    let path = path.to_path_buf();
    if !path.is_dir() {
        panic!("Wrong compress_folder invocation");
    }

    let tmp_dir = &*globals::CACHE_DIR;
    std::fs::create_dir_all(tmp_dir)?;

    let mut zip_file = tempfile::Builder::new()
        .prefix("warp_archive_")
        .suffix(".zip")
        .tempfile_in(&tmp_dir)?;

    log::debug!("Creating archive: {}", zip_file.path().display());
    smol::spawn(async move {
        let mut zip = ZipWriter::new(&mut zip_file);
        zip.create_from_directory(&path)?;
        drop(zip);
        Ok(zip_file)
    })
    .await
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
