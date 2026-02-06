use crate::{
    controller::{
        DEFAULT_STYLESHEET, HEADER, get_ordered_path,
        xml::{remove_part_tags, remove_think_tags, to_xml, update_image_paths, wrap_tag},
    },
    error::Result,
    model::format::FormatPage,
};
use epub::doc::{EpubDoc, ResourceItem};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ZipLibrary};
use quick_xml::escape::escape;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::Cursor,
    mem,
    path::PathBuf,
};

const XHTML_MIME: &str = "application/xhtml+xml";

#[derive(Debug)]
pub struct DocBuilder {
    pub epub: EpubDoc<Cursor<Vec<u8>>>,
    pub builder: EpubBuilder<ZipLibrary>,
    pub name: String,
    pub pages: Vec<BuilderPage>,
}

impl DocBuilder {
    pub fn new(
        epub: EpubDoc<Cursor<Vec<u8>>>,
        name: String,
        pages: Vec<FormatPage>,
    ) -> Result<Self> {
        let builder = EpubBuilder::new(ZipLibrary::new()?)?;
        let pages = pages.into_iter().map(BuilderPage::from).collect();
        Ok(DocBuilder {
            epub,
            name,
            pages,
            builder,
        })
    }

    pub fn build(mut self) -> Result<(Vec<u8>, String)> {
        self.builder
            .epub_version(EpubVersion::V30)
            .stylesheet(DEFAULT_STYLESHEET)?
            .set_lang("en");

        self.add_images()?;
        self.add_cover_image()?;

        let section_content = self.collect_contents()?;

        for content in section_content {
            self.builder.add_content(content)?;
        }

        let mut content = Vec::new();
        self.builder.generate(&mut content)?;

        Ok((content, mem::take(&mut self.name)))
    }

    pub fn add_cover_image(&mut self) -> Result<()> {
        let Some(cover_id) = self.epub.get_cover_id() else {
            return Ok(());
        };

        let ResourceItem { path, mime, .. } = self.epub.resources.get(&cover_id).unwrap().clone();

        let content = self.epub.get_resource_by_path(&path).unwrap();
        let path = PathBuf::from("Images").join(path.file_name().unwrap());

        self.builder.add_cover_image(path, &*content, mime)?;

        Ok(())
    }

    pub fn add_images(&mut self) -> Result<()> {
        let image_folder = PathBuf::from("Images");
        let images = self.get_images();

        for ResourceItem { path, mime, .. } in images {
            let content = self.epub.get_resource_by_path(&path).unwrap();
            let file_name = path.file_name().unwrap();
            let path = image_folder.join(file_name);
            self.builder.add_resource(path, &*content, mime)?;
        }

        Ok(())
    }

    pub fn get_images(&self) -> Vec<ResourceItem> {
        let cover = self.epub.get_cover_id().unwrap_or_default();
        self.epub
            .resources
            .iter()
            .filter_map(|(id, e)| {
                if e.mime.starts_with("image") && &cover != id {
                    Some(e.to_owned())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn collect_contents(&mut self) -> Result<Vec<EpubContent<Cursor<Vec<u8>>>>> {
        let epub_paths = get_ordered_path(&self.epub);

        let path_map = self.path_map();
        let linked_files = epub_paths
            .iter()
            .map(|stem| link_files(stem, &path_map))
            .collect::<Vec<_>>();

        let file_parts = linked_files
            .into_iter()
            .map(|(md_file, xhtml_path)| {
                let epub_buf = self.epub.get_resource_by_path(&xhtml_path).unwrap();
                let href = to_text_path(&xhtml_path);
                (href, md_file, epub_buf)
            })
            .collect::<Vec<_>>();

        let chapter_file_names = self.chapter_file_names();

        let mut count = 0;

        let mut contents = Vec::new();
        for (href, md_file, epub_buf) in file_parts {
            let mut pages = self.pages.iter();
            let html = match pages.find(|&page| page.is_matching_file(&md_file)) {
                Some(e) => gen_html(&e.content)?,
                None => update_image_paths(str::from_utf8(&epub_buf)?)?,
            };

            let file_name = href.file_name().unwrap();
            let mut content =
                EpubContent::new(href.to_string_lossy(), Cursor::new(html.into_bytes()));
            if chapter_file_names.contains(file_name) {
                count += 1;
                content = content.title(format!("Chapter: {}", count));
            }
            contents.push(content);
        }
        Ok(contents)
    }

    pub fn path_map(&self) -> HashMap<Cow<'_, str>, &PathBuf> {
        self.epub
            .resources
            .values()
            .filter(|r| r.mime == XHTML_MIME)
            .map(|item| {
                let stem = item.path.file_stem().unwrap().to_string_lossy();
                (stem, &item.path)
            })
            .collect()
    }

    pub fn chapter_file_names(&self) -> HashSet<&OsStr> {
        self.epub
            .toc
            .iter()
            .map(|n| n.content.file_name().unwrap())
            .collect::<HashSet<_>>()
    }
}

pub fn link_files(
    path: &PathBuf,
    path_map: &HashMap<Cow<'_, str>, &PathBuf>,
) -> (PathBuf, PathBuf) {
    let file_stem = path.file_stem().unwrap();
    let mut md_file = PathBuf::from(file_stem);
    md_file.set_extension("md");

    let xml_file = path_map.get(&file_stem.to_string_lossy()).unwrap();
    (md_file, xml_file.into())
}

fn to_text_path(path: &PathBuf) -> PathBuf {
    let file_name = path.file_name().unwrap();
    PathBuf::from("Text").join(file_name)
}

fn to_name(path: &PathBuf) -> Option<String> {
    Some(path.file_name()?.to_string_lossy().into_owned())
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

fn gen_html(markdown: &str) -> Result<String> {
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

#[derive(Debug)]
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
