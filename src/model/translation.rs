use crate::{
    actions::translation::Action,
    model::{Page, Server},
    view::DisplayType,
    widget::{SidebarAction, SidebarDeps, SidebarRow},
};
use iced_aw::TabLabel;
use std::{borrow::Cow, path::PathBuf};

#[derive(Default, Debug)]
pub struct Translation {
    pub server: Server,
    pub file_path: Option<PathBuf>,
    pub glossary: Option<(PathBuf, String)>,
    pub current_page: usize,
    pub pages: Vec<Page>,
    pub display: DisplayType,
}

impl Translation {
    pub fn tab_label(&self) -> TabLabel {
        let epub_name = self.epub_name().unwrap_or_default().into_owned();
        match self.server.handles.is_empty() {
            true => TabLabel::Text(epub_name),
            false => TabLabel::IconText('\u{25CF}', epub_name),
        }
    }

    pub fn current_page(&self) -> Option<&Page> {
        self.pages.get(self.current_page)
    }

    pub fn epub_name(&self) -> Option<Cow<'_, str>> {
        Some(self.file_path.as_ref()?.file_name()?.to_string_lossy())
    }

    pub fn glossary_name(&self) -> Option<Cow<'_, str>> {
        Some(self.glossary.as_ref()?.0.file_name()?.to_string_lossy())
    }
    pub fn sidebar_deps(&self) -> SidebarDeps {
        let active = self.server.connected() && self.server.handles.is_empty();
        let rows = self
            .pages
            .iter()
            .map(|p| SidebarRow {
                name: p.name(),
                activity: p.activity,
                section_count: p.sections.len(),
            })
            .collect();
        SidebarDeps {
            current_page: self.current_page,
            active,
            rows,
        }
    }

    pub fn can_translate(&self) -> bool {
        self.server.handles.is_empty() && self.server.connected() && self.file_path.is_some()
    }
}

impl SidebarAction for Action {
    fn set_page(page: usize) -> Self {
        Action::SetPage(page)
    }
    fn save_page(name: String, page: usize) -> Self {
        Action::SavePage { name, page }
    }
    fn translate(page: usize) -> Self {
        Action::Translate(page)
    }
    fn translate_page(page: usize) -> Self {
        Action::TranslatePage(page)
    }
    fn translate_part(page: usize, part: usize) -> Self {
        Action::TranslatePart { page, part }
    }
}

impl From<Server> for Translation {
    fn from(server: Server) -> Self {
        Self {
            server,
            ..Default::default()
        }
    }
}
