use crate::model::Activity;
use std::path::PathBuf;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Page {
    pub path: PathBuf,
    pub sections: Vec<Section>,
    pub activity: Activity,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub japanese: String,
    pub text: String,
}

impl Page {
    pub fn new(path: PathBuf, sections: Vec<String>) -> Self {
        let sections = sections
            .into_iter()
            .map(|japanese| Section {
                japanese,
                text: String::new(),
            })
            .collect();

        Page {
            activity: Activity::Incomplete,
            path,
            sections,
        }
    }

    pub fn clear_content(&mut self) {
        self.sections.iter_mut().for_each(|e| e.text.clear());
    }
}
