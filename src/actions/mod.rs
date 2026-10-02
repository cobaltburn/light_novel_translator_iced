use crate::{
    controller::{
        TOC_PAGE_STEM, has_translatable_text, html_to_marked_markdown, markdown_sections,
        toc_to_markdown,
    },
    error::Result,
    model::Page,
};
use rbook::Epub;
use std::{
    ffi::OsStr,
    io::Cursor,
    iter,
    path::{Path, PathBuf},
};
use tokio::fs::{self, read_dir, read_to_string};

pub mod consensus;
pub mod doc;
pub mod format;
pub mod server;
pub mod translation;

pub async fn pick_save_folder(file_name: String) -> Option<PathBuf> {
    let file_name = Path::new(&file_name).file_stem()?.to_str()?;
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .set_file_name(file_name)
        .save_file()
        .await?;
    Some(handle.path().to_path_buf())
}

pub async fn save_file(file_name: String, contents: String) -> Result<()> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .set_file_name(file_name)
        .save_file()
        .await;

    if let Some(handle) = handle {
        handle.write(contents.as_bytes()).await?
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

pub async fn get_pages(file_path: PathBuf, buffer: Vec<u8>) -> Result<(PathBuf, Vec<Page>)> {
    let epub = Epub::read(Cursor::new(buffer))?;
    let toc = toc_to_markdown(epub.toc()).ok().and_then(|markdown| {
        let path = PathBuf::from(TOC_PAGE_STEM).with_extension("xhtml");
        (!markdown.is_empty()).then_some(Page::new(path, vec![markdown]))
    });

    let pages: Result<Vec<_>> = epub
        .spine()
        .iter()
        .filter_map(|e| e.manifest_entry())
        .map(|entry| {
            let html = entry.read_str()?;
            let path = entry.href().as_str().into();
            Ok((path, html_to_marked_markdown(&html)?))
        })
        .map(|result| {
            result.map(|(path, markdown)| {
                // Image-only pages are left untranslated so the build uses the original file
                let sections = if has_translatable_text(&markdown) {
                    markdown_sections(&markdown)
                } else {
                    Vec::new()
                };
                Page::new(path, sections)
            })
        })
        .collect();

    let pages: Vec<_> = iter::once(toc)
        .flatten()
        .chain(pages?)
        .filter(|p| !p.sections.is_empty())
        .collect();

    Ok((file_path, pages))
}

pub async fn complete_dialog(file_name: String) {
    let file_name = Path::new(&file_name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();

    rfd::AsyncMessageDialog::new()
        .set_title("Translation Complete")
        .set_description(file_name)
        .set_buttons(rfd::MessageButtons::Ok)
        .show()
        .await;
}

pub async fn select_format_folder(
    dir: impl AsRef<Path>,
) -> Option<(String, Vec<(PathBuf, String)>)> {
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
        c.is_alphabetic()
            && matches!(c,
                '\u{3040}'..='\u{309F}' |  // Hiragana
                '\u{30A0}'..='\u{30FF}' |  // Katakana
                '\u{4E00}'..='\u{9FFF}' |  // CJK Unified Ideographs (common kanji)
                '\u{3400}'..='\u{4DBF}' |  // CJK Unified Ideographs Extension A
                '\u{FF65}'..='\u{FF9F}' |  // Half-width Katakana
                '\u{31F0}'..='\u{31FF}'    // Katakana Phonetic Extensions
            )
    })
}

pub fn is_japanese_char(ch: &char) -> bool {
    ch.is_alphabetic()
        && matches!(ch,
            '\u{3040}'..='\u{309F}' |  // Hiragana
            '\u{30A0}'..='\u{30FF}' |  // Katakana
            '\u{4E00}'..='\u{9FFF}' |  // CJK Unified Ideographs (common kanji)
            '\u{3400}'..='\u{4DBF}' |  // CJK Unified Ideographs Extension A
            '\u{FF65}'..='\u{FF9F}' |  // Half-width Katakana
            '\u{31F0}'..='\u{31FF}'    // Katakana Phonetic Extensions
        )
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
