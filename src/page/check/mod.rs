mod copy;
mod frequency;

use crate::{
    page::{Activity, Page, PageError, Section, TOC_PAGE_PREFIX},
    text::{contains_japanese, image_marker_indices},
};
use copy::LineSets;
use frequency::check_char_frequency;
use pulldown_cmark::{Event, Parser, Tag};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::ops::Range;

const MIN_PERCENT: f64 = 65.0;
const MAX_PERCENT: f64 = 110.0;
const SIZE_TOLERANCE: f64 = 12.5;
const SMALL_SECTION: usize = 1000;

impl Page {
    pub fn check_incomplete(&self) -> bool {
        self.sections.iter().any(|e| e.content.is_empty())
    }

    fn check_page(&mut self, prior: &[(&Section, &LineSets)], heads: &[LineSets]) {
        self.errors = [
            self.check_size(),
            self.check_japanese(),
            self.check_frequency(),
            self.check_copy(prior, heads),
            self.check_image_tags(),
            self.check_header_tags(),
            self.check_link_tags(),
            self.check_end_character(),
        ]
        .concat();
        self.errors.sort_by_key(|e| e.index);

        self.activity = if let Some(error) = self.errors.first() {
            Activity::Error(error.index + 1)
        } else if self.check_incomplete() {
            Activity::Incomplete
        } else {
            Activity::Complete
        };
    }

    pub fn check_japanese(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter_map(|(i, e)| contains_japanese(&e.content).then_some(PageError::japanese(i)))
            .collect()
    }

    pub fn check_frequency(&self) -> Vec<PageError> {
        self.sections
            .par_iter()
            .enumerate()
            .filter_map(|(i, e)| check_char_frequency(&e.content).then_some(PageError::repeat(i)))
            .collect()
    }

    pub fn check_size(&self) -> Vec<PageError> {
        let last = self.sections.len() - 1;
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                let p = (content.len() as f64 / japanese.len() as f64) * 100.0;
                let (min, max) = if i == last && japanese.chars().count() < SMALL_SECTION {
                    (MIN_PERCENT - SIZE_TOLERANCE, MAX_PERCENT + SIZE_TOLERANCE)
                } else {
                    (MIN_PERCENT, MAX_PERCENT)
                };
                let size_error = p <= min || p >= max;
                size_error.then_some(PageError::size(i, content.len()))
            })
            .collect()
    }

    fn check_image_tags(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                let japanese_tags = image_marker_indices(japanese);
                let content_tags = image_marker_indices(content);
                let dif = japanese_tags.symmetric_difference(&content_tags).count();
                (dif != 0).then_some(PageError::image(i))
            })
            .collect()
    }

    fn check_header_tags(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                (header_tag_count(&japanese) != header_tag_count(&content))
                    .then_some(PageError::header(i))
            })
            .collect()
    }

    fn check_link_tags(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                (link_tag_count(&japanese) != link_tag_count(&content))
                    .then_some(PageError::link(i))
            })
            .collect()
    }

    fn check_end_character(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                let jp = japanese.trim_end().chars().last()?.is_alphanumeric();
                let c = content.trim_end().chars().last()?.is_alphanumeric();
                (jp != c).then_some(PageError::end(i))
            })
            .collect()
    }
}

pub fn check_page_at(pages: &mut [Page], index: usize) {
    let next = index + 1;
    let check_next = pages
        .get(next)
        .is_some_and(|p| !p.active() && p.sections.iter().any(|s| !s.content.is_empty()));
    check_pages(pages, index..if check_next { next + 1 } else { next });
}

pub fn check_all_pages(pages: &mut [Page]) {
    check_pages(pages, 0..pages.len());
}

fn check_pages(pages: &mut [Page], range: Range<usize>) {
    // Always the heads of `pages[index - 1]`.
    let mut previous_heads = range
        .start
        .checked_sub(1)
        .and_then(|previous| pages.get(previous))
        .map(|p| LineSets::heads(&p.sections))
        .unwrap_or_default();

    for index in range {
        let Some((before, after)) = pages.split_at_mut_checked(index) else {
            return;
        };
        let Some(page) = after.first_mut() else {
            return;
        };
        let heads = LineSets::heads(&page.sections);
        let prior: Vec<_> =
            prior_page(before).map_or_default(|p| p.sections.iter().zip(&previous_heads).collect());
        page.check_page(&prior, &heads);
        previous_heads = heads;
    }
}

fn prior_page(before: &[Page]) -> Option<&Page> {
    before
        .last()
        .filter(|p| p.file_prefix().is_some_and(|f| f != TOC_PAGE_PREFIX))
}

fn header_tag_count(text: &str) -> usize {
    Parser::new(text)
        .filter(|e| matches!(e, Event::Start(Tag::Heading { .. })))
        .count()
}

fn link_tag_count(text: &str) -> usize {
    Parser::new(text)
        .filter(|e| matches!(e, Event::Start(Tag::Link { .. })))
        .count()
}

#[cfg(test)]
mod test {
    use super::check_page_at;
    use crate::page::{ErrorType, Page, Section};

    pub(super) fn section(japanese: &str, content: &str) -> Section {
        Section {
            japanese: japanese.into(),
            content: content.into(),
        }
    }

    pub(super) fn page(sections: Vec<Section>) -> Page {
        Page {
            sections,
            ..Default::default()
        }
    }

    #[test]
    fn rechecking_page_updates_next_page() {
        let tail = "On his thick wrist gleamed a resplendent golden guild ring.";
        let mut pages = vec![
            Page::new(
                "p-001.xhtml".into(),
                vec!["その太い手首には、黄金に輝くギルドリングが光っていた。".into()],
            ),
            Page::new(
                "p-002.xhtml".into(),
                vec!["受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。".into()],
            ),
        ];
        pages[0].sections[0].content = tail.into();
        pages[1].sections[0].content = tail.into();
        let has_copy = |p: &Page| p.errors.iter().any(|e| matches!(e.error, ErrorType::Copy));

        check_page_at(&mut pages, 1);
        assert!(has_copy(&pages[1]));

        pages[0].sections[0].content =
            "The man who appeared from behind the counter looked down at me.".into();
        check_page_at(&mut pages, 0);
        assert!(!has_copy(&pages[1]));
    }

    #[test]
    fn small_section_uses_wider_size_range() {
        // 900 characters / 2700 bytes of Japanese against 1650 bytes of English is 61%.
        let japanese = "あ".repeat(900);
        let content = "a".repeat(1650);
        let small = page(vec![section(&japanese, &content)]);
        assert!(small.check_size().is_empty());

        let japanese = "あ".repeat(1200);
        let content = "a".repeat(2200);
        let large = page(vec![section(&japanese, &content)]);
        assert_eq!(large.check_size().len(), 1);
    }
}
