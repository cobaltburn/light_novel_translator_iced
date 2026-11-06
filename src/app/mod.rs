//! The application root: its state, message routing and top-level layout.

mod message;
mod side_bar;
mod state;
mod tabs;
mod view;

pub use message::Message;
pub use state::Translator;
pub use view::View;

use crate::error::Result;
use iced::Theme;
use iced_aw::ICED_AW_FONT_BYTES;

pub fn run() -> Result<()> {
    iced::application(Translator::default, Translator::update, Translator::view)
        .title("light novel translator")
        .theme(Theme::TokyoNightStorm)
        .font(ICED_AW_FONT_BYTES)
        .run()?;
    Ok(())
}
