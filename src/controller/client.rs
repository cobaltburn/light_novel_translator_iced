use crate::{
    actions::{consensus, translation},
    controller::{CONSENSUS_PROMPT, TRANSLATION_PROMPT},
    error::{Error, Result, ResultTaskExt},
    model::{REPEAT_PENALTY, Settings, TEMPERATURE, TOP_P, Think},
};
use iced::Task;
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use rig::{
    AgentBuilder,
    agent::MultiTurnStreamItem,
    id::ConversationId,
    memory::{
        CompactingMemory, ConversationMemory, InMemoryConversationMemory, SlidingWindowMemory,
        TemplateCompactor,
    },
    message::Message,
    providers::ollama::{self},
    rig_reqwest::ReqwestMiddlewareClient,
    run::PromptError,
    streaming::{Item, StreamEvent},
};
use serde_json::Value;
use std::{sync::Arc, time::Duration};

const RETRY_DURATION: Duration = Duration::from_secs(240);
const MIN_RETRY_INTERVAL: Duration = Duration::from_secs(2);
const MAX_RETRY_INTERVAL: Duration = Duration::from_secs(30);
const CONVERSATION_ID: &str = "translation";
const SUMMARY_MAX_BYTES: usize = 8 * 1024;

pub type SharedHistory =
    Arc<CompactingMemory<InMemoryConversationMemory, SlidingWindowMemory, TemplateCompactor>>;

/// `context_window` counts user/assistant pairs, so the window holds twice as many messages.
pub fn shared_history(context_window: usize) -> SharedHistory {
    Arc::new(CompactingMemory::new(
        InMemoryConversationMemory::new(),
        SlidingWindowMemory::last_messages(context_window * 2),
        TemplateCompactor::new().with_max_bytes(SUMMARY_MAX_BYTES),
    ))
}

pub fn seed_history<A>(history: SharedHistory, messages: Vec<Message>) -> Task<A>
where
    A: StreamAction,
{
    Task::future(async move {
        let id = ConversationId::from(CONVERSATION_ID);
        history.append(&id, messages).await
    })
    .then(|result| result.map_err(Error::from).ok_or_display())
}

pub trait StreamAction: 'static + Send + Clone {
    fn update(content: String, page: usize, part: usize) -> Self;
    fn cancel() -> Self;
    fn clean(page: usize, part: usize) -> Self;
}

impl StreamAction for translation::Action {
    fn update(content: String, page: usize, part: usize) -> Self {
        translation::Action::UpdateContent {
            content,
            page,
            part,
        }
    }
    fn cancel() -> Self {
        translation::Action::CancelTranslate
    }
    fn clean(page: usize, part: usize) -> Self {
        translation::Action::CleanText { page, part }
    }
}

impl StreamAction for consensus::Action {
    fn update(content: String, page: usize, part: usize) -> Self {
        consensus::Action::UpdateContent {
            content,
            page,
            part,
        }
    }

    fn cancel() -> Self {
        consensus::Action::CancelConsensus
    }

    fn clean(page: usize, part: usize) -> Self {
        consensus::Action::CleanText { page, part }
    }
}

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub enum Client {
    #[default]
    Disconnected,
    Ollama(ollama::Ollama),
}

impl Client {
    pub fn ollama() -> Client {
        let policy = ExponentialBackoff::builder()
            .retry_bounds(MIN_RETRY_INTERVAL, MAX_RETRY_INTERVAL)
            .build_with_total_retry_duration(RETRY_DURATION);
        let http: ReqwestMiddlewareClient = ClientBuilder::new(Default::default())
            .with(RetryTransientMiddleware::new_with_policy(policy))
            .build()
            .into();

        let ollama = ollama::Ollama::new().with_http(http);
        Client::Ollama(ollama)
    }

    pub async fn get_models(&self) -> Result<Vec<String>> {
        match self {
            Client::Ollama(ollama) => {
                let models = ollama.list_models().await?;
                let mut models: Vec<_> = models.into_iter().map(|model| model.id).collect();
                models.sort();
                Ok(models)
            }
            Client::Disconnected => Err(Error::ServerError("server not connected")),
        }
    }

    pub fn translate(
        &self,
        japanese: &str,
        model: &str,
        settings: Settings,
        page: usize,
        part: usize,
    ) -> Task<translation::Action> {
        let Client::Ollama(ollama) = self else {
            return Task::none();
        };
        let agent = AgentBuilder::new(ollama.completion(model))
            .preamble(TRANSLATION_PROMPT)
            .temperature(settings.temperature)
            .additional_params(settings.agent_params())
            .build();

        let japanese = japanese.to_string();
        let stream =
            Task::future(async move { agent.prompt(japanese).stream() }).then(Task::stream);
        handle_stream::<translation::Action>(stream, page, part)
    }

    pub fn translate_history(
        &self,
        japanese: &str,
        model: &str,
        history: SharedHistory,
        settings: Settings,
        page: usize,
        part: usize,
    ) -> Task<translation::Action> {
        let Client::Ollama(ollama) = self else {
            return Task::none();
        };

        let agent = AgentBuilder::new(ollama.completion(model))
            .preamble(TRANSLATION_PROMPT)
            .temperature(settings.temperature)
            .additional_params(settings.agent_params())
            .memory(history)
            .conversation(CONVERSATION_ID)
            .build();

        let japanese = japanese.to_string();
        let stream =
            Task::future(async move { agent.prompt(japanese).stream() }).then(Task::stream);
        handle_stream::<translation::Action>(stream, page, part)
    }

    pub fn consensus(
        &self,
        prompt: String,
        model: String,
        think: Think,
        page: usize,
        part: usize,
    ) -> Result<Task<consensus::Action>> {
        let Client::Ollama(ollama) = self else {
            return Err(Error::ServerError("server not connected"));
        };
        let agent = AgentBuilder::new(ollama.completion(model))
            .preamble(CONSENSUS_PROMPT)
            .temperature(TEMPERATURE)
            .additional_params(agent_params(think))
            .build();

        let stream = Task::future(async move { agent.prompt(prompt).stream() }).then(Task::stream);
        Ok(handle_stream::<consensus::Action>(stream, page, part))
    }

    pub fn connected(&self) -> bool {
        !matches!(self, Client::Disconnected)
    }
}

fn agent_params(think: Think) -> Value {
    serde_json::json!({
        "top_p": TOP_P,
        "repeat_penalty": REPEAT_PENALTY,
        "think": think,
    })
}

fn handle_stream<A>(
    stream: Task<std::result::Result<MultiTurnStreamItem, PromptError>>,
    page: usize,
    part: usize,
) -> Task<A>
where
    A: StreamAction,
{
    stream
        .map_err(Error::from)
        .and_then(move |item| {
            let content = match item {
                MultiTurnStreamItem::StreamAssistantItem(Item::Event(StreamEvent::Text {
                    text,
                    ..
                })) if !text.is_empty() => Some(text),
                _ => None,
            };
            Task::done(Ok(content))
        })
        .then(move |content| match content {
            Ok(ok) => ok.map_or_default(|content| Task::done(A::update(content, page, part))),
            Err(error) => Task::done(A::cancel()).chain(error.display_error()),
        })
        .chain(Task::done(A::clean(page, part)))
}
