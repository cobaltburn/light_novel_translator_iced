use crate::{
    actions::trans_action::TransAction,
    message::Message,
    state::{doc_model::DocModel, format_model::FormatModel, translation_model::TranslationModel},
    view::View,
};
use iced::{Function, Task};

#[non_exhaustive]
#[derive(Debug)]
pub struct Translator {
    pub view: View,
    pub side_bar_collapsed: bool,
    pub current_tab: usize,
    pub doc_model: DocModel,
    pub translation_models: Vec<TranslationModel>,
    pub format_model: FormatModel,
}

impl Default for Translator {
    fn default() -> Self {
        Self {
            view: Default::default(),
            side_bar_collapsed: Default::default(),
            current_tab: Default::default(),
            doc_model: Default::default(),
            translation_models: vec![
                TranslationModel::default(),
                TranslationModel::default(),
                TranslationModel::default(),
                TranslationModel::default(),
                TranslationModel::default(),
            ],
            format_model: Default::default(),
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
        self.current_tab = tab;
    }

    pub fn close_tab(&mut self, tab: usize) {
        self.translation_models.remove(tab);

        if self.translation_models.get(self.current_tab).is_none() {
            self.current_tab -= 1;
        }
    }

    pub fn translation_action(&mut self, tab: usize, action: TransAction) -> Task<Message> {
        match self.translation_models.get_mut(tab) {
            Some(model) => model
                .perform(action)
                .map(Message::TranslationAction.with(tab)),
            None => Task::none(),
        }
    }
}
