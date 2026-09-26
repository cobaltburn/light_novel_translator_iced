use crate::{
    controller::{
        DEFAULT_STYLESHEET, add_image_tags, get_ordered_path, image_position, insert_toc_ids,
        partition_text, strip_syosetu_tags, strip_tags, to_html, update_image_paths,
        update_style_path, write_body, write_header,
    },
    error::{Error, Result},
    model::{EpubMetadata, FormatPage},
};
use epub::doc::{EpubDoc, NavPoint, ResourceItem};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, TocElement, ZipLibrary};
use html2md::rewrite_html;
use pulldown_cmark::{Parser, Tag, TagEnd};
use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesDecl, BytesStart, Event},
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::{self, Cursor},
    mem,
    path::{Path, PathBuf},
};

const XHTML_MIME: &str = "application/xhtml+xml";
const CSS_MIME: &str = "text/css";
const JS_MIME: &str = "application/javascript";
const NCX_MIME: &str = "application/x-dtbncx+xml";

#[derive(Debug)]
pub struct DocBuilder {
    pub epub: EpubDoc<Cursor<Vec<u8>>>,
    pub toc: Vec<Nav>,
    pub builder: EpubBuilder<ZipLibrary>,
    pub name: String,
    pub pages: Vec<FormatPage>,
    pub metadata: EpubMetadata,
}

impl DocBuilder {
    pub fn new(
        mut epub: EpubDoc<Cursor<Vec<u8>>>,
        name: String,
        pages: Vec<FormatPage>,
        metadata: EpubMetadata,
    ) -> Result<Self> {
        use epub::doc::EpubVersion;

        let toc = match epub.version {
            EpubVersion::Version3_0 => get_nav_doc(&mut epub)
                .and_then(|html| parse_nav_doc(&html).ok())
                .unwrap_or_default(),
            EpubVersion::Version2_0 | EpubVersion::Unknown(_) => get_ncx(&mut epub)
                .and_then(|ncx| parse_ncx(&ncx).ok())
                .unwrap_or_default(),
        };

        Ok(DocBuilder {
            epub,
            toc,
            name,
            metadata,
            pages,
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
        let selected: Vec<_> = self
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

            let path = folder.join(file_name);
            if let Err(error) = self.builder.add_resource(path, &*content, mime) {
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
            .map(|(md_file, path)| {
                let html = self
                    .epub
                    .get_resource_str_by_path(&path)
                    .ok_or(Error::BuildError("Spine resource content is missing"))?;
                let href = to_text_path(&path)?;
                Ok((href, md_file, html))
            })
            .collect::<Result<_>>()?;

        let pages: HashMap<_, _> = self
            .pages
            .iter()
            .filter_map(|e| Some((e.path.file_name()?, e)))
            .collect();

        let mut count = 0;
        let mut contents = Vec::new();
        let mut chapters = self.chapters();
        let toc_ids = self.toc_ids();

        for (href, md_file, html) in file_parts {
            let md_name = md_file.file_name().unwrap_or_default();
            let html = match pages.get(md_name) {
                Some(FormatPage { content, .. }) => build_html(&html, content, &toc_ids)?,
                None => {
                    let html = update_image_paths(&html)?;
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

            if let Some(links) = chapters.remove(&file_name) {
                count += 1;
                let mut links = links.into_iter().enumerate();
                let title = links.next().and_then(|(_, link)| link.title);
                content = content.title(title.unwrap_or(format!("Chapter: {}", count)));
                content = links.fold(content, |content, (i, Link { path, title })| {
                    let title = title.unwrap_or(format!("Chapter: {}-{}", count, i + 1));
                    content.child(TocElement::new(format!("Text/{path}"), title))
                });
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

    pub fn chapters(&self) -> HashMap<String, Vec<Link>> {
        let mut titles = self.links();
        let mut chapters: HashMap<_, Vec<_>> = HashMap::new();

        for nav in &self.toc {
            let Some(file_name) = nav.path.file_name() else {
                continue;
            };
            let path = file_name.to_string_lossy().into_owned();
            let file = path
                .split_once('#')
                .map_or(path.as_str(), |(file, _)| file)
                .to_string();
            let title = titles.remove(&path);

            chapters.entry(file).or_default().push(Link { path, title });
        }

        chapters
    }

    pub fn links(&self) -> HashMap<String, String> {
        use pulldown_cmark::Event;

        let mut links = HashMap::new();
        for FormatPage { content, .. } in &self.pages {
            let mut link: Option<(String, String)> = None;
            for event in Parser::new(content) {
                match event {
                    Event::Start(Tag::Link { dest_url, .. }) => {
                        link = Some((dest_url.into_string(), String::new()));
                    }
                    Event::Text(text) => {
                        if let Some((_, title)) = &mut link {
                            title.push_str(&text);
                        }
                    }
                    Event::End(TagEnd::Link) => {
                        if let Some((url, title)) = link.take()
                            && !title.is_empty()
                        {
                            links.insert(url, title);
                        }
                    }
                    _ => (),
                }
            }
        }

        links
    }

    pub fn toc_ids(&self) -> Vec<String> {
        self.toc
            .iter()
            .filter_map(|n| n.path.file_name())
            .map(OsStr::to_string_lossy)
            .filter_map(|e| e.split("#").last().map(ToString::to_string))
            .collect()
    }
}

#[derive(Debug)]
pub struct IdPosition {
    pub id: String,
    pub section: usize,
    pub line: usize,
    pub position: f64,
}

pub fn id_positions(html: &str, toc_ids: &[String]) -> Result<Vec<IdPosition>> {
    let html = strip_syosetu_tags(html)?;
    let html = strip_tags(&html)?;
    let matched_ids = match_ids(&html, &toc_ids)?;
    let markdown = rewrite_html(&html, false);

    let positions = partition_text(&markdown)
        .iter()
        .enumerate()
        .flat_map(|(section, content)| {
            let lines: Vec<_> = content.lines().collect();
            matched_ids.iter().filter_map(move |(id, text)| {
                let line = lines.iter().position(|l| l.contains(text.as_str()))?;
                Some(IdPosition {
                    id: id.clone(),
                    section,
                    line,
                    position: line as f64 / lines.len() as f64,
                })
            })
        })
        .collect();

    Ok(positions)
}

pub fn match_ids(html: &str, toc_ids: &[String]) -> Result<Vec<(String, String)>> {
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let ids: HashSet<_> = toc_ids.iter().map(|e| e.as_bytes()).collect();

    let mut pairs = Vec::new();
    loop {
        match reader.read_event()? {
            Event::Start(tag) => {
                if let Some(attr) = tag.try_get_attribute("id")?
                    && ids.contains(attr.value.as_ref())
                {
                    let text = reader.read_text(tag.name())?.into_inner();
                    if let Some(text) = next_text(text.as_ref())? {
                        let id = attr.normalized_value(XmlVersion::Implicit1_0)?.to_string();
                        pairs.push((id, text));
                    }
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }

    Ok(pairs)
}

fn next_text(text: &[u8]) -> Result<Option<String>> {
    let mut reader = Reader::from_reader(text);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event()? {
            Event::Text(tag) => {
                return Ok(Some(tag.xml10_content()?.to_string()));
            }
            Event::Eof => break,
            _ => (),
        }
    }
    Ok(None)
}

fn link_files(
    path: &Path,
    path_map: &HashMap<Cow<'_, str>, &PathBuf>,
) -> Result<(PathBuf, PathBuf)> {
    let file_stem = path
        .file_stem()
        .ok_or(Error::BuildError("Spine entry has no file stem"))?;
    let md_file = PathBuf::from(file_stem).with_extension("md");

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

pub fn build_html(html: &str, content: &str, toc_ids: &[String]) -> Result<String> {
    let id_positions = id_positions(html, toc_ids)?;
    let content = insert_toc_ids(&content, id_positions);
    let content = replace_jp_symbols(&content);
    let content = to_html(&content);

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

pub fn replace_jp_symbols(text: &str) -> String {
    text.replace("」", "\"")
        .replace("「", "\"")
        .replace("』", "\"")
        .replace("『", "\"")
}

pub fn get_nav_doc(epub: &mut EpubDoc<Cursor<Vec<u8>>>) -> Option<String> {
    let ResourceItem { path, .. } = epub.resources.get("toc")?;
    epub.get_resource_str_by_path(path.clone())
}

pub fn parse_nav_doc(html: &str) -> Result<Vec<Nav>> {
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let nav_xml = loop {
        match reader.read_event()? {
            Event::Start(tag) if is_nav_toc(&tag) => {
                break reader.read_text(tag.name())?;
            }
            Event::Eof => return Ok(Vec::new()),
            _ => (),
        }
    };

    let nav_html = nav_xml.into_inner();
    let mut reader = Reader::from_reader(nav_html.as_ref());
    let mut navs: Vec<Nav> = Vec::new();
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"a" => {
                if let Some(href) = tag.try_get_attribute("href")? {
                    let path = href.normalized_value(XmlVersion::Implicit1_0)?;
                    let path = PathBuf::from(path.as_ref());
                    let label = reader.read_text(tag.name())?.xml10_content()?.to_string();
                    navs.push(Nav { label, path });
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }
    Ok(navs)
}

pub fn is_nav_toc(tag: &BytesStart<'_>) -> bool {
    tag.try_get_attribute("epub:type")
        .ok()
        .flatten()
        .and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok())
        .is_some_and(|a| a == "toc")
}

pub fn get_ncx(epub: &mut EpubDoc<Cursor<Vec<u8>>>) -> Option<String> {
    let (_, ResourceItem { path, .. }) = epub.resources.iter().find(|(_, r)| r.mime == NCX_MIME)?;
    epub.get_resource_str_by_path(path.clone())
}

pub fn parse_ncx(ncx: &str) -> Result<Vec<Nav>> {
    let mut reader = Reader::from_str(ncx);
    reader.config_mut().trim_text(true);

    let mut navs: Vec<Nav> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let mut in_label = false;

    loop {
        match reader.read_event()? {
            Event::Start(tag) => match tag.name().as_ref() {
                b"navPoint" => {
                    open.push(navs.len());
                    navs.push(Nav::default());
                }
                b"navLabel" => in_label = true,
                _ => (),
            },
            Event::End(tag) => match tag.name().as_ref() {
                b"navPoint" => {
                    open.pop();
                }
                b"navLabel" => in_label = false,
                _ => (),
            },
            Event::Text(text) if in_label => {
                if let Some(&i) = open.last() {
                    navs[i].label.push_str(&text.xml10_content()?);
                }
            }
            Event::Empty(tag) if tag.name().as_ref() == b"content" => {
                if let Some(&i) = open.last()
                    && let Some(src) = tag.try_get_attribute("src")?
                {
                    let value = src.normalized_value(XmlVersion::Implicit1_0)?;
                    navs[i].path = PathBuf::from(value.as_ref());
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }

    Ok(navs)
}

#[derive(Debug)]
pub struct Link {
    pub path: String,
    pub title: Option<String>,
}

#[derive(Debug, Default)]
pub struct Nav {
    pub label: String,
    pub path: PathBuf,
}

impl From<NavPoint> for Nav {
    fn from(NavPoint { label, content, .. }: NavPoint) -> Self {
        Self {
            label,
            path: content,
        }
    }
}

#[cfg(test)]
mod test {
    // use super::*;
}
