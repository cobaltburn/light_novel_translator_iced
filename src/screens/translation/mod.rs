//! Translates an epub. The app can have several of these open, one per tab.

mod state;
mod update;
mod view;

pub use state::Translation;
pub use update::Action;
pub use view::translation_view;
