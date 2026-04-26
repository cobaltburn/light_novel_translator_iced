use crate::{
    controller::{get_ordered_path, parse::partition_text, xml::strip_data_tags},
    error::Result,
    model::page::Page,
};
use epub::doc::EpubDoc;
use htmd::HtmlToMarkdown;
use std::{
    ffi::OsStr,
    io::Cursor,
    path::{Path, PathBuf},
};
use tokio::fs::{self, read_dir, read_to_string};

pub mod consensus_action;
pub mod doc_action;
pub mod extraction_action;
pub mod format_action;
pub mod server_action;
pub mod trans_action;

pub async fn pick_save_folder(file_name: String) -> Option<PathBuf> {
    let file_name = Path::new(&file_name).file_stem()?.to_str()?;
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .set_file_name(file_name)
        .save_file()
        .await?;
    Some(handle.path().to_path_buf())
}

pub async fn save_file(file_name: String, content: String) -> Result<()> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .set_file_name(file_name)
        .save_file()
        .await;

    if let Some(handle) = handle {
        handle.write(content.as_bytes()).await?
    }
    Ok(())
}

pub async fn load_markdown_folder() -> Option<Vec<(PathBuf, String)>> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("load folder")
        .pick_folder()
        .await?;

    let mut dirs = fs::read_dir(handle.path()).await.ok()?;
    let mut pages = Vec::new();
    while let Ok(Some(entry)) = dirs.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|x| x == "md") {
            let content = fs::read_to_string(&path).await.ok()?;
            pages.push((path, content));
        }
    }

    Some(pages)
}

pub async fn get_pages(file_path: PathBuf, buffer: Vec<u8>) -> Result<(PathBuf, Vec<Page>)> {
    let mut epub = EpubDoc::from_reader(Cursor::new(buffer))?;

    let paths = get_ordered_path(&epub);
    let converter = HtmlToMarkdown::builder()
        .skip_tags(vec!["head", "img", "image"])
        .scripting_enabled(false)
        .build();

    let pages: Result<Vec<_>> = paths
        .into_iter()
        .map(|path| {
            let html = epub.get_resource_str_by_path(&path).unwrap();
            let html = strip_data_tags(&html)?;
            let markdown = converter.convert(&html)?;
            let markdown: Vec<_> = markdown.lines().map(|s| s.trim()).collect();
            Ok((path, markdown.join("\n")))
        })
        .map(|result| {
            result.map(|(path, markdown)| {
                let partitioned = partition_text(&markdown);
                let sections = partitioned
                    .chunks(3)
                    .map(|x| x.join(" "))
                    .filter(|e| !e.trim_matches('#').is_empty())
                    .collect();
                Page::new(path, sections)
            })
        })
        .collect();

    let pages: Vec<_> = pages?
        .into_iter()
        .filter(|p| !p.sections.is_empty())
        .collect();

    Ok((file_path, pages))
}

pub async fn complete_dialog(file_name: String) -> bool {
    let file_name = Path::new(&file_name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();

    let dialog = rfd::AsyncMessageDialog::new()
        .set_title("Translation Complete")
        .set_description(format!("Save: {}", file_name))
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        .await;

    matches!(dialog, rfd::MessageDialogResult::Yes)
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

pub fn contains_japanese(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{3040}'..='\u{309F}' |  // Hiragana
            '\u{30A0}'..='\u{30FF}' |  // Katakana
            '\u{4E00}'..='\u{9FFF}' |  // CJK Unified Ideographs (common kanji)
            '\u{3400}'..='\u{4DBF}' |  // CJK Unified Ideographs Extension A
            '\u{FF65}'..='\u{FF9F}' |  // Half-width Katakana
            '\u{31F0}'..='\u{31FF}'    // Katakana Phonetic Extensions
        )
    })
}

pub fn clean_invisible_chars(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            // Keep normal whitespace
            if c == ' ' || c == '\n' || c == '\r' || c == '\t' {
                return true;
            }

            // Remove zero-width and invisible characters
            if matches!(c,
                '\u{200B}'..='\u{200F}' |
                '\u{2060}'..='\u{2064}' |
                '\u{FEFF}' |
                '\u{00AD}' |
                '\u{FFA0}'
            ) {
                return false;
            }

            // Remove other control characters
            !c.is_control()
        })
        .collect()
}
