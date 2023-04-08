use std::path::{Path, PathBuf};

use async_zip::{base::write::ZipFileWriter, Compression, ZipEntryBuilder};
use futures::{AsyncReadExt, AsyncWrite, TryStreamExt};
use smol::fs::File;

use super::error::{AppError, UiError};

// See https://github.com/Majored/rs-async-zip/blob/main/examples/cli_compress.rs
async fn handle_directory<W: AsyncWrite + Unpin>(
    input_path: &Path,
    writer: &mut ZipFileWriter<W>,
) -> Result<(), AppError> {
    let entries = walk_dir(input_path.into()).await?;
    let input_dir_str = input_path
        .as_os_str()
        .to_str()
        .ok_or(UiError::new(&format!(
            "Input path {input_path:#?} not valid UTF-8."
        )))?;

    for entry_path_buf in entries {
        let entry_path = entry_path_buf.as_path();
        let entry_str = entry_path
            .as_os_str()
            .to_str()
            .ok_or(UiError::new(&format!(
                "Directory file path {entry_path:#?} not valid UTF-8."
            )))?;

        if !entry_str.starts_with(input_dir_str) {
            return Err(UiError::new(&format!(
                "Directory file path {entry_str:#?} does not start with base input directory path."
            ))
            .into());
        }

        let entry_str = &entry_str[input_dir_str.len() + 1..];
        write_entry(entry_str, entry_path, writer).await?;
    }

    Ok(())
}

async fn write_entry<W: AsyncWrite + Unpin>(
    filename: &str,
    input_path: &Path,
    writer: &mut ZipFileWriter<W>,
) -> Result<(), AppError> {
    let mut input_file = File::open(input_path).await?;
    let input_file_size = input_file.metadata().await?.len() as usize;

    let mut buffer = Vec::with_capacity(input_file_size);
    input_file.read_to_end(&mut buffer).await?;

    let builder = ZipEntryBuilder::new(filename.into(), Compression::Stored);
    writer.write_entry_whole(builder, &buffer).await?;

    Ok(())
}

async fn walk_dir(dir: PathBuf) -> Result<Vec<PathBuf>, AppError> {
    let mut dirs = vec![dir];
    let mut files = vec![];

    while !dirs.is_empty() {
        let mut dir_iter = smol::fs::read_dir(dirs.remove(0)).await?;

        while let Some(entry) = dir_iter.try_next().await? {
            let entry_path_buf = entry.path();

            if entry_path_buf.is_dir() {
                dirs.push(entry_path_buf);
            } else {
                files.push(entry_path_buf);
            }
        }
    }

    Ok(files)
}

pub async fn zip_dir<W: AsyncWrite + Unpin>(dir: &Path, writer: W) -> Result<(), AppError> {
    let mut output_writer = ZipFileWriter::new(writer);

    if !dir.is_dir() {
        return Err(UiError::new(&"Directory expected").into());
    }

    handle_directory(dir, &mut output_writer).await?;

    output_writer.close().await?;

    Ok(())
}
