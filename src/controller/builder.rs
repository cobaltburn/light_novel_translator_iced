use crate::{
    controller::{
        DEFAULT_STYLESHEET, Nav, TOC_PAGE_STEM, get_ordered_path, html_to_markdown, image_anchors,
        image_marker_indices, insert_anchors, markdown_sections, parse_links, read_toc,
        replace_image_markers, strip_syosetu_tags, strip_tags, to_html, update_image_paths,
        update_style_path, write_body, write_header,
    },
    error::{Error, Result},
    model::{EpubMetadata, FormatPage},
};
use epub::doc::{EpubDoc, ResourceItem};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, TocElement, ZipLibrary};
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
        let toc = read_toc(&mut epub)
            .map(|(_, navs)| navs)
            .unwrap_or_default();

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

    pub fn chapters(&mut self) -> HashMap<String, Vec<TocLink>> {
        let toc_links = self.get_toc_links().unwrap_or_default();
        let mut links = self.links();
        links.extend(toc_links);
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
        self.toc
            .iter()
            .filter_map(|n| n.path.file_name())
            .map(OsStr::to_string_lossy)
            .filter_map(|e| e.split("#").last().map(ToString::to_string))
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
mod test {
    use super::*;
    use crate::controller::{has_translatable_text, html_to_marked_markdown};

    const HTML: &str = r#"<html><head><title>Title</title></head><body>
<p>first line</p>
<p><img src="a.jpg"/></p>
<p>second line</p>
<p>third line</p>
<p><img src="b.jpg"/></p>
</body></html>"#;

    #[test]
    fn image_anchors_use_following_text() {
        let anchors = image_anchors(HTML).unwrap();
        let texts: Vec<_> = anchors.iter().map(|(_, t)| t.as_deref()).collect();
        assert_eq!(texts, [Some("second line"), None]);
    }

    #[test]
    fn image_positions_are_section_relative() {
        let positions = image_positions(HTML, image_anchors(HTML).unwrap()).unwrap();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].section, 0);
        assert!(positions[0].position > 0.0 && positions[0].position < 1.0);
        assert_eq!(positions[1].position, 1.0);
    }

    #[test]
    fn insert_anchors_places_images_in_section() {
        let content = "<part>1</part>\n\none\n\ntwo\n\n<part>2</part>\n\nthree\n\nfour\n";
        let tag = BytesStart::new("img").with_attributes([("src", "../Images/a.jpg")]);
        let anchors = vec![AnchorPosition {
            anchor: Anchor::Image(tag.into_owned()),
            section: 1,
            line: 3,
            position: 0.5,
        }];
        let output = insert_anchors(content, anchors).unwrap();
        let image = output.find("<img").unwrap();
        assert!(output.find("three").unwrap() < image);
        assert!(image < output.find("four").unwrap());

        let html = to_html(&output);
        assert!(html.contains("<p>four</p>"));
    }

    #[test]
    fn source_markdown_has_numbered_markers() {
        let markdown = html_to_marked_markdown(HTML).unwrap();
        let lines: Vec<_> = markdown.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(
            lines,
            [
                "first line",
                "[[IMG:0]]",
                "second line",
                "third line",
                "[[IMG:1]]"
            ]
        );
        assert!(!html_to_markdown(HTML).unwrap().contains("IMG"));
    }

    #[test]
    fn image_only_page_has_no_translatable_text() {
        let html = r#"<html><head><title>Title</title></head><body>
<div><svg><image xlink:href="cover.jpg"/></svg></div>
</body></html>"#;
        let markdown = html_to_marked_markdown(html).unwrap();
        assert!(markdown.contains("[[IMG:0]]"));
        assert!(!has_translatable_text(&markdown));
        assert!(has_translatable_text(
            &html_to_marked_markdown(HTML).unwrap()
        ));
    }

    #[test]
    fn build_html_uses_markers_and_falls_back() {
        // Marker 0 kept, marker 1 dropped by the translation
        let content = "<part>1</part>\n\none\n\n[[IMG:0]]\n\ntwo\n\nthree\n";
        let html = build_html(HTML, content, &[]).unwrap();
        let a = html.find("a.jpg").unwrap();
        let b = html.find("b.jpg").unwrap();
        assert!(html.find("one").unwrap() < a && a < html.find("two").unwrap());
        assert!(html.find("three").unwrap() < b);
        assert!(!html.contains("IMG"));
    }

    #[test]
    fn replace_image_markers_tolerates_escapes_and_drops_unknown() {
        let tag = BytesStart::new("img").with_attributes([("src", "../Images/a.jpg")]);
        let images = HashMap::from([(0, tag)]);
        let output = replace_image_markers("a\n\\[\\[IMG: 0\\]\\]\n[[IMG:7]]\nb", images).unwrap();
        assert!(output.contains("a.jpg"));
        assert!(!output.contains("IMG"));
    }
}
