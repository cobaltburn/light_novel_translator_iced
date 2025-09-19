#[non_exhaustive]
#[derive(Default, Debug)]
pub struct FormatModel {
    pub pages: Vec<FormatPage>,
    pub current_page: Option<usize>,
}

impl FormatModel {
    pub fn current_content(&self) -> Option<&str> {
        let page = self.current_page?;
        self.pages.get(page).map(|e| e.content.as_str())
    }
}

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct FormatPage {
    pub name: String,
    pub content: String,
    pub changed: bool,
}
