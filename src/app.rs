use crate::{
    error::Result,
    message::Message,
    model::translator::Translator,
    view::{
        View, doc_view::doc_view, extraction_view::extraction_view, format_view::format_view,
        translation_view::traslation_view,
    },
    widget::side_bar::side_bar_container,
};
use iced::{
    Element, Length, Theme,
    alignment::Horizontal,
    widget::{column, container, row},
};
use iced_aw::ICED_AW_FONT_BYTES;
use std::{cell::LazyCell, path::PathBuf};

pub const ICONS: LazyCell<PathBuf> = LazyCell::new(|| {
    let mut path = std::env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf();

    path.push("icons");
    path
});

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
            column![Self::view_select(&self)]
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        ])
        .into()
    }

    pub fn view_select(&self) -> Element<'_, Message> {
        match self.view {
            View::Doc => doc_view(&self.doc).map(Into::into),
            View::Translation => traslation_view(&self.translations, self.active_tab),
            View::Format => format_view(&self.format).map(Into::into),
            View::Extraction => extraction_view(&self.extraction).map(Into::into),
        }
    }
}
