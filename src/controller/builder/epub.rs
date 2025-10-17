use crate::{
    controller::{
        builder::{
            DEFAULT_STYLESHEET, HEADER,
            xml::{remove_part_tags, remove_think_tags, to_xml, update_image_paths, wrap_tag},
        },
        doc::get_ordered_path,
        markdown::parse_anchors,
    },
    state::format_model::FormatPage,
};
use epub::{archive::EpubArchive, doc::EpubDoc};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ZipLibrary};
use quick_xml::escape::escape;
use std::{collections::HashMap, io::Cursor, path::PathBuf};
use tokio::fs;

#[derive(Debug, Clone)]
pub struct DocBuilder {
    pub epub: EpubDoc<Cursor<Vec<u8>>>,
    pub archive: EpubArchive<Cursor<Vec<u8>>>,
}

impl DocBuilder {
    pub async fn build(
        mut self,
        toc: Option<PathBuf>,
        name: String,
        pages: Vec<BuilderPage>,
    ) -> anyhow::Result<()> {
        let mut builder = EpubBuilder::new(ZipLibrary::new()?)?;
        builder
            .epub_version(EpubVersion::V30)
            .stylesheet(DEFAULT_STYLESHEET)?
            .set_lang("en");

        self.add_images(&mut builder)?;
        self.add_cover_image(&mut builder)?;

        let mut section_content = self.collect_contents(&pages)?;

        if let Some(toc) = toc {
            let toc_markdown = fs::read_to_string(toc).await?;
            let anchors = parse_anchors(&toc_markdown);
            section_content = add_titles(section_content, anchors);
        }

        for content in section_content {
            builder.add_content(content)?;
        }

        let mut content = Vec::new();

        builder.generate(&mut content)?;

        save_epub(content, &name).await?;

        Ok(())
    }

    pub fn add_cover_image(&mut self, builder: &mut EpubBuilder<ZipLibrary>) -> anyhow::Result<()> {
        let Some(cover_id) = self.epub.get_cover_id() else {
            return Ok(());
        };
        let (path, mime_type) = self.epub.resources.get(&cover_id).unwrap().clone();
        let content = self.epub.get_resource_by_path(&path).unwrap();

        let path = PathBuf::from("Images").join(path.file_name().unwrap());

        builder.add_cover_image(path, &*content, mime_type)?;

        Ok(())
    }

    pub fn add_images(&mut self, builder: &mut EpubBuilder<ZipLibrary>) -> anyhow::Result<()> {
        let image_folder = PathBuf::from("Images");
        let images = self.get_images();

        for (path, mime_type) in images {
            let content = self.epub.get_resource_by_path(&path).unwrap();
            let file_name = path.file_name().unwrap();
            let path = image_folder.join(file_name);
            builder.add_resource(path, &*content, mime_type)?;
        }

        Ok(())
    }

    pub fn get_images(&self) -> Vec<(PathBuf, String)> {
        let cover = self.epub.get_cover_id().unwrap_or(String::from(""));
        self.epub
            .resources
            .iter()
            .filter_map(|(id, e)| {
                if e.1.starts_with("image") && &cover != id {
                    Some(e.to_owned())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn collect_contents(
        &mut self,
        pages: &[BuilderPage],
    ) -> anyhow::Result<Vec<EpubContent<Cursor<Vec<u8>>>>> {
        let archive_files = self.archive_files();
        let epub_paths = get_ordered_path(&mut self.epub);

        let file_parts = epub_paths
            .into_iter()
            .map(|stem| link_files(stem, &archive_files))
            .map(|(md_file, xml_path)| {
                let epub_buf = self.epub.get_resource_by_path(&xml_path).unwrap();
                let href = to_text_path(&xml_path);
                (href, md_file, epub_buf)
            });

        let mut contents = Vec::new();
        for (href, md_file, epub_buf) in file_parts {
            let html = match pages.iter().find(|&page| page.is_matching_file(&md_file)) {
                Some(e) => gen_html(&e.content)?,
                None => {
                    let html = String::from_utf8(epub_buf)?;
                    update_image_paths(&html)?
                }
            };
            let content = EpubContent::new(href.to_string_lossy(), Cursor::new(html.into_bytes()));
            contents.push(content);
        }
        Ok(contents)
    }

    pub fn archive_files(&mut self) -> Vec<PathBuf> {
        self.archive
            .files
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
    }
}

pub fn link_files(path: PathBuf, archive: &[PathBuf]) -> (PathBuf, PathBuf) {
    let mut md_file = PathBuf::from(path.file_stem().unwrap());
    md_file.set_extension("md");

    let xml_file = archive
        .iter()
        .find(|&e| e.file_stem().unwrap() == md_file.file_stem().unwrap())
        .unwrap();
    (md_file, xml_file.into())
}

fn to_text_path(path: &PathBuf) -> PathBuf {
    let file_name = path.file_name().unwrap();
    PathBuf::from("Text").join(file_name)
}

fn to_name(path: &PathBuf) -> Option<String> {
    Some(path.file_name()?.to_string_lossy().to_string())
}

pub fn add_titles(
    contents: Vec<EpubContent<Cursor<Vec<u8>>>>,
    anchors: HashMap<String, String>,
) -> Vec<EpubContent<Cursor<Vec<u8>>>> {
    contents
        .into_iter()
        .map(|c| {
            let href = to_name(&PathBuf::from(&c.toc.url)).unwrap();
            if let Some(title) = anchors.get(&href) {
                c.title(title)
            } else {
                c.level(2)
            }
        })
        .collect()
}

fn gen_html(markdown: &str) -> anyhow::Result<String> {
    let markdown = remove_think_tags(markdown);
    let markdown = remove_part_tags(&markdown);
    let markdown = replace_jp_symbols(&markdown);
    let markdown = &*escape(markdown);
    let xml = to_xml(markdown);

    let body = wrap_tag(&xml, "body");

    let parts = vec![HEADER, &body].join("\n");
    let html = wrap_tag(&parts, "html");
    Ok(html)
}

pub fn replace_jp_symbols(text: &str) -> String {
    text.replace("」", "\"")
        .replace("「", "\"")
        .replace("』", "\"")
        .replace("『", "\"")
}

pub struct BuilderPage {
    pub path: PathBuf,
    pub content: String,
}

impl BuilderPage {
    pub fn is_matching_file(&self, file: &PathBuf) -> bool {
        let page = self.path.file_name().unwrap_or_default();
        let file = file.file_name().unwrap_or_default();
        page == file
    }
}

impl From<FormatPage> for BuilderPage {
    fn from(FormatPage { path, content, .. }: FormatPage) -> Self {
        BuilderPage {
            path,
            content: content.text(),
        }
    }
}

pub async fn save_epub(content: Vec<u8>, file_name: &String) -> anyhow::Result<()> {
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
