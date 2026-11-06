//! Widgets shared by the screens.

mod buttons;
mod icons;
mod page_sidebar;
mod scroll;
mod spinner;

pub use buttons::{context_menu_button, menu_button, text_button};
pub use icons::{ICONS, active_mark, check_mark, cross_mark};
pub use page_sidebar::{SidebarAction, page_side_bar};
pub use scroll::{rich_text_scrollable, scroll_to_section, text_scrollable};
pub use spinner::Spinner;
