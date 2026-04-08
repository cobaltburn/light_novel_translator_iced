use crate::{
    actions::trans_action::TransAction,
    message::Message,
    model::{
        doc::Doc, extraction::Extraction, format::Format, server::Server, translation::Translation,
    },
    view::View,
};
use iced::{Function, Task};
use std::collections::BTreeMap;

#[non_exhaustive]
#[derive(Debug)]
pub struct Translator {
    pub view: View,
    pub side_bar_collapsed: bool,
    pub active_tab: usize,
    pub doc: Doc,
    pub translations: BTreeMap<usize, Translation>,
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
            translations: BTreeMap::from([(0, Translation::default())]),
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

    pub fn add_tab(&mut self) {
        if let Some(key) = self.translations.keys().max() {
            let key = key + 1;
            let server = &self.translations.get(&self.active_tab).unwrap().server;
            self.translations.insert(
                key,
                Translation::from(Server {
                    client: server.client.clone(),
                    models: server.models.clone(),
                    current_model: server.current_model.clone(),
                    settings: server.settings.clone(),
                    method: server.method.clone(),
                    handles: Vec::new(),
                }),
            );
            self.active_tab = key
        }
    }

    pub fn close_tab(&mut self, tab: usize) {
        self.translations.remove(&tab);

        if self.translations.contains_key(&self.active_tab) {
            return;
        }

        if let Some(&tab) = self.translations.keys().find(|&&i| i > self.active_tab) {
            self.active_tab = tab;
        } else if let Some(&tab) = self.translations.keys().max() {
            self.active_tab = tab;
        }
    }

    pub fn translation_action(&mut self, tab: usize, action: TransAction) -> Task<Message> {
        match self.translations.get_mut(&tab) {
            Some(model) => model
                .perform(action)
                .map(Message::TranslationAction.with(tab)),
            None => Task::none(),
        }
    }
}
