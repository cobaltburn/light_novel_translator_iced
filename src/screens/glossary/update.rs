use crate::{
    dialogs::{error_dialog, save_file, select_epub},
    epub::get_chapters,
    error::{Error, Result, ResultTaskExt, TaskResultExt},
    llm::{Reply, fail_run, glossary_prompt, reply_stream},
    screens::glossary::Glossary,
    server,
    text::remove_think_tags,
};
use iced::Task;
use std::{mem, path::PathBuf};

const GLOSSARY_CUT_OFF: &str = "The model hit its limit partway through a glossary pass, so the \
     run stopped and kept the last complete pass. Try a model with a larger context window.";

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Action {
    Server(server::Action),
    SetEpub {
        path: PathBuf,
        chapters: Vec<String>,
    },
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
            Action::Server(action) => self.server.perform(action).map(Into::into),
            Action::OpenEpub => Task::future(select_epub())
                .and_then(|(path, buf)| Task::future(get_chapters(path, buf)))
                .ok_or_display(|(path, chapters)| Task::done(Action::SetEpub { path, chapters })),
            Action::SetEpub { path, chapters } => self.set_epub(path, chapters).into(),
            Action::UpdateContent(content) => self.update_content(content).into(),
            Action::Glossary => self.glossary().ok_or_display(),
            Action::NextPart => self.advance().ok_or_display(),
            Action::Cancel => self.stop().into(),
            Action::Failed(message) => self.fail(message),
            Action::Save => self.save_content().ok_or_display(Into::into),
        }
    }

    pub fn set_epub(&mut self, path: PathBuf, chapters: Vec<String>) {
        self.stop();
        self.content.clear();
        self.file_path = Some(path);
        self.chapters = chapters;
    }

    pub fn update_content(&mut self, content: String) {
        self.content.push_str(&content);
    }

    pub fn glossary(&mut self) -> Result<Task<Action>> {
        self.server.check_ready()?;
        self.pending = self.chapters.iter().cloned().collect();
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
    /// along with it.
    fn next_part(&mut self) -> Result<Task<Action>> {
        let output = remove_think_tags(&self.content);
        if !output.trim().is_empty() {
            self.completed = output;
        }
        let Some(text) = self.pending.pop_front() else {
            self.stop();
            return Ok(Task::none());
        };

        let task = self.pass_task(glossary_prompt(&self.completed, &text))?;
        self.content.clear();

        let next = self.server.bind_handle(Task::done(Action::NextPart));
        Ok(task.chain(next))
    }

    /// Streams one pass of the glossary into the content.
    fn pass_task(&mut self, prompt: String) -> Result<Task<Action>> {
        let model = self.server.check_ready()?;
        let agent = self
            .server
            .client
            .glossary_agent(&model, &self.server.settings)?;
        let task = reply_stream(agent, prompt).then(|reply| match reply {
            Ok(Reply::Text(text)) => Task::done(Action::UpdateContent(text)),
            Ok(Reply::Truncated) => fail_run(Error::Error(GLOSSARY_CUT_OFF), Action::Failed),
            Err(error) => fail_run(error, Action::Failed),
        });
        Ok(self.server.bind_handle(task))
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

impl From<server::Action> for Action {
    fn from(action: server::Action) -> Self {
        Action::Server(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn chapters(texts: &[&str]) -> VecDeque<String> {
        texts.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn stop_restores_the_last_complete_glossary() {
        let mut model = Glossary {
            completed: "complete".to_string(),
            content: "cut sh".to_string(),
            pending: chapters(&["一"]),
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
            pending: chapters(&["一"]),
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
            pending: chapters(&["一"]),
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

        _ = model.next_part().unwrap();

        assert_eq!(model.content, "final");
    }

    #[test]
    fn next_part_keeps_the_glossary_when_a_pass_returns_only_reasoning() {
        let mut model = Glossary {
            completed: "earlier".to_string(),
            content: "<think>nothing</think>".to_string(),
            ..Default::default()
        };

        _ = model.next_part().unwrap();

        assert_eq!(model.content, "earlier");
    }
}
