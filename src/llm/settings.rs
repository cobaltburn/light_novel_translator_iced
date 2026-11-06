use serde::{Serialize, Serializer};
use serde_json::Value;

pub const TEMPERATURE: f64 = 0.8;
pub const TOP_P: f64 = 0.8;
pub const REPEAT_PENALTY: f64 = 1.0;
pub const DEFAULT_CONTEXT_WINDOW: usize = 10;
pub const MAX_NUM_CTX: u32 = 1048576;
pub const DEFAULT_NUM_CTX: u32 = MAX_NUM_CTX / 2;

#[derive(Debug, Clone)]
pub struct Settings {
    pub think: Think,
    pub context_window: usize,
    pub num_ctx: u32,
    pub temperature: f64,
    pub top_p: f64,
    pub repeat_penalty: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            think: Default::default(),
            context_window: DEFAULT_CONTEXT_WINDOW,
            num_ctx: DEFAULT_NUM_CTX,
            temperature: TEMPERATURE,
            top_p: TOP_P,
            repeat_penalty: REPEAT_PENALTY,
        }
    }
}

impl Settings {
    pub fn agent_params(&self) -> Value {
        serde_json::json!({
            "top_p": &self.top_p,
            "repeat_penalty": &self.repeat_penalty,
            "think": &self.think,
            "num_ctx": &self.num_ctx,
        })
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Think {
    #[default]
    High,
    Medium,
    Low,
    None,
}

impl Serialize for Think {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Think::High => serializer.serialize_str("high"),
            Think::Medium => serializer.serialize_str("medium"),
            Think::Low => serializer.serialize_str("low"),
            Think::None => serializer.serialize_bool(false),
        }
    }
}
