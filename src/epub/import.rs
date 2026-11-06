use crate::{
    epub::html::{html_to_marked_markdown, toc_to_markdown},
    error::Result,
    page::{Page, TOC_PAGE_PREFIX},
    text::{has_translatable_text, markdown_sections},
};
use rbook::Epub;
use std::{io::Cursor, iter, path::PathBuf};

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
