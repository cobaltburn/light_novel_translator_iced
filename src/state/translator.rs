use crate::{
    state::{doc_model::DocModel, format_model::FormatModel, translation_model::TranslationModel},
    view::View,
};
use epub::doc::EpubDoc;
use std::io::Cursor;

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct Translator {
    pub view: View,
    pub epub: Option<EpubDoc<Cursor<Vec<u8>>>>,
    pub epub_name: String,
    pub side_bar_collapsed: bool,
    pub doc_model: DocModel,
    pub translation_model: TranslationModel,
    pub format_model: FormatModel,
}

impl Translator {
    pub fn set_view(&mut self, view: View) {
        self.view = view;
    }

    pub fn toggle_side_bar_collapse(&mut self) {
        self.side_bar_collapsed = !self.side_bar_collapsed;
    }
}
