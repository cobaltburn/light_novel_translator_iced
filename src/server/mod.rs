//! The connection to the model server shared by the screens: its settings, the
//! running tasks, and the controls for both.

mod jobs;
mod method;
mod state;
mod update;
mod view;

pub use method::Method;
pub use state::Server;
pub use update::Action;
pub use view::{
    context_window_input, execution_selector, num_ctx_input, ollama_input, server_setting_input,
    think_selector,
};
