use crate::{
    controller::{
        DEFAULT_STYLESHEET, PageLinks, ResourcePaths, TOC_PAGE_STEM, get_ordered_path,
        html_to_markdown, image_anchors, image_marker_indices, insert_anchors, markdown_sections,
        parse_headers, parse_links, replace_image_markers, strip_syosetu_tags, strip_tags, to_html,
        update_image_paths, update_resource_paths, write_body, write_header,
    },
    error::{Error, Result},
    model::{EpubMetadata, FormatPage},
};
use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesDecl, BytesStart, Event},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rbook::{
    Epub,
    epub::{
        EpubChapter,
        manifest::{DetachedEpubManifestEntry, EpubManifestEntry},
    },
};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::Cursor,
    mem,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct DocBuilder {
    pub epub: Epub,
    pub builder: Epub,
    pub name: String,
    pub pages: Vec<FormatPage>,
    pub metadata: EpubMetadata,
}

impl DocBuilder {
    pub fn new(epub: Epub, name: String, pages: Vec<FormatPage>, metadata: EpubMetadata) -> Self {
        DocBuilder {
            epub,
            name,
            metadata,
            pages,
            builder: Epub::new(),
        }
    }

    pub fn build(mut self) -> Result<(Vec<u8>, String)> {
        let title = mem::take(&mut self.metadata.title);
        let identifier = self
            .epub
            .metadata()
            .identifier()
            .map(|e| e.value().to_string())
            .unwrap_or_else(|| format!("urn:light-novel-translator:{}", title));

        let authors: Vec<_> = self
            .metadata
            .authors
            .split("&")
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty())
            .collect();

        self.builder
            .edit()
            .identifier(identifier)
            .title(title)
            .author(authors)
            .language("en")
            .modified_now()
            .resource(("stylesheet.css", DEFAULT_STYLESHEET));

        let manifest = self.epub.manifest();
        let cover = manifest.cover_image();
        let mut paths = ResourcePaths::default();
        if let Some(cover) = cover {
            paths.insert(cover.href(), "Images");
        }

        let images: Vec<_> = manifest
            .images()
            .filter(|e| Some(e.id()) != cover.map(|c| c.id()))
            .collect();
        let styles: Vec<_> = manifest.styles().collect();
        let scripts: Vec<_> = manifest.scripts().collect();
        let entries: Vec<_> = [(images, "Images"), (styles, "Styles"), (scripts, "Script")]
            .into_iter()
            .flat_map(|(entries, folder)| entries.into_iter().map(move |e| (e, folder)))
            .map(|(entry, folder)| {
                let path = paths.insert(entry.href(), folder);
                (entry, path)
            })
            .collect();
        self.builder.edit().resource(read_resources(entries));
        self.add_cover_image(&paths);

        let chapters = self.collect_contents(&paths)?;
        self.builder.edit().chapter(chapters);

        let content = self.builder.write().to_vec()?;

        Ok((content, self.name))
    }

    pub fn add_cover_image(&mut self, paths: &ResourcePaths) {
        let Some(cover) = self.epub.manifest().cover_image() else {
            return;
        };
        let Some(path) = paths.get(cover.href()) else {
            return;
        };

        if let Ok(content) = cover.read_bytes() {
            let path = path.to_string();
            let mime = cover.media_type();

            self.builder
                .edit()
                .cover_image(DetachedEpubManifestEntry::from((path, content)).media_type(mime));
        }
    }

    pub fn collect_contents(&mut self, paths: &ResourcePaths) -> Result<Vec<EpubChapter>> {
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
                let href = to_text_path(path)?;
                let source = entry.href().path().decode().into_owned();
                Ok((href, md_file, source, html))
            })
            .collect::<Result<_>>()?;

        let mut chapters = self.chapters();
        let toc_ids = self.toc_ids();

        let pages: HashMap<_, _> = self
            .pages
            .iter()
            .filter_map(|e| Some((e.path.file_name()?, e)))
            .collect();

        let mut contents = Vec::new();

        for (page, (href, md_file, source, html)) in file_parts.into_iter().enumerate() {
            let md_name = md_file.file_name().unwrap_or_default();
            let links = PageLinks {
                page: &source,
                paths,
            };
            let html = match pages.get(md_name) {
                Some(FormatPage { content, .. }) => {
                    build_html(&html, content, &toc_ids, links, page)?
                }
                None => {
                    let html = update_image_paths(&html, links)?;
                    update_resource_paths(&html, links)?
                }
            };

            let file_name = href
                .file_name()
                .ok_or(Error::BuildError("Invalid file name found"))?
                .to_string_lossy()
                .into_owned();
            let href = href.to_string_lossy().into_owned();

            let mut sub_chapters = parse_headers(&html)?;
            contents.push(EpubChapter::unlisted(href).xhtml(html));
            let Some(links) = chapters.remove(&file_name) else {
                contents.extend(sub_chapters.remove("").unwrap_or_default().into_iter().map(
                    |(id, title)| EpubChapter::new(title).href(format!("Text/{file_name}#{id}")),
                ));
                continue;
            };

            for TocLink { path, title } in links {
                contents.push(EpubChapter::new(title).href(format!("Text/{path}")));
                let fragment = path.rsplit_once('#').map_or_default(|(_, f)| f);
                let sub_chapters = sub_chapters.remove(fragment).unwrap_or_default();
                contents.extend(sub_chapters.into_iter().map(|(id, title)| {
                    EpubChapter::new(title).href(format!("Text/{file_name}#{id}"))
                }));
            }
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

        for (i, page) in pages.flatten().enumerate() {
            let Some(href) = page.href() else {
                continue;
            };
            let file = href.name().decode().to_string();
            let path = href
                .fragment()
                .map_or(file.clone(), |f| format!("{}#{}", file, f));
            let title = links.remove(&path).unwrap_or(format!("Chapter: {}", i + 1));

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

fn read_resources(entries: Vec<(EpubManifestEntry<'_>, String)>) -> Vec<DetachedEpubManifestEntry> {
    entries
        .into_par_iter()
        .filter_map(|(entry, path)| {
            let Ok(content) = entry.read_bytes() else {
                log::warn!("Skipping unreadable resource: {}", entry.href());
                return None;
            };

            Some(DetachedEpubManifestEntry::from((path, content)).media_type(entry.media_type()))
        })
        .collect()
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

pub fn build_html(
    html: &str,
    content: &str,
    toc_ids: &[String],
    links: PageLinks<'_>,
    page: usize,
) -> Result<String> {
    let found = image_marker_indices(content);
    let (marked, estimated): (Vec<_>, Vec<_>) = image_anchors(html, links)?
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
            write_header(writer, html, links)?;
            write_body(writer, &content, page)
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
    pub title: String,
}

#[cfg(test)]
mod test {}
