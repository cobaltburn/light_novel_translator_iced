use crate::{
    actions::trans_action::TransAction, controller::xml::part_tag, state::server_state::ServerState,
};
use iced::Task;
use iced_aw::TabLabel;
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct TranslationModel {
    pub server_state: ServerState,
    pub file_name: String,
    pub current_page: Option<usize>,
    pub pages: Vec<Page>,
    pub method: Method,
}

impl TranslationModel {
    pub fn tab_label(&self) -> TabLabel {
        TabLabel::Text(self.file_name.clone())
    }

    pub fn current_content(&self) -> Option<String> {
        Some(self.pages.get(self.current_page?)?.text.join(""))
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Page {
    pub path: PathBuf,
    pub text: Vec<String>,
    pub sections: Vec<String>,
    pub complete: bool,
}

impl Page {
    pub fn new(path: PathBuf, sections: Vec<String>) -> Self {
        let text = (1..=sections.len()).into_iter().map(part_tag).collect();
        Page {
            text,
            complete: sections.is_empty(),
            path,
            sections,
        }
    }

    pub fn clear_content(&mut self) {
        self.text.iter_mut().enumerate().for_each(|(i, x)| {
            x.clear();
            x.push_str(&part_tag(i + 1));
        });
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Method {
    #[default]
    Chain,
    Batch,
}

impl Method {
    pub fn join_tasks(
        &self,
        tasks: impl IntoIterator<Item = Task<TransAction>>,
    ) -> Task<TransAction> {
        match self {
            Method::Batch => Task::batch(tasks),
            Method::Chain => {
                let mut chain = Task::none();
                for task in tasks {
                    chain = chain.chain(task);
                }
                chain
            }
        }
    }
}
