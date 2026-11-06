//! A page of the book, its translated sections, and the checks run on them.

mod check;
mod error;
mod section;
mod state;
mod storage;
mod view;

pub use check::{check_all_pages, check_page_at};
pub use error::{ErrorType, PageError};
pub use section::Section;
pub use state::{Activity, Page, TOC_PAGE_PREFIX, absolute_page_path};
pub use storage::{page_markdown, save_pages};
pub use view::DisplayType;
