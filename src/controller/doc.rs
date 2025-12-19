use epub::doc::EpubDoc;
use std::{io::Cursor, path::PathBuf};

pub fn get_ordered_path(epub: &EpubDoc<Cursor<Vec<u8>>>) -> Vec<PathBuf> {
    epub.spine
        .iter()
        .map(|e| epub.resources.get(&e.idref).unwrap().path.clone())
        .collect()
}
