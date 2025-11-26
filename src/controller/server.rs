use crate::message::Message;
use crate::state::{server_state::ServerAction, translation_model::TransAction};
use anyhow::anyhow;
use iced::Task;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, ChatMessageResponseStream, request::ChatMessageRequest},
};
use rig::{
    agent::{MultiTurnStreamItem, StreamingPromptRequest},
    client::CompletionClient,
    message::Message as ClaudeMessage,
    providers::anthropic::{
        self, CLAUDE_3_5_SONNET, CLAUDE_3_7_SONNET, CLAUDE_3_HAIKU, CLAUDE_3_OPUS, CLAUDE_3_SONNET,
        CLAUDE_4_OPUS, CLAUDE_4_SONNET, completion::CompletionModel,
    },
    streaming::{StreamedAssistantContent, StreamingChat},
};
use std::time::Duration;
use std::{
    ops::Not,
    sync::{Arc, Mutex},
};
use tokio::time;

pub const MODELS: &[&str] = &[
    "claude-sonnet-4-5-20250929",
    "claude-haiku-4-5-20251001",
    CLAUDE_4_SONNET,
    CLAUDE_4_OPUS,
    CLAUDE_3_7_SONNET,
    CLAUDE_3_5_SONNET,
    CLAUDE_3_SONNET,
    CLAUDE_3_OPUS,
    CLAUDE_3_HAIKU,
];

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub enum Server {
    #[default]
    Disconnected,
    Ollama {
        client: Ollama,
        history: Arc<Mutex<Vec<ChatMessage>>>,
    },
    Cluade {
        client: anthropic::Client,
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

    pub fn claude(client: anthropic::Client) -> Server {
        Server::Cluade {
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
            Server::Cluade { history, .. } => history.lock().unwrap().clear(),
        }
    }

    pub async fn get_models(self) -> anyhow::Result<Vec<String>> {
        let model = match self {
            Server::Ollama { client, .. } => {
                let models = client.list_local_models().await?;
                models.into_iter().map(|model| model.name).collect()
            }
            Server::Cluade { .. } => MODELS.into_iter().map(|&model| model.to_string()).collect(),
            Server::Disconnected => return Err(anyhow!("server not connected")),
        };
        Ok(model)
    }

    pub fn translate(self, model: String, sections: Vec<String>, page: usize) -> Task<Message> {
        match self {
            Server::Disconnected => Task::done(Message::Error("server disconnectd".to_string())),
            Server::Ollama { .. } => self.ollama_translation(model, sections, page),
            Server::Cluade { .. } => self.claude_translation(model, sections, page),
        }
    }

    pub fn claude_translation(
        self,
        model: String,
        sections: Vec<String>,
        page: usize,
    ) -> Task<Message> {
        let Server::Cluade { history, .. } = &self else {
            panic!()
        };

        let history = history.clone();
        let request = self.claude_stream(model, sections);

        Task::future(async {
            time::sleep(Duration::from_secs(10)).await;
            request.await
        })
        .then(|stream| Task::stream(stream))
        .then(move |result| match result {
            Ok(item) => match item {
                MultiTurnStreamItem::StreamItem(content) => match content {
                    StreamedAssistantContent::Text(text) => {
                        Task::done(TransAction::UpdateContent(text.text().to_string(), page).into())
                    }
                    _ => Task::none(),
                },
                MultiTurnStreamItem::FinalResponse(response) => {
                    let msg = ClaudeMessage::assistant(response.response());
                    history.lock().unwrap().push(msg);
                    log::debug!("{:#?}", response);
                    Task::none()
                }
                _ => todo!(),
            },
            Err(error) => Task::done(ServerAction::Abort.into())
                .chain(Task::done(Message::Error(format!("{:#?}", error)))),
        })
    }

    pub fn claude_stream(
        self,
        model: String,
        sections: Vec<String>,
    ) -> StreamingPromptRequest<CompletionModel, ()> {
        let Server::Cluade { client, history } = self else {
            panic!()
        };

        let agent = client.agent(&model).preamble(SYSTEM_PROMPT).build();

        let mut history = history.lock().unwrap();
        let prompt = ClaudeMessage::user(sections.join(" "));

        let request = agent.stream_chat(prompt.clone(), history.clone());
        history.push(prompt);

        request
    }

    pub fn ollama_translation(
        self,
        model: String,
        sections: Vec<String>,
        page: usize,
    ) -> Task<Message> {
        Task::future(self.ollama_stream(model.clone(), sections.to_owned()))
            .and_then(move |stream| Task::stream(stream))
            .then(move |response| match response {
                Ok(msg) => Task::done(TransAction::UpdateContent(msg.message.content, page).into()),
                Err(_) => Task::done(ServerAction::Abort.into()).chain(Task::done(Message::Error(
                    String::from("Failed to read stream"),
                ))),
            })
    }

    pub async fn ollama_stream(
        self,
        model: String,
        section: Vec<String>,
    ) -> anyhow::Result<ChatMessageResponseStream> {
        let Server::Ollama { client, history } = self else {
            return Err(anyhow!("server not connected"));
        };

        let messages = section.into_iter().map(|c| ChatMessage::user(c)).collect();
        let request = ChatMessageRequest::new(model.clone(), messages);
        let stream: ChatMessageResponseStream = client
            .send_chat_messages_with_history_stream(history, request)
            .await?;

        Ok(stream)
    }

    pub fn connected(&self) -> bool {
        matches!(self, Server::Disconnected).not()
    }
}

#[derive(Debug, Clone)]
pub enum Connection {
    Ollama(String),
    Claude(String),
}

pub const SYSTEM_PROMPT: &str = r#"You are a highly skilled professional translator. You will be translating text from a Japanese book. You are a native speaker of English and Japanese. Translate the given text accurately, taking into account the context and specific instructions provided. If no additional instructions or context are provided, use your expertise to consider what the most appropriate context is and provide a natural translation that aligns with that context. When translating, strive to faithfully reflect the meaning and tone of the original text, pay attention to cultural nuances and differences in language usage, and ensure that the translation is grammatically correct and easy to read. For technical terms and proper nouns, either leave them in the original language or use appropriate translations as necessary. The translation should be complete. All the provide text should be translated. The translation should only contain the translated text. This should be a full translation of the given text. Do not summarize the text. Format the translated text into paragraphs with proper syntax. Match the paragraph structure of the original text. The text passed is in a markdown format the response should be in markdown. Take a deep breath, calm down, and start translating."#;
