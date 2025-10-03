use crate::message::Message;
use iced::Task;

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct DocModel {
    pub path: Option<String>,
    pub current_page: Option<usize>,
    pub total_pages: usize,
    pub content: String,
}

impl DocModel {
    pub fn inc_page(&self) -> Task<Message> {
        self.current_page.map_or(Task::none(), |page| {
            let page = page + 1;
            if page < self.total_pages {
                Task::done(Message::SelectPage(page))
            } else {
                Task::none()
            }
        })
    }

    pub fn dec_page(&self) -> Task<Message> {
        self.current_page
            .map_or(Task::none(), |page| match page.checked_sub(1) {
                Some(page) => Task::done(Message::SelectPage(page)),
                None => Task::none(),
            })
    }

    pub fn perform(&self, action: DocAction) -> Task<Message> {
        match action {
            DocAction::Inc => self.inc_page(),
            DocAction::Dec => self.dec_page(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum DocAction {
    Inc,
    Dec,
}
