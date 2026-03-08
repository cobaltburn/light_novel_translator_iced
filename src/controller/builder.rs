use crate::controller::{
    DEFAULT_STYLESHEET, get_ordered_path,
    xml::{
        body_tag, extract_head, extract_html_tag, remove_part_tags, remove_think_tags,
        starting_image_tag, to_xml, update_image_paths,
    },
};
use crate::error::{Error, Result};
use crate::model::format::FormatPage;
use epub::doc::{EpubDoc, ResourceItem};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ZipLibrary};
use quick_xml::{
    Reader, Writer,
    escape::escape,
    events::{BytesDecl, BytesEnd, BytesText, Event},
};
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
            .ok_or(Error::BuildError("Invalid file name"))?;
        let path = PathBuf::from("Images").join(file_name);

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
        let pages = self
            .pages
            .iter()
            .filter_map(|e| Some((e.path.file_name()?, e)))
            .collect::<HashMap<_, _>>();

        for (href, md_file, epub_buf) in file_parts {
            let md_name = md_file.file_name().unwrap_or_default();
            let html = str::from_utf8(&epub_buf)?;
            let html = match pages.get(md_name) {
                Some(e) => build_html(html, &e.content)?,
                None => update_image_paths(html)?,
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

const HTML: &str = "html";
fn build_html(html: &str, content: &str) -> Result<String> {
    let content = convert_content(content);
    let body = to_xml(&content);

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
    writer.write_event(Event::DocType(BytesText::new("html")))?;
    writer.write_event(Event::Start(extract_html_tag(html)?))?;

    write_header(&mut writer, html)?;

    write_body(&mut writer, html, &body)?;

    writer.write_event(Event::End(BytesEnd::new(HTML)))?;

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

fn write_header(writer: &mut Writer<Cursor<Vec<u8>>>, html: &str) -> Result<()> {
    let head = extract_head(html)?;
    let mut reader = Reader::from_str(&head);
    reader.config_mut().trim_text(true);

    let mut events = Vec::new();
    loop {
        match reader.read_event()? {
            Event::Eof => break,
            e => events.push(e),
        }
    }

    writer
        .create_element("head")
        .write_inner_content(|writer| {
            events.into_iter().try_for_each(|e| writer.write_event(e))?;
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

fn write_body(writer: &mut Writer<Cursor<Vec<u8>>>, html: &str, body: &str) -> Result<()> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);

    let mut events = Vec::new();
    loop {
        match reader.read_event()? {
            Event::Eof => break,
            e => events.push(e),
        }
    }
    let image = starting_image_tag(html)?;

    let body = body_tag(html)?;

    writer
        .create_element("body")
        .with_attributes(body.attributes().flatten())
        .write_inner_content(|writer| {
            if let Some(image) = image {
                writer.write_event(Event::Empty(image))?;
            };
            events.into_iter().try_for_each(|e| writer.write_event(e))?;
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
