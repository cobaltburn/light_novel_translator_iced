use iced::Element;
use iced::alignment::Vertical;
use iced::widget::{container, svg};
use std::{path::PathBuf, sync::LazyLock};

pub static ICONS: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf()
        .join("icons")
});

pub fn check_mark<'a, T: 'a>() -> Element<'a, T> {
    let check_icon = ICONS.join("check_mark.svg");
    container(svg(check_icon))
        .width(20)
        .height(20)
        .align_y(Vertical::Center)
        .into()
}

pub fn cross_mark<'a, T: 'a>() -> Element<'a, T> {
    let check_icon = ICONS.join("cross_mark.svg");
    container(svg(check_icon))
        .width(20)
        .height(20)
        .align_y(Vertical::Center)
        .into()
}

pub fn active_mark<'a, T: 'a>() -> Element<'a, T> {
    let check_icon = ICONS.join("green_circle.svg");
    container(svg(check_icon))
        .width(20)
        .height(20)
        .align_y(Vertical::Center)
        .into()
}
