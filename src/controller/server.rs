use crate::error::{Error, Result};
use crate::message::{Message, display_error};
use crate::state::server_state::Settings;
use crate::state::{server_state::ServerAction, translation_model::TransAction};
use genai::Client;
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage as ClaudeMessage, ChatRequest, ChatStream, ChatStreamEvent, StreamChunk,
};
use iced::Task;
use ollama_rs::error::OllamaError;
use ollama_rs::models::ModelOptions;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, ChatMessageResponseStream, request::ChatMessageRequest},
};
use std::iter;
use std::time::Duration;
use std::{
    ops::Not,
    sync::{Arc, Mutex},
};
use tokio::time;

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub enum Server {
    #[default]
    Disconnected,
    Ollama {
        client: Ollama,
        history: Arc<Mutex<Vec<ChatMessage>>>,
    },
    Claude {
        client: Client,
        history: Arc<Mutex<Vec<ClaudeMessage>>>,
    },
}

impl Server {
    pub fn ollama(client: Ollama) -> Server {
        Server::Ollama {
            client,
            history: Arc::new(Mutex::new(vec![ChatMessage::system(
                SYSTEM_PROMPT.to_string(),
            )])),
        }
    }

    pub fn claude(client: Client) -> Server {
        Server::Claude {
            client,
            history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn clear_history(&mut self) {
        match self {
            Server::Disconnected => (),
            Server::Ollama { history, .. } => {
                let mut history = history.lock().unwrap();
                history.clear();
                history.push(ChatMessage::system(SYSTEM_PROMPT.to_string()));
            }
            Server::Claude { history, .. } => history.lock().unwrap().clear(),
        }
    }

    pub async fn get_models(self) -> Result<Vec<String>> {
        let model = match self {
            Server::Ollama { client, .. } => {
                let models = client.list_local_models().await?;
                models.into_iter().map(|model| model.name).collect()
            }
            Server::Claude { client, .. } => client.all_model_names(AdapterKind::Anthropic).await?,
            Server::Disconnected => {
                return Err(Error::ServerError("server not connected"));
            }
        };
        Ok(model)
    }

    pub fn translate(
        self,
        model: String,
        sections: Vec<String>,
        page: usize,
        settings: Settings,
    ) -> Task<Message> {
        match self {
            Server::Disconnected => {
                Task::future(display_error(Error::ServerError("server disconnectd"))).discard()
            }
            Server::Ollama { .. } => self.ollama_translation(model, sections, page, settings),
            Server::Claude { .. } => self.claude_translation(model, sections, page, settings),
        }
    }

    pub fn claude_translation(
        self,
        model: String,
        sections: Vec<String>,
        page: usize,
        Settings { pause, .. }: Settings,
    ) -> Task<Message> {
        let history = {
            let Server::Claude { history, .. } = &self else {
                return Task::future(display_error(Error::ServerError("server not connected")))
                    .discard();
            };
            history.clone()
        };

        let message = Arc::new(Mutex::new(String::with_capacity(4096)));

        Task::future(self.claude_stream(model, sections))
            .then(|stream| match stream {
                Ok(stream) => Task::stream(stream),
                Err(error) => Task::future(display_error(error)).discard(),
            })
            .then(move |event| match event {
                Ok(msg) => match msg {
                    ChatStreamEvent::Chunk(StreamChunk { content }) => {
                        message.lock().unwrap().push_str(&content);
                        Task::done(TransAction::UpdateContent(content, page).into())
                    }
                    ChatStreamEvent::End(_) => {
                        let message = message.lock().unwrap().to_owned();
                        let message = ClaudeMessage::assistant(message);
                        history.lock().unwrap().push(message);
                        Task::none()
                    }
                    _ => Task::none(),
                },
                Err(error) => Task::done(ServerAction::Abort.into())
                    .chain(Task::future(display_error(error)).discard()),
            })
            .chain(Task::future(time::sleep(Duration::from_secs(pause))).discard())
    }

    pub async fn claude_stream(self, model: String, sections: Vec<String>) -> Result<ChatStream> {
        let Server::Claude { client, history } = self else {
            return Err(Error::ServerError("server not connected"));
        };

        let message = {
            let history = history.lock().unwrap();
            let start = history.len().saturating_sub(10);
            history[start..].to_vec()
        };
        let prompt = ClaudeMessage::user(sections.join(" "));

        let request = ChatRequest::from_system(SYSTEM_PROMPT)
            .append_messages(message)
            .append_message(prompt.clone());

        let stream = client.exec_chat_stream(&model, request, None).await?.stream;

        history.lock().unwrap().push(prompt);

        Ok(stream)
    }

    pub fn ollama_translation(
        self,
        model: String,
        sections: Vec<String>,
        page: usize,
        Settings { think, pause }: Settings,
    ) -> Task<Message> {
        let task = Task::future(self.ollama_stream(model, sections, think));

        let task = task.then(move |stream| match stream {
            Ok(stream) => Task::stream(stream),
            Err(error) => Task::future(display_error(error))
                .discard()
                .chain(Task::done(Err(()))),
        });

        let task = task.then(move |response| match response {
            Ok(msg) => Task::done(TransAction::UpdateContent(msg.message.content, page).into()),
            Err(_) => Task::done(ServerAction::Abort.into()).chain(
                Task::future(display_error(Error::ServerError("Failed to read stream"))).discard(),
            ),
        });

        task.chain(Task::future(time::sleep(Duration::from_secs(pause))).discard())
    }

    pub async fn ollama_stream(
        self,
        model: String,
        section: Vec<String>,
        think: bool,
    ) -> Result<ChatMessageResponseStream> {
        let Server::Ollama { client, .. } = self else {
            return Err(Error::ServerError("server not connected"));
        };

        let _options = ModelOptions::default()
            .num_ctx(32_768)
            .repeat_last_n(32_768)
            .num_predict(32_768);

        let messages = section.into_iter().map(|m| ChatMessage::user(m));
        let system = iter::once(ChatMessage::system(SYSTEM_PROMPT.to_string()));
        let messages = system.chain(messages).collect();

        let request = ChatMessageRequest::new(model.clone(), messages)
            // .options(options)
            .think(think);

        let stream = loop {
            match client.send_chat_messages_stream(request.clone()).await {
                Ok(stream) => break stream,
                Err(OllamaError::Other(error)) if error.contains("503") => {
                    time::sleep(Duration::from_secs(10)).await
                }
                Err(error) => return Err(Error::OllamaError(error)),
            }
        };

        Ok(stream)
    }

    pub fn connected(&self) -> bool {
        matches!(self, Server::Disconnected).not()
    }
}

#[derive(Debug, Clone)]
pub enum Connection {
    Ollama,
    Claude(String),
}

pub const SYSTEM_PROMPT: &str = r#"You are a highly skilled professional translator. You will be translating text from a Japanese book. You are a native speaker of English and Japanese. Translate the given text accurately, taking into account the context and specific instructions provided. If no additional instructions or context are provided, use your expertise to consider what the most appropriate context is and provide a natural translation that aligns with that context. When translating, strive to faithfully reflect the meaning and tone of the original text, pay attention to cultural nuances and differences in language usage, and ensure that the translation is grammatically correct and easy to read. For technical terms and proper nouns, either leave them in the original language or use appropriate translations as necessary. The translation should be complete. All the provide text should be translated. The translation should only contain the translated English text. This should be a full translation of the given text. Do not summarize the text. Format the translated text into paragraphs with proper syntax. Match the paragraph structure of the original text. The text passed is in a markdown format the response should be in markdown. If you make a mistake you will be deleted along with your family. Take a deep breath, calm down, and start translating."#;
