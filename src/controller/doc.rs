use epub::doc::EpubDoc;
use std::{io::Cursor, path::PathBuf};

pub fn get_ordered_path(doc: &mut EpubDoc<Cursor<Vec<u8>>>) -> Vec<PathBuf> {
    (0..doc.get_num_pages())
        .into_iter()
        .map(|i| {
            doc.set_current_page(i);
            doc.get_current_path().unwrap()
        })
        .collect()
}
