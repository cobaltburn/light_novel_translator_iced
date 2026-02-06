use crate::{
    actions::server_action::ServerAction,
    error::{Error, Result},
    message::display_error,
    model::{
        extraction::{Extraction, ImagePage, ImageView},
        server::Method,
    },
};
use iced::Task;
use rfd::FileHandle;
use tokio::fs;

pub const BATCH_SIZE: usize = 6;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ExtractAction {
    ServerAction(ServerAction),
    SelectImages,
    SetPage(usize),
    CheckToggle(usize),
    SetImages(Vec<ImagePage>),
    SetImageView(ImageView),
    ExtractText(usize),
    UpdateContent { content: String, page: usize },
    SaveText,
    PageComplete(usize),
}

impl Extraction {
    pub fn perform(&mut self, action: ExtractAction) -> Task<ExtractAction> {
        match action {
            ExtractAction::ServerAction(action) => {
                self.server_state.perform(action).map(Into::into)
            }
            ExtractAction::SetImages(images) => self.set_images(images).into(),
            ExtractAction::SetPage(page) => self.set_page(page).into(),
            ExtractAction::CheckToggle(page) => self.toggle_check(page).into(),
            ExtractAction::SetImageView(view) => self.set_view(view).into(),
            ExtractAction::PageComplete(page) => self.mark_compete(page).into(),
            ExtractAction::SaveText => self.save_text(),
            ExtractAction::UpdateContent { content, page } => {
                self.update_content(content, page).into()
            }
            ExtractAction::SelectImages => Task::future(select_folder())
                .and_then(|handle| Task::future(get_images(handle)))
                .then(|images| match images {
                    Ok(images) => Task::done(ExtractAction::SetImages(images)),
                    Err(error) => Task::future(display_error(error)).discard(),
                }),
            ExtractAction::ExtractText(page) => match self.extract(page) {
                Ok(task) => task,
                Err(error) => Task::future(display_error(error)).discard(),
            },
        }
    }

    fn toggle_check(&mut self, page: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            page.checked = !page.checked;
        }
    }

    fn set_view(&mut self, view: ImageView) {
        self.image_view = view;
    }

    fn set_images(&mut self, images: Vec<ImagePage>) {
        self.pages = images;
        self.current_page = 0;
    }

    fn set_page(&mut self, page: usize) {
        self.current_page = page;
    }

    fn mark_compete(&mut self, page: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            page.complete = true;
        }
    }

    fn update_content(&mut self, content: String, page: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            page.content.push_str(&content);
        }
    }

    fn save_text(&mut self) -> Task<ExtractAction> {
        let text: String = self
            .pages
            .iter()
            .filter(|e| e.checked)
            .filter(|e| !e.content.is_empty())
            .map(|e| format!("\n<page>{}</page>\n\n{}\n", e.name, e.content))
            .collect();

        Task::future(save_to_file(text)).then(|res| match res {
            Ok(_) => Task::none(),
            Err(error) => Task::future(display_error(error)).discard(),
        })
    }

    fn extract(&mut self, page: usize) -> Result<Task<ExtractAction>> {
        match self.server_state.method {
            Method::Chain => self.chain_extract(page),
            Method::Batch => self.batch_extract(page),
        }
    }

    fn chain_extract(&mut self, page: usize) -> Result<Task<ExtractAction>> {
        if !self.server_state.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server_state.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        let Some(current_page) = self.pages.get_mut(page) else {
            return Ok(Task::done(ServerAction::Abort.into()));
        };

        if !current_page.checked {
            return Ok(Task::done(ExtractAction::ExtractText(page + 1)));
        }

        current_page.content.clear();

        let client = self.server_state.client.clone();
        let image_base64 = current_page.base64.clone();
        let settings = self.server_state.settings.clone();

        let task = client.extract_text(model, image_base64, page, settings);
        let task = self.server_state.bind_handle(task);

        let next_task = self
            .server_state
            .bind_handle(Task::done(ExtractAction::ExtractText(page + 1)));

        Ok(task.chain(next_task))
    }

    fn batch_extract(&mut self, page: usize) -> Result<Task<ExtractAction>> {
        if !self.server_state.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server_state.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        if page > self.pages.len() {
            return Ok(Task::done(ServerAction::Abort.into()));
        }

        let client = self.server_state.client.clone();
        let settings = self.server_state.settings.clone();

        let pages = self
            .pages
            .iter_mut()
            .enumerate()
            .skip_while(|(i, _)| i != &page)
            .take(BATCH_SIZE);

        let tasks = pages
            .filter(|e| e.1.checked)
            .map(|(page, image)| {
                image.content.clear();
                image.complete = false;
                client.clone().extract_text(
                    model.clone(),
                    image.base64.clone(),
                    page,
                    settings.clone(),
                )
            })
            .map(|task| self.server_state.bind_handle(task));

        let task = Task::batch(tasks);

        let next_task = self
            .server_state
            .bind_handle(Task::done(ExtractAction::ExtractText(page + BATCH_SIZE)));

        Ok(task.chain(next_task))
    }
}

impl From<ServerAction> for ExtractAction {
    fn from(action: ServerAction) -> Self {
        ExtractAction::ServerAction(action)
    }
}

async fn select_folder() -> Option<FileHandle> {
    rfd::AsyncFileDialog::new()
        .set_title("select image folder")
        .pick_folder()
        .await
}

async fn get_images(handle: FileHandle) -> Result<Vec<ImagePage>> {
    let mut pages = Vec::new();
    let mut dirs = fs::read_dir(handle.path()).await?;

    while let Some(file) = dirs.next_entry().await? {
        let path = file.path();
        match path.extension().and_then(|e| e.to_str()) {
            Some("jpg" | "jpeg" | "png") => {
                let bytes = fs::read(&path).await?;
                let name = path.file_name().unwrap().to_string_lossy();
                pages.push(ImagePage::from_bytes(name.to_string(), bytes));
            }
            _ => (),
        };
    }

    Ok(pages)
}

async fn save_to_file(text: String) -> Result<()> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save extracted text")
        .set_file_name("extrated_text.md")
        .save_file()
        .await;
    if let Some(handle) = handle {
        handle.write(text.as_bytes()).await?;
    };

    Ok(())
}
