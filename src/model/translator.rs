use crate::{
    actions::trans_action::TransAction,
    message::Message,
    model::{doc::Doc, extraction::Extraction, format::Format, translation::Translation},
    view::View,
};
use iced::{Function, Task};

#[non_exhaustive]
#[derive(Debug)]
pub struct Translator {
    pub view: View,
    pub side_bar_collapsed: bool,
    pub active_tab: usize,
    pub doc: Doc,
    pub translations: Vec<Translation>,
    pub format: Format,
    pub extraction: Extraction,
}

impl Default for Translator {
    fn default() -> Self {
        Self {
            view: Default::default(),
            side_bar_collapsed: Default::default(),
            active_tab: Default::default(),
            doc: Default::default(),
            translations: vec![
                Translation::default(),
                Translation::default(),
                Translation::default(),
                Translation::default(),
                Translation::default(),
            ],
            format: Default::default(),
            extraction: Extraction::default(),
        }
    }
}

impl Translator {
    pub fn set_view(&mut self, view: View) {
        self.view = view;
    }

    pub fn toggle_side_bar_collapse(&mut self) {
        self.side_bar_collapsed = !self.side_bar_collapsed;
    }

    pub fn set_tab(&mut self, tab: usize) {
        self.active_tab = tab;
    }

    pub fn close_tab(&mut self, tab: usize) {
        self.translations.remove(tab);

        if self.translations.get(self.active_tab).is_none() {
            self.active_tab -= 1;
        }
    }

    pub fn translation_action(&mut self, tab: usize, action: TransAction) -> Task<Message> {
        match self.translations.get_mut(tab) {
            Some(model) => model
                .perform(action)
                .map(Message::TranslationAction.with(tab)),
            None => Task::none(),
        }
    }
}
