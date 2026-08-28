use crate::{
    controller::{
        DEFAULT_STYLESHEET, count_lines, extract_head, get_ordered_path, image_position,
        remove_part_tags, to_xml, update_image_paths, update_style_path, update_tag_path,
    },
    error::{Error, Result},
    model::{EpubMetadata, FormatPage},
};
use epub::doc::{EpubDoc, ResourceItem};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ZipLibrary};
use quick_xml::{
    Reader, Writer,
    events::{BytesDecl, BytesEnd, BytesStart, Event},
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, VecDeque},
    io::{self, Cursor},
    mem,
    path::{Path, PathBuf},
};

const XHTML_MIME: &str = "application/xhtml+xml";
const CSS_MIME: &str = "text/css";
const JS_MIME: &str = "application/javascript";

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
            .add_language("en");

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
        self.add_js()?;

        for content in self.collect_contents()? {
            self.builder.add_content(content)?;
        }

        let mut content = Vec::new();
        self.builder.generate(&mut content)?;

        Ok((content, self.name))
    }

    pub fn add_cover_image(&mut self) -> Result<()> {
        let Some(cover_id) = self.epub.get_cover_id() else {
            return Ok(());
        };

        let ResourceItem { path, mime, .. } = self
            .epub
            .resources
            .get(&cover_id)
            .ok_or(Error::BuildError("Cover id has no matching resource"))?
            .clone();

        let content = self
            .epub
            .get_resource_by_path(&path)
            .ok_or(Error::BuildError("Cover image content is missing"))?;
        let file_name = path
            .file_name()
            .ok_or(Error::BuildError("Invalid file name found"))?;
        let path = PathBuf::from("Images").join(file_name);

        self.builder
            .add_cover_image(path, content.as_slice(), mime)?;

        Ok(())
    }

    fn add_resources(
        &mut self,
        folder: &str,
        select: impl Fn(&str, &ResourceItem) -> bool,
    ) -> Result<()> {
        let folder = PathBuf::from(folder);
        let selected: Vec<ResourceItem> = self
            .epub
            .resources
            .iter()
            .filter(|(id, resource)| select(id.as_str(), resource))
            .map(|(_, resource)| resource.clone())
            .collect();

        for ResourceItem { path, mime, .. } in selected {
            let Some(file_name) = path.file_name() else {
                log::warn!("Skipping resource without a file name: {}", path.display());
                continue;
            };
            let Some(content) = self.epub.get_resource_by_path(&path) else {
                log::warn!("Skipping unreadable resource: {}", path.display());
                continue;
            };

            if let Err(error) = self
                .builder
                .add_resource(folder.join(file_name), &*content, mime)
            {
                log::warn!("{}", error);
            }
        }

        Ok(())
    }

    pub fn add_images(&mut self) -> Result<()> {
        let cover_id = self.epub.get_cover_id();
        self.add_resources("Images", |id, resource| {
            resource.mime.starts_with("image") && Some(id) != cover_id.as_deref()
        })
    }

    fn add_js(&mut self) -> Result<()> {
        self.add_resources("js", |_, resource| resource.mime == JS_MIME)
    }

    fn add_style_sheets(&mut self) -> Result<()> {
        self.add_resources("Styles", |_, resource| resource.mime == CSS_MIME)
    }

    pub fn collect_contents(&mut self) -> Result<Vec<EpubContent<Cursor<Vec<u8>>>>> {
        let epub_paths = get_ordered_path(&self.epub);

        let path_map = self.path_map();
        let linked_files: Vec<_> = epub_paths
            .iter()
            .map(|stem| link_files(stem, &path_map))
            .collect::<Result<_>>()?;

        let file_parts: Vec<_> = linked_files
            .into_iter()
            .map(|(md_file, xhtml_path)| {
                let epub_buf = self
                    .epub
                    .get_resource_by_path(&xhtml_path)
                    .ok_or(Error::BuildError("Spine resource content is missing"))?;
                let href = to_text_path(&xhtml_path)?;
                Ok((href, md_file, epub_buf))
            })
            .collect::<Result<_>>()?;

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

            let file_name = href
                .file_name()
                .ok_or(Error::BuildError("Invalid file name found"))?
                .to_string_lossy()
                .into_owned();
            let mut content =
                EpubContent::new(href.to_string_lossy(), Cursor::new(html.into_bytes()));
            if chapter_file_names.contains(&file_name) {
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
            .filter_map(|ResourceItem { path, .. }| {
                let stem = path.file_stem()?.to_string_lossy();
                Some((stem, path))
            })
            .collect()
    }

    pub fn chapter_file_names(&self) -> HashSet<String> {
        self.epub
            .toc
            .iter()
            .filter_map(|n| n.content.file_name())
            .filter_map(|p| p.to_string_lossy().split('#').next().map(|e| e.to_string()))
            .collect()
    }
}

fn link_files(
    path: &Path,
    path_map: &HashMap<Cow<'_, str>, &PathBuf>,
) -> Result<(PathBuf, PathBuf)> {
    let file_stem = path
        .file_stem()
        .ok_or(Error::BuildError("Spine entry has no file stem"))?;
    let mut md_file = PathBuf::from(file_stem);
    md_file.set_extension("md");

    let xml_file = path_map
        .get(&file_stem.to_string_lossy())
        .ok_or(Error::BuildError("Spine entry has no xhtml resource"))?;

    Ok((md_file, xml_file.to_path_buf()))
}

fn to_text_path(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or(Error::BuildError("Invalid file name found"))?;
    Ok(PathBuf::from("Text").join(file_name))
}

fn build_html(html: &str, content: &str) -> Result<String> {
    let content = remove_part_tags(content);
    let content = replace_jp_symbols(&content);
    let content = to_xml(&content);

    let images = image_position(html)?;
    let content = add_image_tags(&content, images)?;

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

const ANCHOR_TAG: &[u8] = b"a";
const DIV: &str = "div";

fn write_body(writer: &mut Writer<Cursor<Vec<u8>>>, content: &str) -> Result<()> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    writer
        .create_element("body")
        .with_attribute(("class", "p-text"))
        .write_inner_content(|writer| {
            loop {
                match reader.read_event().map_err(io::Error::other)? {
                    Event::Start(tag) if tag.name().as_ref() == ANCHOR_TAG => {
                        writer.write_event(Event::Start(BytesStart::new(DIV)))?;
                        writer.write_event(Event::Start(tag))?;
                    }
                    Event::End(tag) if tag.name().as_ref() == ANCHOR_TAG => {
                        writer.write_event(Event::End(tag))?;
                        writer.write_event(Event::End(BytesEnd::new(DIV)))?;
                    }
                    Event::Eof => break,
                    e => writer.write_event(e)?,
                }
            }
            Ok(())
        })?;
    Ok(())
}

fn add_image_tags(content: &str, mut images: Vec<(BytesStart<'_>, f64)>) -> Result<String> {
    let lines = count_lines(content)?;
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    let mut line_count = 0;
    images.sort_by(|(_, a), (_, b)| a.total_cmp(b));
    let mut images = VecDeque::from(images);

    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"p" => {
                line_count += 1;
                let position = line_count as f64 / lines as f64;

                let mut image_tags = Vec::new();
                while let Some((tag, _)) = images.pop_front_if(|&mut (_, i)| position >= i) {
                    image_tags.push(tag);
                }

                write_image_tags(&mut writer, image_tags)?;
                writer.write_event(Event::Start(tag))?;
            }
            Event::Eof => break,
            e => writer.write_event(e)?,
        }
    }

    let remaining: Vec<_> = images.into_iter().map(|(tag, _)| tag).collect();
    write_image_tags(&mut writer, remaining)?;

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

fn write_image_tags(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    image_tags: Vec<BytesStart<'_>>,
) -> Result<()> {
    if image_tags.is_empty() {
        return Ok(());
    }

    writer
        .create_element("div")
        .with_attribute(("style", "text-align: center;"))
        .write_inner_content(|writer| {
            writer.create_element("p").write_inner_content(|writer| {
                for tag in image_tags {
                    writer.write_event(Event::Empty(tag))?;
                }
                Ok(())
            })?;
            Ok(())
        })?;

    Ok(())
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
