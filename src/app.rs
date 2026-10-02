use crate::{
    error::Result,
    message::Message,
    model::Translator,
    view::{View, consensus_view, doc_view, format_view, translation_view},
    widget::side_bar_container,
};
use iced::{
    Element, Length, Theme,
    alignment::Horizontal,
    widget::{column, container, row},
};
use iced_aw::ICED_AW_FONT_BYTES;
use std::{path::PathBuf, sync::LazyLock};

pub static ICONS: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf()
        .join("icons")
});

pub static RECOVERY_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| std::env::temp_dir().join("light_novel_translator"));

pub fn app() -> Result<()> {
    iced::application(Translator::default, Translator::update, Translator::view)
        .title("light novel translator")
        .theme(Theme::TokyoNightStorm)
        .font(ICED_AW_FONT_BYTES)
        .run()?;
    Ok(())
}

impl Translator {
    pub fn view(&self) -> Element<'_, Message> {
        container(row![
            side_bar_container(self),
            column![self.view_select()]
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        ])
        .into()
    }

    pub fn view_select(&self) -> Element<'_, Message> {
        match self.view {
            View::Doc => doc_view(&self.doc).map(Into::into),
            View::Translation => translation_view(&self.translations, self.active_tab),
            View::Format => format_view(&self.format).map(Into::into),
            View::Consensus => consensus_view(&self.consensus).map(Into::into),
        }
    }
}
