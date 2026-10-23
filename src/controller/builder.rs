use crate::{
    controller::{
        CSS_MIME, DEFAULT_STYLESHEET, PageLinks, ResourcePaths, TOC_PAGE_PREFIX, TextRun,
        content_sections, get_ordered_path, html_to_markdown, image_anchors, image_marker_indices,
        insert_anchors, markdown_sections, parse_headers, parse_links, replace_image_markers,
        strip_jp_layout, strip_syosetu_tags, strip_tags, to_html, update_image_paths,
        update_resource_paths, write_body, write_header,
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
    ebook::spine::PageDirection,
    epub::{
        EpubChapter,
        manifest::{DetachedEpubManifestEntry, EpubManifestEntry},
        metadata::DetachedEpubMetaEntry,
    },
};
use regex::{Captures, Regex};
use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    mem,
    path::{Path, PathBuf},
    sync::LazyLock,
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
        self.config_metadata();

        let paths = self.get_resources();
        self.add_cover_image(&paths);
        self.add_chapters(&paths)?;

        let content = self.builder.write().to_vec()?;

        Ok((content, self.name))
    }

    pub fn config_metadata(&mut self) {
        let title = mem::take(&mut self.metadata.title);
        let source_id = self
            .epub
            .metadata()
            .identifier()
            .map(|e| e.value().to_string());

        let identifier = match &source_id {
            Some(id) => format!("urn:light-novel-translator:en:{id}"),
            None => format!("urn:light-novel-translator:{title}"),
        };
        if let Some(id) = source_id {
            self.builder
                .edit()
                .meta(DetachedEpubMetaEntry::dublin_core("dc:source").value(id));
        }

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
            .page_direction(PageDirection::LeftToRight)
            .modified_now()
            .resource(("stylesheet.css", DEFAULT_STYLESHEET));
    }

    pub fn get_resources(&mut self) -> ResourcePaths {
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

        paths
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

    pub fn add_chapters(&mut self, paths: &ResourcePaths) -> Result<()> {
        let epub_paths = get_ordered_path(&self.epub);
        let path_map = self.path_map();

        let file_parts: Vec<_> = epub_paths
            .iter()
            .filter_map(|path| link_files(path, &path_map))
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

        let mut chapter_count = 0;
        for (page, (href, md_file, source, html)) in file_parts.into_iter().enumerate() {
            let md_name = md_file.file_name().unwrap_or_default();
            let links = PageLinks {
                page: &source,
                paths,
            };
            let file_name = href
                .file_name()
                .ok_or(Error::BuildError("Invalid file name found"))?
                .to_string_lossy();

            let html = match pages.get(md_name) {
                Some(FormatPage { content, .. }) => {
                    let titles = chapters
                        .get(file_name.as_ref())
                        .map_or_default(|links| translated_titles(links));
                    build_html(&html, content, &toc_ids, &titles, links, page)?
                }
                None => {
                    let html = update_image_paths(&html, links)?;
                    update_resource_paths(&html, links)?
                }
            };

            let href = href.to_string_lossy();

            let mut sub_chapters = parse_headers(&html)?;
            contents.push(EpubChapter::unlisted(href).xhtml(html));
            let Some(links) = chapters.remove(file_name.as_ref()) else {
                contents.extend(sub_chapters.remove("").unwrap_or_default().into_iter().map(
                    |(id, title)| EpubChapter::new(title).href(format!("Text/{file_name}#{id}")),
                ));
                continue;
            };

            for TocLink { path, title, .. } in links {
                chapter_count += 1;
                let title = title.unwrap_or(format!("Chapter {}", chapter_count));
                contents.push(EpubChapter::new(title).href(format!("Text/{path}")));
                let fragment = path.rsplit_once('#').map_or_default(|(_, f)| f);
                let sub_chapters = sub_chapters.remove(fragment).unwrap_or_default();
                contents.extend(sub_chapters.into_iter().map(|(id, title)| {
                    EpubChapter::new(title).href(format!("Text/{file_name}#{id}"))
                }));
            }
        }

        self.builder.edit().chapter(contents);

        Ok(())
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
            // Kept percent-encoded to match the written chapter hrefs and the parsed links
            let file = href.name().to_string();
            let path = href
                .fragment()
                .map_or(file.clone(), |f| format!("{}#{}", file, f));
            let title = links.remove(&path);
            let link = TocLink {
                path,
                title,
                label: page.label().to_string(),
            };

            chapters.entry(file).or_default().push(link);
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
            .position(|p| p.path.file_prefix().is_some_and(|p| p == TOC_PAGE_PREFIX))?;

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

fn find_line(
    sections: &[Vec<&str>],
    text: &str,
    (start_section, start_line): (usize, usize),
) -> Option<(usize, usize, f64)> {
    sections
        .iter()
        .enumerate()
        .skip(start_section)
        .find_map(|(section, lines)| {
            let skip = if section == start_section {
                start_line
            } else {
                0
            };
            let line = lines.iter().skip(skip).position(|l| l.contains(text))? + skip;
            Some((section, line, line as f64 / lines.len() as f64))
        })
}

pub fn id_positions(html: &str, toc_ids: &[String]) -> Result<Vec<AnchorPosition>> {
    let sections = section_markdown(html)?;
    let sections: Vec<Vec<_>> = sections.iter().map(|s| s.lines().collect()).collect();
    let html = strip_tags(&strip_syosetu_tags(html)?)?;

    let mut cursor = (0, 0);
    let positions = match_ids(&html, toc_ids)?
        .into_iter()
        .filter_map(|(id, text)| {
            let found = find_line(&sections, &text, cursor);
            if let Some((section, line, _)) = found {
                cursor = (section, line + 1);
            }
            let (section, line, position) =
                found.or_else(|| find_line(&sections, &text, (0, 0)))?;
            Some(AnchorPosition {
                anchor: Anchor::Id(id),
                section,
                line,
                position,
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
            let found = text.and_then(|text| find_line(&sections, &text, (0, 0)));
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
    let ids: HashSet<_> = toc_ids.iter().map(|e| e.as_str()).collect();

    let mut pairs = Vec::new();
    let mut pending = Vec::new();
    let mut run = TextRun::default();
    loop {
        let event = reader.read_event()?;
        if run.push(&event)? {
            continue;
        }
        if let Some(text) = run.finish()
            && !pending.is_empty()
        {
            pairs.extend(pending.drain(..).map(|id| (id, text.clone())));
        }

        match event {
            Event::Start(tag) => {
                if let Some(attr) = tag.try_get_attribute("id")?
                    && ids.contains(attr.value.as_ref())
                {
                    let id = attr.normalized_value(XmlVersion::Implicit1_0)?.to_string();
                    let text = reader.read_text(tag.name())?.into_inner();
                    match next_text(text.as_ref())? {
                        Some(text) => pairs.push((id, text)),
                        None => pending.push(id),
                    }
                }
            }
            Event::Empty(tag) => {
                if let Some(attr) = tag.try_get_attribute("id")?
                    && ids.contains(attr.value.as_ref())
                {
                    pending.push(attr.normalized_value(XmlVersion::Implicit1_0)?.to_string());
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }

    Ok(pairs)
}

fn next_text(text: &str) -> Result<Option<String>> {
    let mut reader = Reader::from_str(text);
    let mut run = TextRun::default();

    loop {
        let event = reader.read_event()?;
        if run.push(&event)? {
            continue;
        }
        if let Some(text) = run.finish() {
            return Ok(Some(text));
        }
        if let Event::Eof = event {
            return Ok(None);
        }
    }
}

fn read_resources(entries: Vec<(EpubManifestEntry<'_>, String)>) -> Vec<DetachedEpubManifestEntry> {
    entries
        .into_par_iter()
        .filter_map(|(entry, path)| {
            let Ok(content) = entry.read_bytes() else {
                log::warn!("Skipping unreadable resource: {}", entry.href());
                return None;
            };
            let content = if entry.media_type() == CSS_MIME
                && let Ok(css) = str::from_utf8(&content)
            {
                strip_jp_layout(css).into_bytes()
            } else {
                content
            };

            Some(DetachedEpubManifestEntry::from((path, content)).media_type(entry.media_type()))
        })
        .collect()
}

fn link_files<'a>(
    path: &Path,
    path_map: &'a HashMap<String, EpubManifestEntry<'a>>,
) -> Option<(PathBuf, &'a EpubManifestEntry<'a>)> {
    let file_stem = path.file_stem()?;
    let md_file = PathBuf::from(file_stem).with_extension("md");

    let xhtml_file = path_map.get(file_stem.to_string_lossy().as_ref())?;
    Some((md_file, xhtml_file))
}

fn to_text_path(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or(Error::BuildError("Invalid file name found"))?;
    Ok(PathBuf::from("Text").join(file_name))
}

/// Maps each toc fragment of a page to its translated title.
fn translated_titles(links: &[TocLink]) -> HashMap<String, String> {
    links
        .iter()
        .filter_map(|link| {
            let (_, fragment) = link.path.rsplit_once('#')?;
            Some((fragment.to_string(), link.title.clone()?))
        })
        .collect()
}

/// Lowercased words, with a trailing plural `s` dropped so "Flowers" matches "Flower's".
fn title_words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let w = w.to_lowercase();
            match w.strip_suffix('s') {
                Some(stem) if stem.len() > 2 => stem.to_string(),
                _ => w,
            }
        })
        .filter(|w| w != "s")
        .collect()
}

/// Dice coefficient of the word sets, so long prose lines that mention the title score low.
fn title_similarity(title: &HashSet<String>, line: &str) -> f64 {
    let line = title_words(line);
    if title.is_empty() || line.is_empty() {
        return 0.0;
    }
    let shared = title.intersection(&line).count();
    2.0 * shared as f64 / (title.len() + line.len()) as f64
}

const TITLE_MATCH: f64 = 0.6;

/// A short line without sentence punctuation, like a chapter title, or a markdown heading.
fn looks_like_heading(line: &str) -> bool {
    let line = line.trim();
    if line.starts_with('#') {
        return true;
    }
    let opens_like_prose = line.starts_with(['"', '“', '‘', '\'', '(', '「', '『', '<', '[']);
    let ends_like_prose = line.ends_with([
        '.', '!', '?', '"', '”', '’', '\'', '…', '—', '–', ')', ':', ',', ';', '」', '』',
    ]);
    line.chars().any(char::is_alphabetic)
        && line.split_whitespace().count() <= 20
        && !opens_like_prose
        && !ends_like_prose
}

/// Index of the line from `start` on that best matches `title`, earliest on ties.
fn find_title(lines: &[TranslatedLine<'_>], title: &str, start: usize) -> Option<usize> {
    let title = title_words(title);
    let mut best = None;
    let mut best_score = TITLE_MATCH;
    for (i, line) in lines.iter().enumerate().skip(start) {
        let score = title_similarity(&title, line.text);
        if score > best_score || (best.is_none() && score == best_score) {
            best = Some(i);
            best_score = score;
        }
    }
    best
}

/// How far, in lines, a heading may be from the estimated position and still be used.
const HEADING_WINDOW: usize = 120;

/// Index of the heading from `start` on that is closest to `estimate`.
fn find_heading(lines: &[TranslatedLine<'_>], start: usize, estimate: usize) -> Option<usize> {
    let from = start.max(estimate.saturating_sub(HEADING_WINDOW));
    let to = lines.len().min(estimate + HEADING_WINDOW + 1);
    (from..to)
        .filter(|&i| looks_like_heading(lines[i].text))
        .min_by_key(|&i| i.abs_diff(estimate))
}

struct TranslatedLine<'a> {
    section: usize,
    line: usize,
    text: &'a str,
}

/// Places the toc ids in the translated content, in document order. An id goes on the
/// line matching its translated toc title, else on the heading nearest its estimated
/// position, else on the estimate. Estimates use the share of non-blank lines before the
/// id in the whole source page, since translation merges and splits paragraphs and the
/// source may no longer split into the same sections it was translated in.
pub fn place_ids(
    html: &str,
    content: &str,
    toc_ids: &[String],
    titles: &HashMap<String, String>,
) -> Result<Vec<AnchorPosition>> {
    let positions = id_positions(html, toc_ids)?;
    let sections = content_sections(content).collect::<Vec<_>>();
    let lines: Vec<_> = sections
        .iter()
        .enumerate()
        .flat_map(|(section, text)| {
            text.lines()
                .enumerate()
                .map(move |(line, text)| TranslatedLine {
                    section,
                    line,
                    text,
                })
        })
        .collect();
    let filled: Vec<_> = (0..lines.len())
        .filter(|&i| !lines[i].text.trim().is_empty())
        .collect();
    if filled.is_empty() {
        return Ok(positions);
    }

    let source = section_markdown(html)?;
    let is_filled = |l: &&str| !l.trim().is_empty();
    let offsets: Vec<_> = source
        .iter()
        .scan(0, |total, s| {
            let offset = *total;
            *total += s.lines().filter(is_filled).count();
            Some(offset)
        })
        .collect();
    let source_total = source
        .iter()
        .map(|s| s.lines().filter(is_filled).count())
        .sum::<usize>();
    let section_lens: Vec<_> = sections.iter().map(|s| s.lines().count()).collect();

    let mut cursor = 0;
    let positions = positions
        .into_iter()
        .map(|position| {
            let Anchor::Id(id) = &position.anchor else {
                return position;
            };
            let before = source
                .get(position.section)
                .map_or_default(|s| s.lines().take(position.line).filter(is_filled).count());
            let share = (offsets.get(position.section).unwrap_or(&0) + before) as f64
                / source_total.max(1) as f64;
            let estimate = filled[((share * filled.len() as f64) as usize).min(filled.len() - 1)];

            let found = titles
                .get(id)
                .and_then(|title| find_title(&lines, title, cursor))
                .or_else(|| find_heading(&lines, cursor, estimate));
            if let Some(i) = found {
                cursor = i + 1;
            }
            let TranslatedLine { section, line, .. } = lines[found.unwrap_or(estimate)];
            AnchorPosition {
                position: line as f64 / section_lens[section] as f64,
                section,
                line,
                ..position
            }
        })
        .collect();

    Ok(positions)
}

pub fn build_html(
    html: &str,
    content: &str,
    toc_ids: &[String],
    titles: &HashMap<String, String>,
    links: PageLinks<'_>,
    page: usize,
) -> Result<String> {
    let found = image_marker_indices(content);
    let (marked, estimated): (Vec<_>, Vec<_>) = image_anchors(html, links)?
        .into_iter()
        .enumerate()
        .partition(|(i, _)| found.contains(i));

    let mut anchors = place_ids(html, content, toc_ids, titles)?;
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
        .with_attribute(("xmlns", "http://www.w3.org/1999/xhtml"))
        .with_attribute(("xmlns:epub", "http://www.idpf.org/2007/ops"))
        .with_attribute(("xml:lang", "en"))
        .write_inner_content(|writer| {
            write_header(writer, html, links)?;
            write_body(writer, &content, page)
        })?;

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

/// Full-width spaces at the start of a line are Japanese paragraph indents.
static INDENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\u{3000}+").unwrap());
static DASH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[―—─━]+").unwrap());
static ELLIPSIS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"…+|・{2,}").unwrap());
static COMMA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"、[ \u{3000}]?").unwrap());
/// Longer dash runs are scene separators rather than em dashes.
const MAX_DASH_RUN: usize = 3;

pub fn replace_jp_symbols(text: &str) -> String {
    let text = INDENT_RE.replace_all(text, "");
    let text = DASH_RE.replace_all(&text, |caps: &Captures| match caps[0].chars().count() {
        n if n <= MAX_DASH_RUN => "—".to_string(),
        _ => caps[0].to_string(),
    });
    let text = ELLIPSIS_RE.replace_all(&text, "…");
    let text = COMMA_RE.replace_all(&text, ", ");

    text.chars()
        .map(|c| match c {
            '「' | '」' => '"',
            '『' | '』' => '\'',
            '\u{3000}' => ' ',
            '。' => '.',
            '〜' => '~',
            '！'
            | '？'
            | '（'
            | '）'
            | '，'
            | '．'
            | '：'
            | '；'
            | '～'
            | '０'..='９'
            | 'Ａ'..='Ｚ'
            | 'ａ'..='ｚ' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            c => c,
        })
        .collect()
}

#[derive(Debug)]
pub struct TocLink {
    pub path: String,
    pub title: Option<String>,
    pub label: String,
}

#[cfg(test)]
mod test {
    use super::{
        Anchor, id_positions, looks_like_heading, match_ids, place_ids, replace_jp_symbols,
    };
    use std::collections::HashMap;

    #[test]
    fn matches_ids_on_empty_anchors() {
        let html = r#"<body><p><a id="toc-003"/><span><span>プロローグ</span></span></p><p><a id="toc-002"/>　<span>剣と魔法</span></p><h1 id="toc-004">＃１</h1></body>"#;
        let ids = ["toc-002", "toc-003", "toc-004"].map(String::from);
        assert_eq!(
            match_ids(html, &ids).unwrap(),
            [
                ("toc-003", "プロローグ"),
                ("toc-002", "剣と魔法"),
                ("toc-004", "＃１")
            ]
            .map(|(id, text)| (id.to_string(), text.to_string()))
        );
    }

    #[test]
    fn places_repeated_titles_once_in_order() {
        let html = r#"<html><head></head><body><p id="toc-1">閑話</p><p>本文</p><p id="toc-2">見送り</p><p>本文</p><p id="toc-3">閑話</p><p>本文</p></body></html>"#;
        let ids = ["toc-1", "toc-2", "toc-3"].map(String::from);
        let positions: Vec<_> = id_positions(html, &ids)
            .unwrap()
            .into_iter()
            .map(|p| match p.anchor {
                Anchor::Id(id) => (id, p.section, p.line),
                Anchor::Image(_) => unreachable!(),
            })
            .collect();
        assert_eq!(
            positions,
            [("toc-1", 0, 0), ("toc-2", 0, 2), ("toc-3", 0, 4)].map(|(id, section, line)| (
                id.to_string(),
                section,
                line
            ))
        );
    }

    #[test]
    fn places_ids_on_translated_headings() {
        let html = r#"<html><head></head><body><p id="c1">第一章</p><p>あ。</p><p>い。</p><p>う。</p><p id="c2">第二章</p><p>え。</p></body></html>"#;
        // The translation split paragraphs, so the source position of c2 lands on prose
        let content = "<part>1</part>\n\nChapter One\n\nA.\n\nB.\n\nC.\n\nD.\n\nE.\n\nF.\n\nThe Second Chapter\n\nG.\n";
        let ids = ["c1", "c2"].map(String::from);
        let place = |titles: &HashMap<String, String>| -> Vec<_> {
            place_ids(html, content, &ids, titles)
                .unwrap()
                .into_iter()
                .map(|p| (p.section, p.line))
                .collect()
        };

        // Without titles, the nearest heading-like line is used
        assert_eq!(place(&HashMap::new()), [(0, 2), (0, 16)]);
        // A loosely matching translated toc title wins over the estimate
        let titles = HashMap::from([("c2".to_string(), "Second Chapters".to_string())]);
        assert_eq!(place(&titles), [(0, 2), (0, 16)]);
    }

    #[test]
    fn detects_headings() {
        assert!(looks_like_heading(
            "Side Story: The Witch of Ordine and the Black Fan"
        ));
        assert!(looks_like_heading("## Chapter 3"));
        assert!(!looks_like_heading("He bowed and said to her:"));
        assert!(!looks_like_heading("“Alright, I’m off! Fabiola!”"));
        assert!(!looks_like_heading("The time goes back slightly—"));
        assert!(!looks_like_heading("◆◆◆"));
    }

    #[test]
    fn matches_ids_with_entities() {
        let html =
            r#"<body><h2 id="c1">A &amp; B&#x3042; Saga</h2><p><a id="c2"/>Tom &lt;3</p></body>"#;
        let ids = ["c1", "c2"].map(String::from);
        assert_eq!(
            match_ids(html, &ids).unwrap(),
            [("c1", "A & Bあ Saga"), ("c2", "Tom <3")]
                .map(|(id, text)| (id.to_string(), text.to_string()))
        );
    }

    #[test]
    fn converts_quotes() {
        assert_eq!(
            replace_jp_symbols("「She said 『no』」"),
            "\"She said 'no'\""
        );
    }

    #[test]
    fn converts_dashes_and_ellipses() {
        assert_eq!(replace_jp_symbols("Wait――no……"), "Wait—no…");
        assert_eq!(replace_jp_symbols("Well・・・"), "Well…");
        // Long runs are separators and stay as they are
        assert_eq!(replace_jp_symbols("――――――"), "――――――");
    }

    #[test]
    fn converts_full_width_punctuation() {
        assert_eq!(
            replace_jp_symbols("\u{3000}What！？ Level ５０、ok。"),
            "What!? Level 50, ok."
        );
        assert_eq!(replace_jp_symbols("Hey～ 〜"), "Hey~ ~");
    }

    #[test]
    fn keeps_markdown_and_html_characters() {
        assert_eq!(replace_jp_symbols("＊＃＜＞＆"), "＊＃＜＞＆");
    }
}
