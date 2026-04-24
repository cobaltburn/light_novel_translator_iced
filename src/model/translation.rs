use crate::{
    actions::trans_action::TransAction,
    model::{Activity, page::Page, server::Server},
    widget::{active_mark, check_mark, context_menu_button, cross_mark, text_button},
};
use async_tempfile::TempFile;
use iced::{
    Color, Element, Length, Padding,
    widget::{Column, column, container, row, scrollable, text},
};
use iced_aw::{ContextMenu, TabLabel};
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Translation {
    pub server: Server,
    pub file_path: PathBuf,
    pub current_page: usize,
    pub pages: Vec<Page>,
    pub recovery_file: Option<TempFile>,
}

impl Translation {
    pub fn tab_label(&self) -> TabLabel {
        match self.server.handles.is_empty() {
            true => TabLabel::Text(self.file_name()),
            false => TabLabel::IconText('\u{25CF}', self.file_name()),
        }
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    pub fn current_content(&self) -> Option<Vec<&String>> {
        let text = self
            .pages
            .get(self.current_page)?
            .sections
            .iter()
            .map(|e| &e.content)
            .collect();

        Some(text)
    }

    pub fn current_jap_errors(&self) -> Option<&[usize]> {
        Some(&self.pages.get(self.current_page)?.jap_error)
    }

    pub fn current_size_errors(&self) -> Option<&[usize]> {
        Some(&self.pages.get(self.current_page)?.size_error)
    }

    pub fn path_buttons(&self) -> Column<'_, TransAction> {
        self.pages
            .iter()
            .enumerate()
            .map(|(i, page)| {
                let name = page.path.file_stem().unwrap().to_string_lossy();
                let button_text =
                    text!("{}. {}", i + 1, &name)
                        .width(Length::Fill)
                        .style(move |theme| {
                            if self.current_page == i {
                                text::primary(theme)
                            } else {
                                text::default(theme)
                            }
                        });

                let mark = match page.activity {
                    Activity::Incomplete => None,
                    Activity::Complete => Some(check_mark()),
                    Activity::Error(i) => Some(row![text(i), cross_mark()].spacing(5).into()),
                    Activity::Active => Some(active_mark()),
                };

                let button_content = row![button_text]
                    .push(mark)
                    .padding(Padding::default().right(10));

                let count = page.sections.len();
                let active = self.server.connected() && self.server.handles.is_empty();
                let underlay = text_button(button_content).on_press(TransAction::SetPage(i));

                ContextMenu::new(underlay, move || {
                    path_button_overlay(count, name.to_string(), i, active)
                })
                .into()
            })
            .collect()
    }
}

impl From<Server> for Translation {
    fn from(server: Server) -> Self {
        Self {
            server,
            ..Default::default()
        }
    }
}

impl From<TempFile> for Translation {
    fn from(file: TempFile) -> Self {
        Self {
            recovery_file: Some(file),
            ..Default::default()
        }
    }
}

fn path_button_overlay<'a>(
    count: usize,
    name: String,
    page: usize,
    active: bool,
) -> Element<'a, TransAction> {
    let part_buttons = (0..count).map(|part| {
        context_menu_button(text!("translate part {}", part + 1).color(Color::WHITE))
            .on_press_maybe(active.then_some(TransAction::TranslatePart { page, part }))
            .into()
    });

    let overlay = column![
        context_menu_button(text("save").color(Color::WHITE))
            .on_press(TransAction::SavePage { name, page }),
        context_menu_button(text("translate").color(Color::WHITE))
            .on_press_maybe(active.then_some(TransAction::Translate(page))),
        context_menu_button(text("translate page").color(Color::WHITE))
            .on_press_maybe(active.then_some(TransAction::TranslatePage(page)))
    ]
    .extend(part_buttons)
    .padding(5)
    .spacing(5);

    container(scrollable(overlay).width(Length::Fill))
        .style(container::rounded_box)
        .max_height(400)
        .width(300)
        .into()
}
