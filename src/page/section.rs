use crate::text::clean_invisible_chars;
use rig::message::Message;
use serde::{Deserialize, Serialize};

const SECTION_CAPACITY: usize = 8 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Section {
    pub japanese: String,
    pub content: String,
}

impl Section {
    pub fn new(japanese: String) -> Self {
        Self {
            japanese,
            content: String::with_capacity(SECTION_CAPACITY),
        }
    }

    pub fn history_message(&self) -> [Message; 2] {
        [
            Message::user(&self.japanese),
            Message::assistant(&self.content),
        ]
    }

    pub fn clean(&mut self) {
        self.content = clean_invisible_chars(&self.content);
        self.content = self.content.replace(['“', '”'], "\"");
        self.split_lines();
    }

    fn split_lines(&mut self) {
        let lines: Vec<_> = self.content.lines().collect();
        let mut text = Vec::new();
        for [l1, l2] in lines.array_windows() {
            text.push(*l1);
            if !l1.is_empty() && !l2.is_empty() {
                text.push("");
            }
        }

        if let Some(last) = lines.last() {
            text.push(last);
        };

        self.content = text.join("\n")
    }
}
