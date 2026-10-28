use crate::{
    controller::{
        TOC_PAGE_PREFIX, has_translatable_text, html_to_marked_markdown, markdown_sections,
        part_tag, remove_think_tags, toc_to_markdown,
    },
    error::{Error, Result, TaskResultExt},
    model::Page,
};
use iced::{Task, advanced::graphics::futures::MaybeSend, futures::future::join_all};
use rbook::Epub;
use std::{
    collections::BTreeMap,
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

pub async fn pick_save_folder(folder_name: String) -> Option<PathBuf> {
    let folder_name = Path::new(&folder_name).file_prefix()?.to_string_lossy();
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .set_file_name(folder_name)
        .save_file()
        .await?;
    Some(handle.path().to_path_buf())
}

/// The page's translated sections, each preceded by its part tag.
pub fn page_markdown(page: &Page) -> String {
    let text: String = page
        .sections
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}{}\n", part_tag(i + 1), s.content))
        .collect();
    remove_think_tags(&text)
}

/// Writes every page to `folder` as markdown concurrently, showing a single
/// error listing every page that failed to save.
pub fn save_pages<T: Clone + MaybeSend + 'static>(pages: &[Page], folder: &Path) -> Task<T> {
    match page_files(pages, folder) {
        Ok(files) => Task::future(write_files(files)).ok_or_display(Into::into),
        Err(error) => error.display_error(),
    }
}

fn page_files(pages: &[Page], folder: &Path) -> Result<Vec<(PathBuf, String)>> {
    let files = pages
        .iter()
        .map(|page| {
            let name = page
                .path
                .file_name()
                .ok_or_else(|| Error::InvalidPagePath(page.path.clone()))?;
            Ok((folder.join(name).with_extension("md"), page))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut sources: BTreeMap<&Path, Vec<PathBuf>> = BTreeMap::new();
    for (file_path, page) in &files {
        sources
            .entry(file_path)
            .or_default()
            .push(page.path.clone());
    }
    let collisions: Vec<_> = sources.into_values().filter(|p| p.len() > 1).collect();
    if !collisions.is_empty() {
        return Err(Error::PageNameCollision(collisions));
    }

    Ok(files
        .into_iter()
        .map(|(file_path, page)| (file_path, page_markdown(page)))
        .collect())
}

async fn write_files(files: Vec<(PathBuf, String)>) -> Result<()> {
    let writes = files.into_iter().map(|(path, contents)| async move {
        fs::write(&path, contents)
            .await
            .err()
            .map(|error| (path, error))
    });
    let failures: Vec<_> = join_all(writes).await.into_iter().flatten().collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(Error::SaveError(failures))
    }
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
        let path = PathBuf::from(TOC_PAGE_PREFIX).with_extension("xhtml");
        (!markdown.is_empty()).then_some(Page::new(path, vec![markdown]))
    });

    let pages: Result<Vec<_>> = epub
        .spine()
        .iter()
        .filter_map(|e| e.manifest_entry())
        .map(|entry| {
            let html = entry.read_str()?;
            let path = entry.href().decode().as_ref().into();
            Ok((path, html_to_marked_markdown(&html)?))
        })
        .map(|result| {
            result.map(|(path, markdown)| {
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
    text.chars().any(is_japanese_char)
}

pub fn is_japanese_char(ch: char) -> bool {
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

#[cfg(test)]
mod test {
    use super::page_files;
    use crate::{error::Error, model::Page};
    use std::path::Path;

    #[test]
    fn rejects_pages_sharing_a_file_name() {
        let pages = vec![
            Page::new("/OEBPS/text/p-001.xhtml".into(), vec!["一".into()]),
            Page::new("/OEBPS/extra/p-001.xhtml".into(), vec!["二".into()]),
            Page::new("/OEBPS/text/p-002.xhtml".into(), vec!["三".into()]),
        ];
        let result = page_files(&pages, Path::new("out"));
        assert!(
            matches!(result, Err(Error::PageNameCollision(c)) if c.len() == 1 && c[0].len() == 2)
        );
    }

    #[test]
    fn names_files_after_pages() {
        let pages = vec![Page::new(
            "/OEBPS/text/p-001.xhtml".into(),
            vec!["一".into()],
        )];
        let files = page_files(&pages, Path::new("out")).unwrap();
        assert_eq!(files[0].0, Path::new("out/p-001.md"));
    }
}
