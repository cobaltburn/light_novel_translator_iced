use crate::{
    actions::{
        complete_dialog, contains_japanese, get_pages, pick_save_folder, save_file,
        server_action::ServerAction,
    },
    controller::{parse::remove_think_tags, part_tag},
    error::{Error, Result},
    message::{display_error, select_epub},
    model::{Activity, consensus::Consensus, page::Page},
};
use iced::Task;
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone)]
pub enum ConsensusAction {
    ServerAction(ServerAction),
    UpdateContent {
        content: String,
        page: usize,
        part: usize,
    },
    PageComplete(usize),
    Translate(usize),
    TranslatePage(usize),
    TranslatePart {
        page: usize,
        part: usize,
    },
    CancelTranslate,
    SaveTranslation(String),
    SavePage {
        name: String,
        page: usize,
    },
    SetPage(usize),
    SavePages(PathBuf),
    SetEpub {
        name: String,
        pages: Vec<Page>,
    },
    OpenEpub,
}

impl Consensus {
    pub fn perform(&mut self, action: ConsensusAction) -> Task<ConsensusAction> {
        match action {
            ConsensusAction::ServerAction(action) => self.server.perform(action).map(Into::into),
            ConsensusAction::Translate(_) => todo!(),
            ConsensusAction::TranslatePage(_) => todo!(),
            ConsensusAction::TranslatePart { page, part } => todo!(),
            ConsensusAction::SaveTranslation(file_name) => {
                Task::future(pick_save_folder(file_name))
                    .and_then(|path| {
                        Task::future(async { fs::create_dir(&path).await.map(|_| path) })
                    })
                    .then(|path| match path {
                        Ok(path) => Task::done(ConsensusAction::SavePages(path).into()),
                        Err(err) => Task::future(display_error(err)).discard(),
                    })
            }
            ConsensusAction::SavePages(path) => self.save_pages(path),
            ConsensusAction::SavePage { name, page } => self.save_page(name, page),
            ConsensusAction::UpdateContent {
                content,
                page,
                part,
            } => self.update_content(content, page, part).into(),
            ConsensusAction::PageComplete(page) => self.check_complete(page).into(),
            ConsensusAction::SetEpub { name, pages } => self.set_epub(name, pages).into(),
            ConsensusAction::OpenEpub => Task::future(select_epub())
                .and_then(|(name, buffer)| Task::future(get_pages(name, buffer)))
                .then(|doc| match doc {
                    Ok((name, pages)) => Task::done(ConsensusAction::SetEpub { name, pages }),
                    Err(error) => Task::future(display_error(error)).discard(),
                }),
            ConsensusAction::CancelTranslate => self.cancel().into(),
            ConsensusAction::SetPage(page) => self.set_page(page).into(),
        }
    }

    pub fn translate(&mut self, page: usize) -> Result<Task<ConsensusAction>> {
        if !self.server.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        let Some(pages) = self.pages.get_mut(..page + 1) else {
            let file_name = self.file_name.clone();
            self.server.abort();
            return Ok(
                Task::future(complete_dialog(file_name.clone())).then(move |x| match x {
                    true => Task::done(ConsensusAction::SaveTranslation(file_name.to_owned())),
                    false => Task::none(),
                }),
            );
        };

        let current_page = pages.last_mut().unwrap();
        current_page.activity = Activity::Active;
        current_page.clear_content();

        let server = &mut self.server;
        // let task = server.translation(pages, &model, page)?;

        let complete_task = server.bind_handle(Task::done(ConsensusAction::PageComplete(page)));
        let next_task = server.bind_handle(Task::done(ConsensusAction::Translate(page + 1)));

        todo!()
        // Ok(task.chain(complete_task).chain(next_task))
    }

    fn set_page(&mut self, page: usize) {
        self.current_page = page
    }

    fn cancel(&mut self) {
        self.pages
            .iter_mut()
            .filter(|p| matches!(p.activity, Activity::Active))
            .for_each(|p| p.activity = Activity::Incomplete);
        self.server.abort();
    }

    pub fn set_epub(&mut self, name: String, pages: Vec<Page>) {
        self.current_page = 0;
        self.file_name = name;
        self.pages = pages;
    }

    fn check_complete(&mut self, page: usize) {
        let Some(page) = self.pages.get_mut(page) else {
            return;
        };

        page.activity = if page.sections.iter().any(|e| e.content.is_empty()) {
            Activity::Incomplete
        } else if let Some(i) = page
            .sections
            .iter()
            .position(|e| contains_japanese(&e.content))
        {
            Activity::Error(i + 1)
        } else {
            Activity::Complete
        };
    }

    pub fn update_content(&mut self, content: String, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            if let Some(section) = page.sections.get_mut(part) {
                section.content.push_str(&content);
            };
        };
    }

    pub fn save_pages(&mut self, path: PathBuf) -> Task<ConsensusAction> {
        let tasks = self
            .pages
            .iter()
            .map(|page| {
                let name = page.path.file_stem().unwrap().to_os_string();
                let text: String = page
                    .sections
                    .iter()
                    .enumerate()
                    .map(|(i, e)| format!("{}{}\n", part_tag(i + 1), e.content))
                    .collect();
                (name, remove_think_tags(&text))
            })
            .map(|(name, contents)| {
                let file_path = path.join(name).with_extension("md");
                Task::future(fs::write(file_path, contents)).then(|r| match r {
                    Ok(_) => Task::none(),
                    Err(error) => Task::future(display_error(error)),
                })
            });
        Task::batch(tasks).discard()
    }

    pub fn save_page(&mut self, name: String, page: usize) -> Task<ConsensusAction> {
        match self.pages.get(page) {
            Some(page) => {
                let content: String = page
                    .sections
                    .iter()
                    .enumerate()
                    .map(|(i, e)| format!("{}{}\n", part_tag(i + 1), e.content))
                    .collect();

                let name = format!("{name}.md");

                Task::future(save_file(name, content)).discard()
            }
            None => Task::none(),
        }
    }
}

impl From<ServerAction> for ConsensusAction {
    fn from(action: ServerAction) -> Self {
        ConsensusAction::ServerAction(action)
    }
}
