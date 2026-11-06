use crate::page::{PageError, Section};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub const TOC_PAGE_PREFIX: &str = "__toc__";

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub enum Activity {
    #[default]
    Incomplete,
    Complete,
    Active(usize),
    Error(usize),
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Page {
    pub path: PathBuf,
    pub sections: Vec<Section>,
    #[serde(skip)]
    pub activity: Activity,
    #[serde(skip)]
    pub errors: Vec<PageError>,
}

impl Page {
    pub fn new(path: PathBuf, sections: Vec<String>) -> Self {
        let sections = sections.into_iter().map(Section::new).collect();
        Page {
            path: absolute_page_path(path),
            sections,
            ..Default::default()
        }
    }

    pub fn clear(&mut self) {
        self.sections.iter_mut().for_each(|e| e.content.clear());
        self.errors.clear();
    }

    pub fn file_prefix(&self) -> Option<&OsStr> {
        self.path.file_prefix()
    }

    pub fn name(&self) -> String {
        self.file_prefix()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    pub fn active(&self) -> bool {
        matches!(self.activity, Activity::Active(_))
    }
}

pub fn absolute_page_path(path: PathBuf) -> PathBuf {
    if path.is_relative() {
        Path::new("/").join(path)
    } else {
        path
    }
}

#[cfg(test)]
mod test {
    use super::{Page, absolute_page_path};
    use std::path::PathBuf;

    #[test]
    fn page_paths_are_absolute() {
        let toc = Page::new(PathBuf::from("__toc__.xhtml"), Vec::new());
        assert_eq!(toc.path, PathBuf::from("/__toc__.xhtml"));
        assert_eq!(
            absolute_page_path("/OEBPS/text/p-001.xhtml".into()),
            PathBuf::from("/OEBPS/text/p-001.xhtml")
        );
    }
}
