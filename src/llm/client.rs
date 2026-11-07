use crate::{
    error::{Error, Result},
    llm::{
        Settings, SharedHistory,
        history::CONVERSATION_ID,
        prompts::{CONSENSUS_PROMPT, GLOSSARY_PROMPT, TRANSLATION_PROMPT, translation_glossary},
    },
};
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use rig::{
    Agent, AgentBuilder,
    providers::ollama::{Ollama, OllamaConfig},
    rig_reqwest::ReqwestMiddlewareClient,
};
use std::time::Duration;

const RETRY_DURATION: Duration = Duration::from_secs(240);
const MIN_RETRY_INTERVAL: Duration = Duration::from_secs(2);
const MAX_RETRY_INTERVAL: Duration = Duration::from_secs(30);

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub enum Client {
    #[default]
    Disconnected,
    Ollama(Ollama),
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

        Client::Ollama(OllamaConfig::new().connect(http))
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

    pub fn translation_agent(
        &self,
        model: &str,
        settings: &Settings,
        history: Option<SharedHistory>,
        glossary: Option<&str>,
    ) -> Result<Agent> {
        let builder = self
            .agent_builder(model)?
            .preamble(TRANSLATION_PROMPT)
            .temperature(settings.temperature)
            .additional_params(settings.agent_params());
        let builder = match history {
            Some(history) => builder.memory(history.store).conversation(CONVERSATION_ID),
            None => builder,
        };
        let builder = match glossary {
            Some(glossary) => builder.append_preamble(&translation_glossary(glossary)),
            None => builder,
        };
        Ok(builder.build())
    }

    pub fn consensus_agent(&self, model: &str, settings: &Settings) -> Result<Agent> {
        Ok(self
            .agent_builder(model)?
            .preamble(CONSENSUS_PROMPT)
            .temperature(settings.temperature)
            .additional_params(settings.agent_params())
            .build())
    }

    pub fn glossary_agent(&self, model: &str, settings: &Settings) -> Result<Agent> {
        Ok(self
            .agent_builder(model)?
            .preamble(GLOSSARY_PROMPT)
            .temperature(settings.temperature)
            .additional_params(settings.agent_params())
            .build())
    }

    fn agent_builder(&self, model: &str) -> Result<AgentBuilder> {
        let Client::Ollama(ollama) = self else {
            return Err(Error::ServerError("server not connected"));
        };
        Ok(AgentBuilder::new(ollama.native_completion(model)))
    }

    pub fn connected(&self) -> bool {
        !matches!(self, Client::Disconnected)
    }
}
