//! Reads the book to generate a glossary for the translators.

mod state;
mod update;
mod view;

pub use state::Glossary;
pub use update::Action;
pub use view::glossary_view;
