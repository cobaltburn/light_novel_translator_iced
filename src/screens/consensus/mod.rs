//! Merges several finished translations of a book into one.

mod state;
mod update;
mod view;

pub use state::{Candidate, Consensus};
pub use update::Action;
pub use view::consensus_view;
