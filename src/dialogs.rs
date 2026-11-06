//! The native file pickers and message boxes.

use crate::{error::Result, page::Page};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
use tokio::fs::{read_dir, read_to_string};

pub async fn select_epub() -> Option<(PathBuf, Vec<u8>)> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select epub")
        .add_filter("epub", &["epub"])
        .pick_file()
        .await?;
    let content = handle.read().await;
    Some((handle.path().to_path_buf(), content))
}

pub async fn select_glossary() -> Option<(PathBuf, Vec<u8>)> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select glossary")
        .pick_file()
        .await?;
    let content = handle.read().await;
    Some((handle.path().to_path_buf(), content))
}

pub async fn select_format_folder(dir: PathBuf) -> Option<(String, Vec<(PathBuf, String)>)> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select translated folder")
        .set_directory(dir)
        .pick_folder()
        .await?;

    let mut dirs = read_dir(handle.path()).await.ok()?;
    let mut pages = Vec::new();
    while let Ok(Some(entry)) = dirs.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == OsStr::new("md")) {
            let content = read_to_string(&path).await.ok()?;
            pages.push((path, content));
        }
    }

    Some((handle.file_name(), pages))
}

pub async fn load_recovery() -> Option<Vec<Page>> {
    let handle = rfd::AsyncFileDialog::new()
        .add_filter("json", &["json"])
        .set_title("recovery")
        .pick_file()
        .await?;
    let json = handle.read().await;
    let pages: Vec<Page> = serde_json::from_slice(&json).ok()?;
    Some(pages)
}

pub async fn pick_save_folder(folder_name: String) -> Option<PathBuf> {
    let folder_name = Path::new(&folder_name).file_prefix()?.to_string_lossy();
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .set_file_name(folder_name)
        .save_file()
        .await?;
    Some(handle.path().to_path_buf())
}

pub async fn save_file<T: AsRef<[u8]>>(file_name: impl Into<String>, content: T) -> Result<()> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save")
        .set_file_name(file_name)
        .save_file()
        .await;

    if let Some(handle) = handle {
        handle.write(content.as_ref()).await?
    }
    Ok(())
}

pub async fn save_epub(content: Vec<u8>, file_name: impl Into<String>) -> Result<()> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save epub")
        .set_file_name(file_name)
        .add_filter("epub", &["epub"])
        .save_file()
        .await;

    if let Some(handle) = handle {
        handle.write(&content).await?;
    }
    Ok(())
}

pub async fn complete_dialog(file_name: impl AsRef<str>) {
    let file_name = Path::new(file_name.as_ref())
        .file_prefix()
        .unwrap_or_default()
        .to_string_lossy();

    rfd::AsyncMessageDialog::new()
        .set_title("Translation Complete")
        .set_description(file_name)
        .set_buttons(rfd::MessageButtons::Ok)
        .show()
        .await;
}

pub async fn error_dialog(description: String) {
    _ = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_description(description)
        .set_buttons(rfd::MessageButtons::Ok)
        .set_title("error message")
        .show()
        .await;
}
