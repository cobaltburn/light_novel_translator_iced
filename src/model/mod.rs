pub mod doc;
pub mod extraction;
pub mod format;
pub mod server;
pub mod translation;
pub mod translator;

#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub enum Activity {
    #[default]
    Incomplete,
    Complete,
    Active,
    Error,
}
