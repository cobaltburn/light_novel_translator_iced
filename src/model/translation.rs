use crate::{
    actions::trans_action::TransAction,
    model::{page::Page, server::Server},
    widget::page_sidebar::{SidebarAction, SidebarDeps, SidebarRow},
};
use iced_aw::TabLabel;
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Translation {
    pub server: Server,
    pub file_path: PathBuf,
    pub current_page: usize,
    pub pages: Vec<Page>,
}

impl Translation {
    pub fn tab_label(&self) -> TabLabel {
        match self.server.handles.is_empty() {
            true => TabLabel::Text(self.file_name()),
            false => TabLabel::IconText('\u{25CF}', self.file_name()),
        }
    }

    pub fn current_page(&self) -> Option<&Page> {
        self.pages.get(self.current_page)
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    pub fn sidebar_deps(&self) -> SidebarDeps {
        let active = self.server.connected() && self.server.handles.is_empty();
        let rows = self
            .pages
            .iter()
            .map(|p| SidebarRow {
                name: p
                    .path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                activity: p.activity.clone(),
                section_count: p.sections.len(),
            })
            .collect();
        SidebarDeps {
            current_page: self.current_page,
            active,
            rows,
        }
    }
}

impl SidebarAction for TransAction {
    fn set_page(page: usize) -> Self {
        TransAction::SetPage(page)
    }
    fn save_page(name: String, page: usize) -> Self {
        TransAction::SavePage { name, page }
    }
    fn translate(page: usize) -> Self {
        TransAction::Translate(page)
    }
    fn translate_page(page: usize) -> Self {
        TransAction::TranslatePage(page)
    }
    fn translate_part(page: usize, part: usize) -> Self {
        TransAction::TranslatePart { page, part }
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