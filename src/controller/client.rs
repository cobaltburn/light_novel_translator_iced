use crate::{
    actions::{consensus_action::ConsensusAction, trans_action::TransAction},
    controller::prompts::{CONSENSUS_PROMPT, TRANSLATION_PROMPT},
    error::{Error, Result},
    model::server::Think,
};
use iced::Task;
use reqwest_middleware::{ClientBuilder as MiddlewareBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use rig::{
    agent::{MultiTurnStreamItem, StreamingError},
    client::{CompletionClient, ModelListingClient, Nothing},
    completion::{CompletionError, GetTokenUsage},
    message::Message,
    providers::ollama::{self, OllamaApiKey, OllamaExt},
    streaming::{StreamedAssistantContent, StreamingChat, StreamingPrompt},
};
use serde_json::Value;
use std::{
    ops::Not,
    sync::{Arc, Mutex},
    time::Duration,
};

const TEMPERATURE: f64 = 0.3;
const TOP_P: f64 = 0.8;
const REPEAT_PENALTY: f64 = 1.05;

pub type SharedHistory = Arc<Mutex<Vec<Message>>>;

pub trait StreamAction: 'static + Send + Clone {
    fn update(content: String, page: usize, part: usize) -> Self;
    fn cancel() -> Self;
    fn clean(page: usize, part: usize) -> Self;
}

impl StreamAction for TransAction {
    fn update(content: String, page: usize, part: usize) -> Self {
        TransAction::UpdateContent {
            content,
            page,
            part,
        }
    }
    fn cancel() -> Self {
        TransAction::CancelTranslate
    }
    fn clean(page: usize, part: usize) -> Self {
        TransAction::CleanText { page, part }
    }
}

impl StreamAction for ConsensusAction {
    fn update(content: String, page: usize, part: usize) -> Self {
        ConsensusAction::UpdateContent {
            content,
            page,
            part,
        }
    }
    fn cancel() -> Self {
        ConsensusAction::CancelConsensus
    }
    fn clean(page: usize, part: usize) -> Self {
        ConsensusAction::CleanText { page, part }
    }
}

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub enum Client {
    #[default]
    Disconnected,
    Ollama(rig::client::Client<OllamaExt, ClientWithMiddleware>),
}

impl Client {
    pub fn ollama() -> Client {
        let policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(5), Duration::from_secs(30 * 60))
            .build_with_total_retry_duration(Duration::from_secs(60));
        let http = MiddlewareBuilder::new(Default::default())
            .with(RetryTransientMiddleware::new_with_policy(policy))
            .build();
        let client = ollama::Client::builder()
            .api_key(OllamaApiKey::from(Nothing))
            .http_client(http)
            .build()
            .unwrap();
        Client::Ollama(client)
    }

    pub async fn get_models(&self) -> Result<Vec<String>> {
        match self {
            Client::Ollama(client) => {
                let models = client.list_models().await?;
                let mut models: Vec<_> = models.into_iter().map(|model| model.id).collect();
                models.sort();
                Ok(models)
            }
            Client::Disconnected => Err(Error::ServerError("server not connected")),
        }
    }

    pub fn translate(
        &self,
        prompt: String,
        model: &str,
        think: Think,
        page: usize,
        part: usize,
    ) -> Result<Task<TransAction>> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };
        let agent = client
            .agent(model)
            .preamble(TRANSLATION_PROMPT)
            .temperature(TEMPERATURE)
            .additional_params(agent_params(think))
            .build();

        let stream =
            Task::future(async move { agent.stream_prompt(prompt).await }).then(Task::stream);
        Ok(handle_stream::<TransAction, _>(stream, None, 0, page, part))
    }

    pub fn translate_history(
        &self,
        prompt: String,
        model: &str,
        history: SharedHistory,
        context_window: usize,
        think: Think,
        page: usize,
        part: usize,
    ) -> Result<Task<TransAction>> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };
        let agent = client
            .agent(model)
            .preamble(TRANSLATION_PROMPT)
            .temperature(TEMPERATURE)
            .additional_params(agent_params(think))
            .build();

        let chat_history = history.clone();
        let stream = Task::future(async move {
            let chat_history = chat_history.lock().unwrap().to_vec();
            agent.stream_chat(prompt, chat_history).await
        })
        .then(Task::stream);
        Ok(handle_stream::<TransAction, _>(
            stream,
            Some(history),
            context_window,
            page,
            part,
        ))
    }

    pub fn consensus(
        &self,
        prompt: String,
        model: String,
        think: Think,
        page: usize,
        part: usize,
    ) -> Result<Task<ConsensusAction>> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };
        let agent = client
            .agent(&model)
            .preamble(CONSENSUS_PROMPT)
            .temperature(TEMPERATURE)
            .additional_params(agent_params(think))
            .build();

        let stream =
            Task::future(async move { agent.stream_prompt(prompt).await }).then(Task::stream);
        Ok(handle_stream::<ConsensusAction, _>(
            stream, None, 0, page, part,
        ))
    }

    pub fn connected(&self) -> bool {
        matches!(self, Client::Disconnected).not()
    }
}

fn agent_params(think: Think) -> Value {
    serde_json::json!({
        "top_p": TOP_P,
        "repeat_penalty": REPEAT_PENALTY,
        "think": !matches!(think, Think::None),
    })
}

fn handle_stream<A, R>(
    stream: Task<std::result::Result<MultiTurnStreamItem<R>, StreamingError>>,
    history: Option<SharedHistory>,
    context_window: usize,
    page: usize,
    part: usize,
) -> Task<A>
where
    A: StreamAction,
    R: Clone + Unpin + GetTokenUsage + Send + 'static,
{
    stream
        .and_then(move |item| {
            let content = match item {
                MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(t))
                    if t.text.is_empty() =>
                {
                    None
                }
                MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(t)) => {
                    Some(t.text)
                }
                MultiTurnStreamItem::FinalResponse(response) => {
                    if let Some(history) = &history
                        && let Some(hist) = response.history()
                    {
                        let mut shared = history.lock().unwrap();
                        shared.extend_from_slice(hist);
                        shift_history(&mut shared, context_window);
                    }
                    None
                }
                _ => None,
            };
            Task::done(Ok(content))
        })
        .then(move |content| match content {
            Ok(None) => Task::none(),
            Ok(Some(content)) => Task::done(A::update(content, page, part)),
            Err(StreamingError::Completion(CompletionError::JsonError(_))) => Task::none(),
            Err(error) => Task::done(A::cancel()).chain(Error::from(error).display_error()),
        })
        .chain(Task::done(A::clean(page, part)))
}

fn shift_history(history: &mut Vec<Message>, context_window: usize) {
    let pair_count = history.len() / 2;
    if pair_count <= context_window {
        return;
    }
    let skip = (pair_count - context_window) * 2;
    history.drain(..skip);
}
