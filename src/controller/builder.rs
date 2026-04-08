use crate::controller::xml::{count_lines, image_position};
use crate::error::{Error, Result};
use crate::model::format::FormatPage;
use crate::{
    controller::{
        DEFAULT_STYLESHEET, get_ordered_path,
        xml::{
            extract_head, remove_part_tags, to_xml, update_image_paths, update_style_path,
            update_tag_path,
        },
    },
    model::format::EpubMetadata,
};
use epub::doc::{EpubDoc, ResourceItem};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ZipLibrary};
use quick_xml::{
    Reader, Writer,
    escape::escape,
    events::{BytesDecl, BytesStart, Event},
};
use std::collections::VecDeque;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::{self, Cursor},
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
    pub metadata: EpubMetadata,
}

impl DocBuilder {
    pub fn new(
        epub: EpubDoc<Cursor<Vec<u8>>>,
        name: String,
        pages: Vec<FormatPage>,
        metadata: EpubMetadata,
    ) -> Result<Self> {
        Ok(DocBuilder {
            epub,
            name,
            metadata,
            pages: pages.into_iter().map(BuilderPage::from).collect(),
            builder: EpubBuilder::new(ZipLibrary::new()?)?,
        })
    }

    pub fn build(mut self) -> Result<(Vec<u8>, String)> {
        self.builder
            .epub_version(EpubVersion::V30)
            .stylesheet(DEFAULT_STYLESHEET)?
            .set_lang("en");

        self.builder.set_title(mem::take(&mut self.metadata.title));

        let authors = self
            .metadata
            .authors
            .split("&")
            .map(|e| e.trim().to_string())
            .collect();

        self.builder.set_authors(authors);

        self.add_images()?;
        self.add_cover_image()?;
        self.add_style_sheets()?;

        for content in self.collect_contents()? {
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
        let file_name = path
            .file_name()
            .ok_or(Error::BuildError("Invalid file name found"))?;
        let path = PathBuf::from("Images").join(file_name);

        self.builder.add_cover_image(path, &*content, mime)?;

        Ok(())
    }

    pub fn add_images(&mut self) -> Result<()> {
        let folder = PathBuf::from("Images");
        let image_resources: Vec<_> = self
            .get_images()
            .into_iter()
            .flat_map(|ResourceItem { path, mime, .. }| {
                let content = self.epub.get_resource_by_path(&path)?;
                let path = folder.join(path.file_name()?);
                Some((path, content, mime))
            })
            .collect();

        for (path, content, mime) in image_resources {
            if let Err(error) = self.builder.add_resource(path, &*content, mime) {
                log::error!("{:#?}", error);
            }
        }

        Ok(())
    }

    pub fn get_images(&self) -> Vec<ResourceItem> {
        let cover = self.epub.get_cover_id().unwrap_or_default();
        self.epub
            .resources
            .iter()
            .filter(|(id, e)| e.mime.starts_with("image") && &cover != *id)
            .map(|(_, e)| e.to_owned())
            .collect()
    }

    fn add_style_sheets(&mut self) -> Result<()> {
        let mime = "text/css";
        let folder = PathBuf::from("Styles");
        let style_sheets: Vec<_> = self
            .get_style_sheets()
            .into_iter()
            .flat_map(|ResourceItem { path, .. }| {
                let content = self.epub.get_resource_by_path(&path)?;
                let path = folder.join(path.file_name()?);
                Some((path, content))
            })
            .collect();

        for (path, content) in style_sheets {
            if let Err(error) = self.builder.add_resource(path, &*content, mime) {
                log::error!("{:#?}", error);
            }
        }

        Ok(())
    }

    pub fn get_style_sheets(&self) -> Vec<ResourceItem> {
        self.epub
            .resources
            .iter()
            .filter(|(_, e)| e.mime == "text/css")
            .map(|(_, e)| e.to_owned())
            .collect()
    }

    pub fn collect_contents(&mut self) -> Result<Vec<EpubContent<Cursor<Vec<u8>>>>> {
        let epub_paths = get_ordered_path(&self.epub);

        let path_map = self.path_map();
        let linked_files: Vec<_> = epub_paths
            .iter()
            .map(|stem| link_files(stem, &path_map))
            .collect();

        let file_parts: Vec<_> = linked_files
            .into_iter()
            .map(|(md_file, xhtml_path)| {
                let epub_buf = self.epub.get_resource_by_path(&xhtml_path).unwrap();
                let href = to_text_path(&xhtml_path);
                (href, md_file, epub_buf)
            })
            .collect();

        let chapter_file_names = self.chapter_file_names();

        let mut contents = Vec::new();
        let pages: HashMap<_, _> = self
            .pages
            .iter()
            .filter_map(|e| Some((e.path.file_name()?, e)))
            .collect();

        let mut count = 0;
        for (href, md_file, epub_buf) in file_parts {
            let md_name = md_file.file_name().unwrap_or_default();
            let html = str::from_utf8(&epub_buf)?;
            let html = match pages.get(md_name) {
                Some(e) => build_html(html, &e.content)?,
                None => {
                    let html = update_image_paths(html)?;
                    update_style_path(&html)?
                }
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
            .map(|ResourceItem { path, .. }| {
                let stem = path.file_stem().unwrap().to_string_lossy();
                (stem, path)
            })
            .collect()
    }

    pub fn chapter_file_names(&self) -> HashSet<&OsStr> {
        self.epub
            .toc
            .iter()
            .map(|n| n.content.file_name().unwrap())
            .collect()
    }
}

fn link_files(path: &PathBuf, path_map: &HashMap<Cow<'_, str>, &PathBuf>) -> (PathBuf, PathBuf) {
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

fn build_html(html: &str, content: &str) -> Result<String> {
    let content = remove_part_tags(content);
    let content = replace_jp_symbols(&content);
    let content = escape(content);
    let content = to_xml(&content);

    let lines = count_lines(&content)?;
    let images = image_position(html)?;
    let content = add_image_tags(&content, lines, images)?;

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    writer
        .create_element("html")
        .with_attribute(("xmlns:epub", "http://www.idpf.org/2007/ops"))
        .with_attribute(("xml:lang", "en"))
        .write_inner_content(|writer| {
            write_header(writer, html).map_err(io::Error::other)?;
            write_body(writer, &content).map_err(io::Error::other)
        })?;

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

fn write_header(writer: &mut Writer<Cursor<Vec<u8>>>, html: &str) -> Result<()> {
    let head = extract_head(html)?;

    writer
        .create_element("head")
        .write_inner_content(|writer| write_head(writer, head).map_err(io::Error::other))?;

    Ok(())
}

pub fn write_head(writer: &mut Writer<Cursor<Vec<u8>>>, head: Cow<'_, str>) -> Result<()> {
    let folder = PathBuf::from("../Styles");
    let mut reader = Reader::from_str(&head);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event()? {
            Event::Empty(tag) if tag.name().as_ref() == b"link" => {
                let tag = update_tag_path(tag, &folder, "href")?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Eof => break,
            e => writer.write_event(e)?,
        }
    }

    writer
        .create_element("link")
        .with_attribute(("rel", "stylesheet"))
        .with_attribute(("type", "text/css"))
        .with_attribute(("href", "../stylesheet.css"))
        .write_empty()?;
    Ok(())
}

fn write_body(writer: &mut Writer<Cursor<Vec<u8>>>, content: &str) -> Result<()> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    writer
        .create_element("body")
        .with_attribute(("class", "p-text"))
        .write_inner_content(|writer| {
            loop {
                match reader.read_event().map_err(io::Error::other)? {
                    Event::Eof => break,
                    e => writer.write_event(e)?,
                }
            }
            Ok(())
        })?;
    Ok(())
}

fn add_image_tags(content: &str, lines: i32, images: Vec<(BytesStart<'_>, f64)>) -> Result<String> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    let mut count = 0;
    let mut images = VecDeque::from(images);

    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"p" => {
                if let Some((tag, _)) =
                    images.pop_front_if(|(_, i)| (count as f64 / lines as f64) >= *i)
                {
                    writer.write_event(Event::Empty(tag))?;
                }

                writer.write_event(Event::Start(tag))?;
                count += 1;
            }
            Event::Eof => break,
            e => writer.write_event(e)?,
        }
    }

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
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

impl From<FormatPage> for BuilderPage {
    fn from(FormatPage { path, content, .. }: FormatPage) -> Self {
        BuilderPage {
            path,
            content: content,
        }
    }
}
