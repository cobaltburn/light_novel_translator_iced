use crate::{
    epub::{
        chapter::{build_html, parse_headers},
        html::{parse_links, update_image_paths, update_resource_paths},
        resources::{CSS_MIME, PageLinks, ResourcePaths, strip_jp_layout},
        stylesheet::DEFAULT_STYLESHEET,
    },
    error::{Error, Result},
    page::TOC_PAGE_PREFIX,
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
use std::{
    collections::HashMap,
    mem,
    path::{Path, PathBuf},
};

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct FormatPage {
    pub path: PathBuf,
    pub content: String,
}

impl From<(PathBuf, String)> for FormatPage {
    fn from((path, content): (PathBuf, String)) -> Self {
        FormatPage { path, content }
    }
}

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct EpubMetadata {
    pub title: String,
    pub authors: String,
}

#[derive(Debug)]
pub struct TocLink {
    pub path: String,
    pub title: Option<String>,
}

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

            for TocLink { path, title } in links {
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
            let link = TocLink { path, title };

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

pub fn get_ordered_path(epub: &Epub) -> Vec<PathBuf> {
    epub.spine()
        .iter()
        .filter_map(|e| e.manifest_entry())
        .map(|e| e.href().decode().as_ref().into())
        .collect()
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
