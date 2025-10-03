use epub::doc::EpubDoc;
use std::{
    io::{Read, Seek},
    path::PathBuf,
};

pub fn get_ordered_path<R: Read + Seek>(doc: &mut EpubDoc<R>) -> Vec<PathBuf> {
    (0..doc.get_num_pages())
        .into_iter()
        .map(|i| {
            doc.set_current_page(i);
            doc.get_current_path().unwrap()
        })
        .collect()
}
