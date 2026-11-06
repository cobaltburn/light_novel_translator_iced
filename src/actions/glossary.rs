use crate::{
    actions::{get_pages, save_file, server},
    controller::{GLOSSARY_PROMPT, glossary_prompt, remove_think_tags},
    error::{Error, Result, ResultTaskExt, TaskResultExt},
    message::{error_dialog, select_epub},
    model::{Glossary, Page},
};
use iced::Task;
use std::{collections::VecDeque, mem, path::PathBuf};

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Action {
    ServerAction(server::Action),
    SetEpub { path: PathBuf, pages: Vec<Page> },
    OpenEpub,
    Glossary,
    NextPart,
    Cancel,
    Failed(String),
    Save,
    UpdateContent(String),
}

impl Glossary {
    #[allow(clippy::unit_arg)]
    pub fn perform(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::ServerAction(action) => self.server.perform(action).map(Into::into),
            Action::OpenEpub => Task::future(select_epub())
                .and_then(|(path, buf)| Task::future(get_pages(path, buf)))
                .ok_or_display(|(path, pages)| Task::done(Action::SetEpub { path, pages })),
            Action::SetEpub { path, pages } => self.set_epub(path, pages).into(),
            Action::UpdateContent(content) => self.update_content(content).into(),
            Action::Glossary => self.glossary().ok_or_display(),
            Action::NextPart => self.advance().ok_or_display(),
            Action::Cancel => self.stop().into(),
            Action::Failed(message) => self.fail(message),
            Action::Save => self.save_content().ok_or_display(Into::into),
        }
    }

    pub fn set_epub(&mut self, path: PathBuf, pages: Vec<Page>) {
        self.stop();
        self.content.clear();
        self.file_path = Some(path);
        self.pages = pages;
    }

    pub fn update_content(&mut self, content: String) {
        self.content.push_str(&content);
    }

    pub fn glossary(&mut self) -> Result<Task<Action>> {
        self.server.check_ready()?;
        let sections = self.pages.iter().flat_map(|page| &page.sections);
        self.pending = sections.map(|s| s.japanese.clone()).collect();
        if self.pending.is_empty() {
            return Err(Error::Error("No text to read in this epub"));
        }
        self.completed.clear();
        self.content.clear();
        self.advance()
    }

    fn advance(&mut self) -> Result<Task<Action>> {
        self.next_part().inspect_err(|_| self.stop())
    }

    /// Keeps the pass that just ended, then sends the next part of the book
    /// along with it. Parts are sized from the glossary so far, so a part is
    /// only cut once the previous pass has finished.
    fn next_part(&mut self) -> Result<Task<Action>> {
        let output = remove_think_tags(&self.content);
        if !output.trim().is_empty() {
            self.completed = output;
        }
        if self.pending.is_empty() {
            self.stop();
            return Ok(Task::none());
        }

        let model = self.server.check_ready()?;
        let budget = part_budget(self.server.settings.num_ctx, &self.completed);
        let part = take_part(&mut self.pending, budget).ok_or(Error::Error(
            "num_ctx is too small for the glossary and the next part of the book",
        ))?;
        let task = self
            .server
            .glossary(&model, glossary_prompt(&self.completed, &part))?;
        self.content.clear();

        // Bound, so cancelling never starts another pass.
        let next = self.server.bind_handle(Task::done(Action::NextPart));
        Ok(task.chain(next))
    }

    /// Ends the run. Safe to call when idle. A pass that was cut short gives
    /// way to the last complete glossary, because each pass rewrites it whole.
    fn stop(&mut self) {
        self.pending.clear();
        if !self.completed.is_empty() {
            self.content = mem::take(&mut self.completed);
        }
        self.server.abort();
    }

    /// Stops the run and shows `message`.
    fn fail(&mut self, message: String) -> Task<Action> {
        self.stop();
        Task::future(error_dialog(message)).discard()
    }

    fn save_content(&self) -> Task<Result<()>> {
        let Some(file_stem) = self.file_stem() else {
            return Task::none();
        };
        let content = remove_think_tags(&self.content);
        Task::future(save_file(format!("{file_stem}.md"), content))
    }
}

/// Characters of book text that fit in one pass. Counts one token per
/// character, which is on the safe side for Japanese. `num_ctx` also has to
/// hold the reply, so `existing` counts twice, once in and once back out, and
/// a quarter is kept for what the pass adds and for reasoning.
fn part_budget(num_ctx: u32, existing: &str) -> usize {
    let reserved = GLOSSARY_PROMPT.chars().count() + existing.chars().count() * 2;
    (num_ctx as usize * 3 / 4).saturating_sub(reserved)
}

/// Removes as many leading sections as fit in `budget` characters and joins
/// them. Returns `None`, leaving `sections` alone, if the first does not fit.
fn take_part(sections: &mut VecDeque<String>, budget: usize) -> Option<String> {
    let mut used = 0;
    let count = sections
        .iter()
        .take_while(|section| {
            used += section.chars().count();
            used <= budget
        })
        .count();
    (count > 0).then(|| sections.drain(..count).collect::<Vec<_>>().join("\n\n"))
}

impl From<server::Action> for Action {
    fn from(action: server::Action) -> Self {
        Action::ServerAction(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sections(texts: &[&str]) -> VecDeque<String> {
        texts.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn take_part_stops_at_the_budget_and_keeps_the_rest() {
        let mut pending = sections(&["一二三", "四五", "六七八"]);

        assert_eq!(take_part(&mut pending, 5).unwrap(), "一二三\n\n四五");
        assert_eq!(pending, sections(&["六七八"]));
    }

    #[test]
    fn take_part_refuses_a_section_it_cannot_fit() {
        let mut pending = sections(&["一二三"]);

        assert_eq!(take_part(&mut pending, 2), None);
        assert_eq!(pending, sections(&["一二三"]));
    }

    #[test]
    fn part_budget_shrinks_as_the_glossary_grows() {
        let empty = part_budget(100_000, "");
        let grown = part_budget(100_000, "用語集");

        assert_eq!(grown, empty - 6);
        assert_eq!(part_budget(100, ""), 0);
    }

    #[test]
    fn stop_restores_the_last_complete_glossary() {
        let mut model = Glossary {
            completed: "complete".to_string(),
            content: "cut sh".to_string(),
            pending: sections(&["一"]),
            ..Default::default()
        };

        model.stop();

        assert_eq!(model.content, "complete");
        assert!(model.pending.is_empty());
    }

    #[test]
    fn stop_keeps_a_first_pass_with_nothing_complete_to_restore() {
        let mut model = Glossary {
            content: "cut sh".to_string(),
            ..Default::default()
        };

        model.stop();

        assert_eq!(model.content, "cut sh");
    }

    #[test]
    fn failed_ends_the_run_without_waiting_for_the_dialog() {
        let mut model = Glossary {
            completed: "complete".to_string(),
            content: "cut sh".to_string(),
            pending: sections(&["一"]),
            ..Default::default()
        };
        _ = model.server.bind_handle(Task::<()>::none());

        _ = model.perform(Action::Failed("offline".to_string()));

        assert!(model.server.handles.is_empty());
        assert!(model.pending.is_empty());
        assert_eq!(model.content, "complete");
    }

    #[test]
    fn set_epub_drops_the_previous_books_glossary_and_run() {
        let mut model = Glossary {
            completed: "old book".to_string(),
            content: "old book, part 2".to_string(),
            pending: sections(&["一"]),
            ..Default::default()
        };

        model.set_epub(PathBuf::from("new.epub"), Vec::new());

        assert_eq!(model.content, "");
        assert_eq!(model.completed, "");
        assert!(model.pending.is_empty());
    }

    #[test]
    fn next_part_finishes_with_cleaned_output_once_nothing_is_pending() {
        let mut model = Glossary {
            completed: "earlier".to_string(),
            content: "<think>guess</think>final".to_string(),
            ..Default::default()
        };

        assert!(model.next_part().is_ok());

        assert_eq!(model.content, "final");
    }

    #[test]
    fn next_part_keeps_the_glossary_when_a_pass_returns_only_reasoning() {
        let mut model = Glossary {
            completed: "earlier".to_string(),
            content: "<think>nothing</think>".to_string(),
            ..Default::default()
        };

        assert!(model.next_part().is_ok());

        assert_eq!(model.content, "earlier");
    }
}
