use crate::{
    actions::extraction_action::ExtractAction,
    model::server::Server,
    widget::{check_mark, text_button},
};
use base64::{Engine, engine::general_purpose};
use iced::{
    Length, Padding,
    alignment::Vertical,
    widget::{Column, checkbox, image::Handle, row, text},
};

#[derive(Default, Debug)]
pub struct Extraction {
    pub server_state: Server,
    pub current_page: usize,
    pub pages: Vec<ImagePage>,
    pub image_view: ImageView,
}

impl Extraction {
    pub fn image_buttons(&self) -> Column<'_, ExtractAction> {
        self.pages
            .iter()
            .enumerate()
            .map(|(i, page)| {
                let button_text = text!("{}. {}", i + 1, &page.name)
                    .width(Length::Fill)
                    .style(move |theme| {
                        if self.current_page == i {
                            text::primary(theme)
                        } else {
                            text::default(theme)
                        }
                    });

                text_button(
                    row![
                        button_text,
                        page.complete.then_some(check_mark()),
                        checkbox(page.checked).on_toggle(move |_| ExtractAction::CheckToggle(i)),
                    ]
                    .padding(Padding::default().right(5))
                    .align_y(Vertical::Center),
                )
                .on_press(ExtractAction::SetPage(i))
                .into()
            })
            .collect()
    }

    pub fn get_current_page(&self) -> Option<&ImagePage> {
        self.pages.get(self.current_page)
    }
}

#[derive(Debug, Clone)]
pub struct ImagePage {
    pub name: String,
    pub handle: Handle,
    pub base64: String,
    pub checked: bool,
    pub content: String,
    pub complete: bool,
}

impl ImagePage {
    pub fn from_bytes(name: String, bytes: Vec<u8>) -> Self {
        ImagePage {
            name,
            base64: general_purpose::STANDARD.encode(&bytes),
            handle: Handle::from_bytes(bytes),
            checked: true,
            content: String::new(),
            complete: false,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImageView {
    #[default]
    Image,
    Text,
    Split,
}
