mod consensus;
mod doc;
mod format;
mod page;
mod server;
mod translation;
mod translator;

pub use consensus::*;
pub use doc::*;
pub use format::*;
pub use page::*;
pub use server::*;
pub use translation::*;
pub use translator::*;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub enum Activity {
    #[default]
    Incomplete,
    Complete,
    Active(usize),
    Error(usize),
}
