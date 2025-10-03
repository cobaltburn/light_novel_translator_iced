use crate::{controller::server::Server, message::Message};
use iced::{Task, task::Handle};
use ollama_rs::{Ollama, generation::chat::ChatMessage};
use std::sync::{Arc, Mutex};

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct ServerState {
    pub server: Server,
    pub models: Vec<String>,
    pub current_model: Option<String>,
    pub url: String,
    pub invalid_url: bool,
    pub history: Arc<Mutex<Vec<ChatMessage>>>,
    pub handles: Vec<Handle>,
}

impl ServerState {
    pub fn connected(&self) -> bool {
        self.server.connected()
    }
}

impl ServerState {
    pub fn connect_server(&mut self, url: Option<String>) -> Task<Message> {
        let ollama = match url {
            None => Ollama::default(),
            Some(url) => match Ollama::try_new(url) {
                Ok(ollama) => ollama,
                Err(error) => {
                    log::error!("url parse error: {}", error);
                    self.invalid_url = true;
                    return Task::none();
                }
            },
        };
        self.server = Server::Connected(ollama);

        Task::future(self.server.clone().get_models())
            .and_then(|models| Task::done(ServerAction::SetModels(models).into()))
    }

    pub fn set_models(&mut self, models: Vec<String>) -> Task<Message> {
        self.current_model = models.first().cloned();
        self.models = models;
        Task::none()
    }

    pub fn set_current_model(&mut self, model: String) -> Task<Message> {
        self.current_model = Some(model);
        Task::none()
    }

    pub fn edit_url(&mut self, url: String) -> Task<Message> {
        self.url = url;
        self.invalid_url = false;
        Task::none()
    }

    pub fn abort(&mut self) -> Task<Message> {
        self.handles.iter().for_each(|handle| handle.abort());
        self.handles.clear();
        self.clear_history();
        Task::none()
    }

    pub fn clear_history(&mut self) {
        let mut history = self.history.lock().unwrap();
        history.clear();
    }

    pub fn init_history(&mut self, context: Option<String>) {
        let mut history = self.history.lock().unwrap();
        if history.is_empty() {
            let system_message = generate_system_message(&context);
            history.push(system_message);
        }
    }

    pub fn perform(&mut self, action: ServerAction) -> Task<Message> {
        match action {
            ServerAction::SelectModel(model) => self.set_current_model(model),
            ServerAction::SetModels(models) => self.set_models(models),
            ServerAction::Connect(url) => self.connect_server(url),
            ServerAction::EditUrl(url) => self.edit_url(url),
            ServerAction::Abort => self.abort(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ServerAction {
    SelectModel(String),
    SetModels(Vec<String>),
    Connect(Option<String>),
    EditUrl(String),
    Abort,
}

pub fn generate_system_message(context: &Option<String>) -> ChatMessage {
    let mut prompt = String::from(
        r#"You are a highly skilled professional translator. You will be translating text from a Japanese book. You are a native speaker of English and Japanese. Translate the given text accurately, taking into account the context and specific instructions provided. If no additional instructions or context are provided, use your expertise to consider what the most appropriate context is and provide a natural translation that aligns with that context. When translating, strive to faithfully reflect the meaning and tone of the original text, pay attention to cultural nuances and differences in language usage, and ensure that the translation is grammatically correct and easy to read. For technical terms and proper nouns, either leave them in the original language or use appropriate translations as necessary. The translation should be complete. All the provide text should be translated. The translation should only contain the translated text. This should be a full translation of the given text. Do not summarize the text. Format the translated text into paragraphs with proper syntax. Match the paragraph structure of the original text. The text passed is in a markdown format the response should be in markdown. "#,
    );

    if let Some(context) = context {
        let context = format!(
            "Here is context about the story of the book, {}.\n",
            context
        );
        prompt.push_str(&context);
    }
    prompt.push_str("Take a deep breath, calm down, and start translating.");

    ChatMessage::system(prompt)
}
