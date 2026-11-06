//! Talking to the model: the client, its settings, prompts, history and streaming.

mod client;
mod history;
mod prompts;
mod settings;
mod stream;

pub use client::Client;
pub use history::{SharedHistory, compact_history, seed_history, shared_history};
pub use prompts::{GLOSSARY_PROMPT, consensus_prompt, glossary_prompt};
pub use settings::{MAX_NUM_CTX, Settings, Think};
pub use stream::{Reply, StreamAction, fail_run, reply_stream, stream_task};
