mod message;
mod side_bar;
mod state;
mod tabs;
mod view;

pub use message::*;
pub use state::*;
pub use view::*;

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
