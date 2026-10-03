use percent_encoding::percent_decode_str;
use rbook::ebook::element::Href;
use std::collections::{HashMap, HashSet};

/// Output locations for the source EPUB's images, styles and scripts.
///
/// Resources are flattened into one folder per kind, so files that share a
/// name in different source directories get a numbered suffix (`001-2.jpg`).
#[derive(Debug, Default)]
pub struct ResourcePaths {
    paths: HashMap<String, String>,
    used: HashSet<String>,
}

impl ResourcePaths {
    /// Reserves a unique path in `folder` for the resource at `href` and returns it.
    pub fn insert(&mut self, href: Href<'_>, folder: &str) -> String {
        let source = href.path().decode().into_owned();
        self.insert_source(source, &href.name().decode(), folder)
    }

    fn insert_source(&mut self, source: String, name: &str, folder: &str) -> String {
        if let Some(path) = self.paths.get(&source) {
            return path.clone();
        }

        let (stem, ext) = match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => (stem, format!(".{ext}")),
            _ => (name, String::new()),
        };

        let path = (1..)
            .map(|n| match n {
                1 => format!("{folder}/{stem}{ext}"),
                n => format!("{folder}/{stem}-{n}{ext}"),
            })
            .find(|path| !self.used.contains(path))
            .unwrap_or_default();

        self.used.insert(path.clone());
        self.paths.insert(source, path.clone());
        path
    }

    pub fn get(&self, href: Href<'_>) -> Option<&str> {
        self.paths
            .get(href.path().decode().as_ref())
            .map(String::as_str)
    }

    /// Resolves `link` as written in the source page at `page` and returns
    /// the output path, relative to the output `Text` folder.
    pub fn link(&self, page: &str, link: &str) -> Option<String> {
        let link = link.split(['#', '?']).next()?;
        let link = percent_decode_str(link).decode_utf8_lossy();

        let mut segments: Vec<&str> = match link.starts_with('/') {
            true => vec![],
            false => page
                .rsplit_once('/')
                .map_or("", |(dir, _)| dir)
                .split('/')
                .collect(),
        };
        for segment in link.split('/') {
            match segment {
                "" | "." => (),
                ".." => _ = segments.pop(),
                segment => segments.push(segment),
            }
        }
        segments.retain(|segment| !segment.is_empty());

        let source = format!("/{}", segments.join("/"));
        self.paths.get(&source).map(|path| format!("../{path}"))
    }
}

/// A source page together with the resource paths its links resolve against.
#[derive(Debug, Clone, Copy)]
pub struct PageLinks<'a> {
    pub page: &'a str,
    pub paths: &'a ResourcePaths,
}

impl PageLinks<'_> {
    /// The output path for `link`, falling back to its file name in `folder`
    /// when it isn't a known resource.
    pub fn resolve(&self, link: &str, folder: &str) -> String {
        self.paths.link(self.page, link).unwrap_or_else(|| {
            let name = link.rsplit('/').next().unwrap_or(link);
            format!("{folder}/{name}")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(sources: &[&str]) -> ResourcePaths {
        let mut paths = ResourcePaths::default();
        for source in sources {
            let name = source.rsplit('/').next().unwrap();
            paths.insert_source(source.to_string(), name, "Images");
        }
        paths
    }

    #[test]
    fn suffixes_colliding_names() {
        let mut paths = paths(&["/a/001.jpg", "/b/001.jpg", "/c/001.jpg", "/d/README"]);
        assert_eq!(paths.paths["/a/001.jpg"], "Images/001.jpg");
        assert_eq!(paths.paths["/b/001.jpg"], "Images/001-2.jpg");
        assert_eq!(paths.paths["/c/001.jpg"], "Images/001-3.jpg");
        assert_eq!(paths.paths["/d/README"], "Images/README");
        // The same source keeps its path.
        assert_eq!(
            paths.insert_source("/b/001.jpg".into(), "001.jpg", "Images"),
            "Images/001-2.jpg"
        );
    }

    #[test]
    fn resolves_relative_to_page() {
        let paths = paths(&[
            "/OEBPS/img/ch1/001.jpg",
            "/OEBPS/img/ch2/001.jpg",
            "/OEBPS/img/表紙.jpg",
        ]);
        let page = "/OEBPS/text/ch2.xhtml";

        assert_eq!(
            paths.link(page, "../img/ch2/001.jpg").as_deref(),
            Some("../Images/001-2.jpg")
        );
        assert_eq!(
            paths.link(page, "../img/ch1/./001.jpg").as_deref(),
            Some("../Images/001.jpg")
        );
        assert_eq!(
            paths.link(page, "../img/%E8%A1%A8%E7%B4%99.jpg").as_deref(),
            Some("../Images/表紙.jpg")
        );
        assert_eq!(paths.link(page, "../img/missing.jpg"), None);
    }

    #[test]
    fn falls_back_to_file_name() {
        let paths = ResourcePaths::default();
        let links = PageLinks {
            page: "/OEBPS/text/ch1.xhtml",
            paths: &paths,
        };
        assert_eq!(
            links.resolve("../Style/a.css", "../Styles"),
            "../Styles/a.css"
        );
    }
}
