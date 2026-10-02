use crate::{
    controller::{
        DEFAULT_STYLESHEET, TOC_PAGE_STEM, get_ordered_path, html_to_markdown, image_anchors,
        image_marker_indices, insert_anchors, markdown_sections, parse_links,
        replace_image_markers, strip_syosetu_tags, strip_tags, to_html, update_image_paths,
        update_style_path, write_body, write_header,
    },
    error::{Error, Result},
    model::{EpubMetadata, FormatPage},
};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, TocElement, ZipLibrary};
use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesDecl, BytesStart, Event},
};
use rbook::{Epub, epub::manifest::EpubManifestEntry};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::{self, Cursor},
    mem,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct DocBuilder {
    pub epub: Epub,
    pub builder: EpubBuilder<ZipLibrary>,
    pub name: String,
    pub pages: Vec<FormatPage>,
    pub metadata: EpubMetadata,
}

impl DocBuilder {
    pub fn new(
        epub: Epub,
        name: String,
        pages: Vec<FormatPage>,
        metadata: EpubMetadata,
    ) -> Result<Self> {
        Ok(DocBuilder {
            epub,
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
        let Some(cover) = self.epub.manifest().cover_image() else {
            return Ok(());
        };

        let content = cover.read_bytes()?;
        let file_name = cover.href().name().as_str();
        let path = PathBuf::from("Images").join(file_name);
        let mime = cover.media_type();

        self.builder
            .add_cover_image(path, content.as_slice(), mime)?;

        Ok(())
    }

    pub fn add_images(&mut self) -> Result<()> {
        let cover_id = self
            .epub
            .manifest()
            .cover_image()
            .map(|e| e.id())
            .unwrap_or_default();

        let folder = PathBuf::from("Images");
        let selected: Vec<_> = self
            .epub
            .manifest()
            .images()
            .filter(|e| e.id() != cover_id)
            .collect();

        for entry in selected {
            let href = entry.href();
            let file_name = href.name().decode();
            let Ok(content) = self.epub.read_resource_bytes(href) else {
                log::warn!("Skipping unreadable resource: {}", href);
                continue;
            };

            let mime = entry.media_type();
            let path = folder.join(file_name.as_ref());
            if let Err(error) = self.builder.add_resource(path, &*content, mime) {
                log::warn!("{}", error);
            }
        }

        Ok(())
    }

    fn add_js(&mut self) -> Result<()> {
        let folder = PathBuf::from("js");
        let selected: Vec<_> = self.epub.manifest().scripts().collect();

        for entry in selected {
            let href = entry.href();
            let file_name = href.name().decode();
            let Ok(content) = self.epub.read_resource_bytes(href) else {
                log::warn!("Skipping unreadable resource: {}", href);
                continue;
            };

            let mime = entry.media_type();
            let path = folder.join(file_name.as_ref());
            if let Err(error) = self.builder.add_resource(path, &*content, mime) {
                log::warn!("{}", error);
            }
        }
        Ok(())
    }

    fn add_style_sheets(&mut self) -> Result<()> {
        let folder = PathBuf::from("Styles");
        let selected: Vec<_> = self.epub.manifest().styles().collect();

        for entry in selected {
            let href = entry.href();
            let file_name = href.name().decode();
            let Ok(content) = self.epub.read_resource_bytes(href) else {
                log::warn!("Skipping unreadable resource: {}", href);
                continue;
            };

            let mime = entry.media_type();
            let path = folder.join(file_name.as_ref());
            if let Err(error) = self.builder.add_resource(path, &*content, mime) {
                log::warn!("{}", error);
            }
        }
        Ok(())
    }

    pub fn collect_contents(&mut self) -> Result<Vec<EpubContent<Cursor<Vec<u8>>>>> {
        // TODO return manifestentry
        let epub_paths = get_ordered_path(&self.epub);
        let path_map = self.path_map();

        let linked_files: Vec<_> = epub_paths
            .iter()
            .map(|stem| link_files(stem, &path_map))
            .collect::<Result<_>>()?;

        let file_parts: Vec<_> = linked_files
            .into_iter()
            .map(|(md_file, entry)| {
                let html = entry.read_str()?;
                let path = Path::new(entry.href().path().as_str());
                let href = to_text_path(&path)?;
                Ok((href, md_file, html))
            })
            .collect::<Result<_>>()?;

        let mut chapters = self.chapters();
        let toc_ids = self.toc_ids();

        let pages: HashMap<_, _> = self
            .pages
            .iter()
            .filter_map(|e| Some((e.path.file_name()?, e)))
            .collect();

        let mut count = 0;
        let mut contents = Vec::new();

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
                content = links.fold(content, |content, (i, TocLink { path, title })| {
                    let title = title.unwrap_or(format!("Chapter: {}-{}", count, i + 1));
                    content.child(TocElement::new(format!("Text/{path}"), title))
                });
            }
            contents.push(content);
        }

        Ok(contents)
    }

    pub fn path_map(&self) -> HashMap<String, EpubManifestEntry<'_>> {
        self.epub
            .manifest()
            .readable_content()
            .filter_map(|e| {
                let path: PathBuf = e.href().name().decode().as_ref().into();
                let stem = path.file_stem()?.to_string_lossy().to_string();
                Some((stem, e))
            })
            .collect()
    }

    pub fn chapters(&mut self) -> HashMap<String, Vec<TocLink>> {
        let toc_links = self.get_toc_links().unwrap_or_default();
        let mut links = self.links();
        links.extend(toc_links);
        let mut chapters: HashMap<_, Vec<_>> = HashMap::new();
        let Some(pages) = self.epub.toc().contents() else {
            return chapters;
        };

        for page in pages.flatten() {
            let Some(href) = page.href() else {
                continue;
            };
            let file = href.name().decode().to_string();
            let path = href
                .fragment()
                .map_or(file.clone(), |f| format!("{}#{}", file, f));
            let title = links.remove(&path);

            chapters
                .entry(file)
                .or_default()
                .push(TocLink { path, title });
        }

        chapters
    }

    pub fn links(&self) -> HashMap<String, String> {
        let mut links = HashMap::new();
        for FormatPage { content, .. } in &self.pages {
            links.extend(parse_links(content));
        }

        links
    }

    pub fn get_toc_links(&mut self) -> Option<HashMap<String, String>> {
        let i = self
            .pages
            .iter()
            .position(|p| p.path.file_stem() == Some(OsStr::new(TOC_PAGE_STEM)))?;

        Some(parse_links(&self.pages.remove(i).content))
    }

    pub fn toc_ids(&self) -> Vec<String> {
        let Some(pages) = self.epub.toc().contents() else {
            return Vec::new();
        };

        pages
            .flatten()
            .filter_map(|e| e.href().and_then(|e| e.fragment()))
            .map(str::to_string)
            .collect()
    }
}

#[derive(Debug)]
pub enum Anchor {
    Id(String),
    Image(BytesStart<'static>),
}

#[derive(Debug)]
pub struct AnchorPosition {
    pub anchor: Anchor,
    pub section: usize,
    pub line: usize,
    pub position: f64,
}

fn section_markdown(html: &str) -> Result<Vec<String>> {
    Ok(markdown_sections(&html_to_markdown(html)?))
}

fn line_position(lines: &[&str], text: &str) -> Option<(usize, f64)> {
    let line = lines.iter().position(|l| l.contains(text))?;
    Some((line, line as f64 / lines.len() as f64))
}

pub fn id_positions(html: &str, toc_ids: &[String]) -> Result<Vec<AnchorPosition>> {
    let sections = section_markdown(html)?;
    let html = strip_tags(&strip_syosetu_tags(html)?)?;
    let matched_ids = match_ids(&html, toc_ids)?;

    let positions = sections
        .iter()
        .enumerate()
        .flat_map(|(section, content)| {
            let lines: Vec<_> = content.lines().collect();
            matched_ids.iter().filter_map(move |(id, text)| {
                let (line, position) = line_position(&lines, text)?;
                Some(AnchorPosition {
                    anchor: Anchor::Id(id.clone()),
                    section,
                    line,
                    position,
                })
            })
        })
        .collect();

    Ok(positions)
}

pub fn image_positions(
    html: &str,
    images: Vec<(BytesStart<'static>, Option<String>)>,
) -> Result<Vec<AnchorPosition>> {
    let sections = section_markdown(html)?;
    let sections: Vec<Vec<_>> = sections.iter().map(|s| s.lines().collect()).collect();
    let last_section = sections.len().saturating_sub(1);

    let positions = images
        .into_iter()
        .map(|(tag, text)| {
            let found = text.and_then(|text| {
                sections.iter().enumerate().find_map(|(section, lines)| {
                    let (line, position) = line_position(lines, &text)?;
                    Some((section, line, position))
                })
            });
            let (section, line, position) = found.unwrap_or((last_section, usize::MAX, 1.0));
            AnchorPosition {
                anchor: Anchor::Image(tag),
                section,
                line,
                position,
            }
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

fn link_files<'a>(
    path: &Path,
    path_map: &'a HashMap<String, EpubManifestEntry<'a>>,
) -> Result<(PathBuf, &'a EpubManifestEntry<'a>)> {
    let file_stem = path
        .file_stem()
        .ok_or(Error::BuildError("Spine entry has no file stem"))?;
    let md_file = PathBuf::from(file_stem).with_extension("md");

    let xhtml_file = path_map
        .get(file_stem.to_string_lossy().as_ref())
        .ok_or(Error::BuildError("Spine entry has no xhtml resource"))?;

    Ok((md_file, xhtml_file))
}

fn to_text_path(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or(Error::BuildError("Invalid file name found"))?;
    Ok(PathBuf::from("Text").join(file_name))
}

pub fn build_html(html: &str, content: &str, toc_ids: &[String]) -> Result<String> {
    let found = image_marker_indices(content);
    let (marked, estimated): (Vec<_>, Vec<_>) = image_anchors(html)?
        .into_iter()
        .enumerate()
        .partition(|(i, _)| found.contains(i));

    let mut anchors = id_positions(html, toc_ids)?;
    let estimated = estimated.into_iter().map(|(_, image)| image).collect();
    anchors.extend(image_positions(html, estimated)?);
    let content = insert_anchors(content, anchors)?;

    let marked = marked.into_iter().map(|(i, (tag, _))| (i, tag)).collect();
    let content = replace_image_markers(&content, marked)?;
    let content = replace_jp_symbols(&content);
    let content = to_html(&content);

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

#[derive(Debug)]
pub struct TocLink {
    pub path: String,
    pub title: Option<String>,
}

#[cfg(test)]
mod test {}
