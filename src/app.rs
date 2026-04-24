use crate::{
    error::Result,
    message::Message,
    model::translator::Translator,
    view::{
        View, consensus_view::consensus_view, doc_view::doc_view, extraction_view::extraction_view,
        format_view::format_view, translation_view::translation_view,
    },
    widget::side_bar::side_bar_container,
};
use iced::{
    Element, Length, Task, Theme,
    alignment::Horizontal,
    widget::{column, container, row},
};
use iced_aw::ICED_AW_FONT_BYTES;
use std::{cell::LazyCell, fs, path::PathBuf};

pub const ICONS: LazyCell<PathBuf> = LazyCell::new(|| {
    std::env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf()
        .join("icons")
});

pub const RECOVERY_DIR: LazyCell<PathBuf> =
    LazyCell::new(|| std::env::temp_dir().join("light_novel_translator"));

pub fn app() -> Result<()> {
    iced::application(Translator::new, Translator::update, Translator::view)
        .title("light novel translator")
        .theme(Theme::TokyoNightStorm)
        .font(ICED_AW_FONT_BYTES)
        .run()?;
    Ok(())
}

impl Translator {
    fn new() -> (Self, Task<Message>) {
        fs::create_dir_all(&*RECOVERY_DIR).expect("Unable to create temp directory");
        let read_dir = fs::read_dir(&*RECOVERY_DIR).unwrap();
        let files: Vec<_> = read_dir
            .flatten()
            .filter_map(|e| e.file_type().ok()?.is_file().then_some(e.path()))
            .collect();

        (
            Translator {
                ..Default::default()
            },
            Task::none(),
        )
    }

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
            View::Extraction => extraction_view(&self.extraction).map(Into::into),
            View::Consensus => consensus_view(&self.consensus).map(Into::into),
        }
    }
}
