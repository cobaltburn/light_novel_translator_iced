use crate::message::Message;
use iced::Task;

#[non_exhaustive]
#[derive(Default)]
pub struct DocModel {
    pub path: Option<String>,
    pub current_page: Option<usize>,
    pub total_pages: usize,
    pub content: String,
}

impl DocModel {
    pub fn inc_page(&mut self) -> Task<Message> {
        self.current_page.map_or(Task::none(), |page| {
            let page = page + 1;
            if page < self.total_pages {
                Task::done(Message::SelectPage(page))
            } else {
                Task::none()
            }
        })
    }

    pub fn dec_page(&mut self) -> Task<Message> {
        self.current_page
            .map_or(Task::none(), |page| match page.checked_sub(1) {
                Some(page) => Task::done(Message::SelectPage(page)),
                None => Task::none(),
            })
    }
}
