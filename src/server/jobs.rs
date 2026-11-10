//! The translation and consensus runs. Each part of a page becomes a task bound
//! to the server, so that cancelling a run aborts all of them.

use crate::{
    error::{Error, Result},
    llm::{
        Reply, SharedHistory, StreamAction, compact_history, consensus_prompt, fail_run,
        reply_stream, seed_history, shared_history, stream_task,
    },
    page::{Page, Section},
    screens::glossary,
    server::{Method, Server, state::bind},
};
use iced::Task;
use rig::{Agent, message::Message};
use std::{collections::HashMap, ffi::OsStr};

const GLOSSARY_CUT_OFF: &str = "The model hit its limit partway through a glossary pass, so the \
     run stopped and kept the last complete pass. Try a model with a larger context window.";

impl Server {
    pub fn translate<A: StreamAction>(
        &mut self,
        pages: &[Page],
        model: &str,
        page: usize,
        glossary: Option<&str>,
    ) -> Result<Task<A>> {
        let (current, pages) = pages.split_last().expect("dont pass an empty array");
        let (seed, history) = self.seeded_history(newest_first(pages)).unzip();
        let agent =
            self.client
                .translation_agent(model, &self.settings, history.clone(), glossary)?;

        let handles = &mut self.handles;
        let seed = seed.map_or_default(|seed| bind(handles, seed));
        let tasks: Vec<_> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                let compact = history.clone().map_or_default(compact_history);
                stream_task(&agent, compact, &section.japanese, page, part)
            })
            .map(|task| bind(handles, task))
            .collect();

        Ok(seed.chain(self.method.join_tasks(tasks)))
    }

    pub fn translate_part<A: StreamAction>(
        &mut self,
        pages: &[Page],
        model: &str,
        page: usize,
        part: usize,
        glossary: Option<&str>,
    ) -> Result<Task<A>> {
        let (Page { sections, .. }, pages) = pages.split_last().unwrap();
        let (section, current_sections) = sections
            .get(..part + 1)
            .and_then(|p| p.split_last())
            .unwrap();
        let recent = current_sections.iter().rev().chain(newest_first(pages));
        let (seed, history) = self.seeded_history(recent).unzip();
        let agent = self
            .client
            .translation_agent(model, &self.settings, history, glossary)?;
        let seed = seed.unwrap_or_default();
        let task = stream_task(&agent, seed, &section.japanese, page, part);

        Ok(self.bind_handle(task))
    }

    /// In History mode, returns a task seeding a new shared history from `recent`
    /// (sections ordered newest first) along with that history.
    fn seeded_history<'a, A: StreamAction>(
        &self,
        recent: impl Iterator<Item = &'a Section>,
    ) -> Option<(Task<A>, SharedHistory)> {
        if self.method != Method::History {
            return None;
        }
        let history = shared_history(self.settings.context_window);
        let messages = build_history(recent, self.settings.context_window);
        Some((seed_history(history.clone(), messages), history))
    }

    pub fn consensus<A: StreamAction>(
        &mut self,
        pages: &[Page],
        candidates: HashMap<&OsStr, Vec<&[String]>>,
        model: &str,
        page: usize,
    ) -> Result<Task<A>> {
        let current = pages.last().expect("dont pass an empty array");
        let candidates = page_candidates(current, &candidates)?;
        let agent = self.client.consensus_agent(model, &self.settings)?;

        let handles = &mut self.handles;
        let tasks = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                consensus_task(&agent, section, candidates, page, part)
                    .map(|task| bind(handles, task))
            })
            .collect::<Result<_>>()?;

        Ok(self.method.join_tasks(tasks))
    }

    pub fn consensus_part<A: StreamAction>(
        &mut self,
        pages: &[Page],
        candidates: HashMap<&OsStr, Vec<&[String]>>,
        model: &str,
        page: usize,
        part: usize,
    ) -> Result<Task<A>> {
        let current = pages.last().expect("dont pass an empty array");
        let section = current.sections.get(part).unwrap();
        let candidates = page_candidates(current, &candidates)?;
        let agent = self.client.consensus_agent(model, &self.settings)?;
        let task = consensus_task(&agent, section, candidates, page, part)?;

        Ok(self.bind_handle(task))
    }

    /// Streams one pass of the glossary into the content.
    pub fn glossary(&mut self, chapter: usize, prompt: String) -> Result<Task<glossary::Action>> {
        let model = self.check_ready()?;
        let agent = self.client.glossary_agent(&model, &self.settings)?;
        let task = reply_stream(agent, prompt).then(move |reply| match reply {
            Ok(Reply::Text(text)) => Task::done(glossary::Action::UpdateContent(chapter, text)),
            Ok(Reply::Truncated) => {
                fail_run(Error::Error(GLOSSARY_CUT_OFF), glossary::Action::Failed)
            }
            Err(error) => fail_run(error, glossary::Action::Failed),
        });
        Ok(self.bind_handle(task))
    }
}

fn page_candidates<'a, 'b>(
    page: &Page,
    candidates: &'a HashMap<&OsStr, Vec<&'b [String]>>,
) -> Result<&'a [&'b [String]]> {
    candidates
        .get(page.file_prefix().unwrap_or_default())
        .map(Vec::as_slice)
        .ok_or(Error::Error("missing candidate files"))
}

fn consensus_task<A: StreamAction>(
    agent: &Agent,
    section: &Section,
    candidates: &[&[String]],
    page: usize,
    part: usize,
) -> Result<Task<A>> {
    let candidates: Vec<_> = candidates.iter().flat_map(|e| e.get(part)).collect();
    let prompt = consensus_prompt(&section.japanese, &candidates)?;

    Ok(stream_task(agent, Task::none(), prompt, page, part))
}

/// Sections of `pages` ordered newest first.
fn newest_first(pages: &[Page]) -> impl Iterator<Item = &Section> {
    pages.iter().rev().flat_map(|p| p.sections.iter().rev())
}

/// Builds history messages from the latest `context_window` translated sections,
/// given `recent` ordered newest first. Stops walking once the window is full.
fn build_history<'a>(
    recent: impl Iterator<Item = &'a Section>,
    context_window: usize,
) -> Vec<Message> {
    let recent: Vec<_> = recent
        .filter(|s| !s.content.is_empty())
        .take(context_window)
        .collect();

    recent
        .into_iter()
        .rev()
        .flat_map(Section::history_message)
        .collect()
}
