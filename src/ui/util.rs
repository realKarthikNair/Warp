use async_std::fs::OpenOptions;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub async fn open_file_find_new_filename_if_exists(
    path: &Path,
) -> (std::io::Result<async_std::fs::File>, PathBuf) {
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
        file_res = OpenOptions::new()
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
