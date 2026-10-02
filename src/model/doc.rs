use rbook::Epub;

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct Doc {
    pub epub: Option<Epub>,
    pub file_name: Option<String>,
    pub current_page: Option<usize>,
    pub total_pages: usize,
    pub content: String,
}
