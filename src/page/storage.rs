use crate::{
    error::{Error, Result, TaskResultExt},
    page::Page,
    text::{part_tag, remove_think_tags},
};
use iced::{Task, advanced::graphics::futures::MaybeSend, futures::future::join_all};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tokio::fs;

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

#[cfg(test)]
mod test {
    use super::page_files;
    use crate::{error::Error, page::Page};
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
