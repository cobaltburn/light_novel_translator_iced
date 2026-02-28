use crate::{
    actions::trans_action::TransAction,
    model::{Activity, server::Server},
    widget::{active_mark, check_mark, context_menu_button, cross_mark, text_button},
};
use iced::{
    Color, Element, Length, Padding,
    widget::{Column, column, container, row, scrollable, text},
};
use iced_aw::{ContextMenu, TabLabel};
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Translation {
    pub server_state: Server,
    pub file_name: String,
    pub current_page: usize,
    pub pages: Vec<Page>,
}

impl Translation {
    pub fn tab_label(&self) -> TabLabel {
        let file_name = self.file_name.clone();
        match self.server_state.handles.is_empty() {
            true => TabLabel::Text(file_name),
            false => TabLabel::IconText('\u{25CF}', file_name),
        }
    }

    pub fn current_content(&self) -> Option<&[String]> {
        Some(self.pages.get(self.current_page)?.text.as_ref())
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
                    Activity::Error => Some(cross_mark()),
                    Activity::Active => Some(active_mark()),
                };

                let button_content = row![button_text]
                    .push(mark)
                    .padding(Padding::default().right(10));

                let count = page.sections.len();
                let active = self.server_state.connected() && self.server_state.handles.is_empty();
                let underlay = text_button(button_content).on_press(TransAction::SetPage(i));

                ContextMenu::new(underlay, move || {
                    path_button_overlay(count, name.to_string(), i, active)
                })
                .into()
            })
            .collect()
    }
}

fn path_button_overlay<'a>(
    count: usize,
    name: String,
    page: usize,
    active: bool,
) -> Element<'a, TransAction> {
    let part_buttons = (0..count).map(|i| {
        context_menu_button(text(format!("translate part {}", i + 1)).color(Color::WHITE))
            .on_press_maybe(active.then_some(TransAction::TranslatePart(page, i)))
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

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Page {
    pub path: PathBuf,
    pub text: Vec<String>,
    pub sections: Vec<String>,
    pub activity: Activity,
}

impl Page {
    pub fn new(path: PathBuf, sections: Vec<String>) -> Self {
        Page {
            text: vec![String::new(); sections.len()],
            activity: Activity::Incomplete,
            path,
            sections,
        }
    }

    pub fn clear_content(&mut self) {
        self.text.iter_mut().for_each(String::clear);
    }
}
