use crate::controller::{
    DEFAULT_STYLESHEET, get_ordered_path,
    xml::{
        extract_head, remove_part_tags, remove_think_tags, starting_image_tag, to_xml,
        update_image_paths, update_style_path, update_tag_path,
    },
};
use crate::error::{Error, Result};
use crate::model::format::FormatPage;
use epub::doc::{EpubDoc, ResourceItem};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ZipLibrary};
use quick_xml::{
    Reader, Writer,
    escape::escape,
    events::{BytesDecl, BytesStart, BytesText, Event},
};
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
        for ResourceItem { path, mime, .. } in self.get_images() {
            let content = self.epub.get_resource_by_path(&path).unwrap();
            let file_name = path.file_name().unwrap();
            let path = folder.join(file_name);
            self.builder.add_resource(path, &*content, mime)?;
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
        let folder = PathBuf::from("Styles");
        for ResourceItem { path, mime, .. } in self.get_style_sheets() {
            let content = self.epub.get_resource_by_path(&path).unwrap();
            let file_name = path.file_name().unwrap();
            let path = folder.join(file_name);
            self.builder.add_resource(path, &*content, mime)?;
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
    let content = convert_content(content);
    let content = to_xml(&content);
    let image = starting_image_tag(html)?;

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
    writer.write_event(Event::DocType(BytesText::new("html")))?;

    writer
        .create_element("html")
        .with_attribute(("xmlns:epub", "http://www.idpf.org/2007/ops"))
        .with_attribute(("xml:lang", "en"))
        .with_attribute(("class", "vrtl"))
        .write_inner_content(|writer| {
            write_header(html, writer).map_err(io::Error::other)?;
            write_body(writer, &content, image).map_err(io::Error::other)
        })?;

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

fn write_header(html: &str, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
    let head = extract_head(html)?;

    writer
        .create_element("head")
        .write_inner_content(|writer| {
            write_head(head, writer).map_err(io::Error::other)?;
            writer
                .create_element("link")
                .with_attribute(("rel", "stylesheet"))
                .with_attribute(("type", "text/css"))
                .with_attribute(("href", "../stylesheet.css"))
                .write_empty()?;
            Ok(())
        })?;

    Ok(())
}

pub fn write_head(head: Cow<'_, str>, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
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

    Ok(())
}

fn write_body(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    content: &str,
    header_image: Option<BytesStart<'_>>,
) -> Result<()> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    writer
        .create_element("body")
        .write_inner_content(|writer| {
            if let Some(image) = header_image {
                writer.write_event(Event::Empty(image))?;
            };
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

fn convert_content(content: &str) -> Cow<'static, str> {
    let content = remove_think_tags(content);
    let content = remove_part_tags(&content);
    let content = replace_jp_symbols(&content);
    let content = escape(content);
    content
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
            content: content.text(),
        }
    }
}
