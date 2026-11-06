//! Finds sections the model repeated instead of translating: whole sections
//! copied from earlier, and text re-translated across a section boundary.

use crate::{
    page::{Page, PageError, Section},
    text::IMAGE_MARKER_RE,
};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::{collections::HashSet, hash::Hash};

const JACCARD_TOLERANCE: f64 = 0.25;
const BOUNDARY_PARAGRAPHS: usize = 5;
const BOUNDARY_MIN_WORDS: usize = 5;
const BOUNDARY_MIN_BIGRAMS: usize = 5;
const BOUNDARY_TOLERANCE: f64 = 0.3;
const BOUNDARY_JAPANESE_TOLERANCE: f64 = 0.5;
const HEAD_MIN_LINES: usize = 2;

impl Page {
    pub(super) fn check_copy(
        &self,
        prior: &[(&Section, &LineSets)],
        heads: &[LineSets],
    ) -> Vec<PageError> {
        debug_assert_eq!(self.sections.len(), heads.len());
        let full: Vec<_> = prior
            .iter()
            .copied()
            .chain(self.sections.iter().zip(heads))
            .collect();
        let word_sets: Vec<HashSet<&str>> = full
            .par_iter()
            .map(|(s, _)| s.content.split_whitespace().collect())
            .collect();
        let prior_len = prior.len();
        self.sections
            .par_iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, _)| {
                let index = prior_len + i;
                let words = &word_sets[index];
                let (_, head) = full[index];
                let duplicate = word_sets[..index]
                    .iter()
                    .any(|b| !b.is_empty() && set_jaccard(words, b) > JACCARD_TOLERANCE);
                let overlap = || {
                    index
                        .checked_sub(1)
                        .map(|previous| full[previous].0)
                        .is_some_and(|p| {
                            !p.content.is_empty() && sections_overlap(&LineSets::tail(p), head)
                        })
                };
                let repeated_head = || {
                    full[..index]
                        .iter()
                        .any(|(p, h)| !p.content.is_empty() && head_repeated(h, head))
                };
                (duplicate || overlap() || repeated_head()).then_some(PageError::copy(i))
            })
            .collect()
    }
}

/// The word set of the English and bigram set of the Japanese for the lines at
/// one end of a section, each compared as a whole by the copy checks.
pub(super) struct LineSets {
    english: HashSet<String>,
    japanese: HashSet<(char, char)>,
    /// English lines that went into `english`.
    lines: usize,
}

impl LineSets {
    pub(super) fn heads(sections: &[Section]) -> Vec<Self> {
        sections.par_iter().map(Self::head).collect()
    }

    fn head(section: &Section) -> Self {
        Self::new(section.content.lines(), section.japanese.lines())
    }

    fn tail(section: &Section) -> Self {
        Self::new(
            section.content.lines().rev(),
            section.japanese.lines().rev(),
        )
    }

    fn new<'a>(
        english: impl Iterator<Item = &'a str>,
        japanese: impl Iterator<Item = &'a str>,
    ) -> Self {
        let (lines, english) = line_set(english, BOUNDARY_PARAGRAPHS, BOUNDARY_MIN_WORDS, word_set);
        let (_, japanese) = line_set(
            japanese,
            BOUNDARY_PARAGRAPHS * 2,
            BOUNDARY_MIN_BIGRAMS,
            bigram_set,
        );
        Self {
            english,
            japanese,
            lines,
        }
    }
}

fn sections_overlap(prev: &LineSets, next: &LineSets) -> bool {
    set_jaccard(&prev.english, &next.english) > BOUNDARY_TOLERANCE
        && set_jaccard(&prev.japanese, &next.japanese) <= BOUNDARY_JAPANESE_TOLERANCE
}

fn head_repeated(earlier: &LineSets, next: &LineSets) -> bool {
    earlier.lines >= HEAD_MIN_LINES && sections_overlap(earlier, next)
}

fn set_jaccard<T: Eq + Hash>(a: &HashSet<T>, b: &HashSet<T>) -> f64 {
    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn line_set<'a, T: Eq + Hash>(
    lines: impl Iterator<Item = &'a str>,
    n: usize,
    min_size: usize,
    to_set: impl Fn(&str) -> HashSet<T>,
) -> (usize, HashSet<T>) {
    lines
        .map(|line| to_set(IMAGE_MARKER_RE.replace_all(line, "").trim()))
        .filter(|set| set.len() >= min_size)
        .take(n)
        .fold((0, HashSet::new()), |(count, mut union), set| {
            union.extend(set);
            (count + 1, union)
        })
}

fn word_set(line: &str) -> HashSet<String> {
    line.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn bigram_set(line: &str) -> HashSet<(char, char)> {
    line.chars().zip(line.chars().skip(1)).collect()
}

#[cfg(test)]
mod test {
    use super::{LineSets, head_repeated, sections_overlap};
    use crate::page::{
        ErrorType, Section,
        check::test::{page, section},
        check_all_pages,
    };
    use std::slice;

    fn boundary_overlap(prev: &Section, next: &Section) -> bool {
        sections_overlap(&LineSets::tail(prev), &LineSets::head(next))
    }

    fn head_overlap(earlier: &Section, next: &Section) -> bool {
        head_repeated(&LineSets::head(earlier), &LineSets::head(next))
    }

    #[test]
    fn detects_retranslated_short_dialogue() {
        let prev = section(
            "ハーデン侯爵は口の端を大きく吊り上げて、意地の悪い笑みを浮かべた。\n「そういった狙いでございましたか。感服いたしました」",
            "Marquis Harden gave a malicious smile.\n\n\"So that was your aim. I am in awe.\"",
        );
        let next = section(
            "「……とはいえアイザス伯は、密偵など警戒もせんだろうがね」",
            "\"So that was your aim. I am in awe.\"\n\n\"...That said, Count Aizas likely isn't wary of spies at all.\"",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn scene_break_does_not_hide_overlap() {
        let prev = section(
            "その太い手首には、黄金に輝くギルドリングが光っていた。\n◇◇◇◇◇◇◇◇◇◇",
            "On his thick wrist gleamed a resplendent golden guild ring.\n\n◇◇◇◇◇◇◇◇◇◇",
        );
        let next = section(
            "◇◇◇◇◇◇◇◇◇◇\n受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
            "◇◇◇◇◇◇◇◇◇◇\n\nOn his thick wrist gleamed a resplendent golden guild ring.",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn image_markers_do_not_hide_overlap() {
        let prev = section(
            "その太い手首には、黄金に輝くギルドリングが光っていた。\n[[IMG: 3]]",
            "On his thick wrist gleamed a resplendent golden guild ring.\n\n\\[\\[IMG: 3\\]\\]",
        );
        let next = section(
            "[[IMG: 4]]\n受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
            "\\[\\[IMG: 4\\]\\]\n\nOn his thick wrist gleamed a resplendent golden guild ring.",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn detects_retranslated_status_line() {
        let prev = section(
            "【スキル獲得：剣術熟練度レベル３】",
            "[Skill acquired: Sword Mastery Lv. 3]",
        );
        let next = section(
            "受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
            "[Skill acquired: Sword Mastery Lv. 3]\n\nThe man who appeared from behind the counter looked down at me.",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn duplicate_and_overlap_report_one_copy_error() {
        let text = "On his thick wrist gleamed a resplendent golden guild ring.";
        let mut page = page(vec![
            section(
                "その太い手首には、黄金に輝くギルドリングが光っていた。",
                text,
            ),
            section(
                "受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
                text,
            ),
        ]);
        check_all_pages(slice::from_mut(&mut page));
        let copies: Vec<_> = page
            .errors
            .iter()
            .filter(|e| matches!(e.error, ErrorType::Copy))
            .map(|e| e.index)
            .collect();
        assert_eq!(copies, [1]);
    }

    #[test]
    fn detects_retranslated_tail() {
        let prev = section(
            "ハーデン侯爵は口の端を大きく吊り上げて、意地の悪い笑みを浮かべた。\n「そういった狙いでございましたか。感服いたしました」",
            "Marquis Harden curled up the corners of his mouth broadly and gave a malicious smile.\n\n\"So that was your aim. I am in awe.\"",
        );
        let next = section(
            "「……とはいえアイザス伯は、密偵など警戒もせんだろうがね」\nハーデン侯爵は会談でのアイザス伯爵の様子を思い出し、深い溜息を吐いた。",
            "Marquis Harden curled up the corners of his mouth broadly and gave a malicious smile.\n\n\"...That said, Count Aizas likely isn't wary of spies at all.\"",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn detects_reworded_retranslation() {
        let prev = section(
            "その太い手首には、黄金に輝くギルドリングが光っていた。",
            "On his thick wrist gleamed a resplendent golden guild ring. No explanation needed, it was the very proof of his rank.",
        );
        let next = section(
            "受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
            "On his thick wrist shone a brilliantly golden guild ring. No explanation needed, it was the proof of his rank.",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn ignores_lines_repeated_in_japanese() {
        let prev = section(
            "俺がそう言うと先輩はゆっくりと頷いた、そして少し考え込んだ。",
            "When I say that, Senpai slowly nods, and then falls into thought for a little while.",
        );
        let next = section(
            "俺がそう言うと先輩はゆっくりと頷いた、そして少し考え込んだ。",
            "When I say that, Senpai slowly nods, and then falls into thought for a little while.",
        );
        assert!(!boundary_overlap(&prev, &next));
    }

    #[test]
    fn detects_short_section_repeated_later() {
        let short =
            "Marquis Harden gave a malicious smile.\n\n\"So that was your aim. I am in awe.\"";
        let repeated = format!(
            "{short}\n\n\
             The man who appeared from behind the counter looked down at me appraisingly.\n\n\
             On his thick wrist gleamed a resplendent golden guild ring, proof of his rank.\n\n\
             I want to observe the relationship between Count Aizas and Erma-kun carefully.\n\n\
             Snow had begun falling over the capital while nobody inside noticed anything strange.\n\n\
             Her voice trembled slightly, yet she refused to lower her gaze from his face."
        );
        let mut page = page(vec![
            section(
                "ハーデン侯爵は意地の悪い笑みを浮かべた。\n「そういった狙いでございましたか。感服いたしました」",
                short,
            ),
            section(
                "二次試験の結果が発表され、試験を受けた七四八二人中五五八人が合格した。",
                "The results of the second exam were announced, and of the 7,482 people who took the exam, 558 passed.",
            ),
            section(
                "受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。\nアイザス伯とエルマ君の関係をしっかりと見ておきたいのだ。",
                &repeated,
            ),
        ]);
        check_all_pages(slice::from_mut(&mut page));
        let copies: Vec<_> = page
            .errors
            .iter()
            .filter(|e| matches!(e.error, ErrorType::Copy))
            .map(|e| e.index)
            .collect();
        assert_eq!(copies, [2]);
    }

    #[test]
    fn ignores_heads_repeated_in_japanese() {
        let earlier = section(
            "俺がそう言うと先輩はゆっくりと頷いた、そして少し考え込んだ。",
            "When I say that, Senpai slowly nods, and then falls into thought for a little while.",
        );
        let next = section(
            "俺がそう言うと先輩はゆっくりと頷いた、そして少し考え込んだ。\n二次試験の結果が発表された。",
            "When I say that, Senpai slowly nods, and then falls into thought for a little while.\n\nThe results of the second exam were announced.",
        );
        assert!(!head_overlap(&earlier, &next));
    }

    #[test]
    fn ignores_head_with_one_compared_line() {
        let earlier = section(
            "ハーデン侯爵は口の端を大きく吊り上げて、意地の悪い笑みを浮かべた。\n「はい」",
            "Marquis Harden gave a malicious smile.\n\n\"Yes.\"",
        );
        let next = section(
            "ハーデン侯爵は意地悪く笑った。\n二次試験の結果が発表され、試験を受けた七四八二人中五五八人が合格した。",
            "Marquis Harden gave a malicious smile.\n\nThe results of the second exam were announced, and of the 7,482 people who took the exam, 558 passed.",
        );
        assert!(!head_overlap(&earlier, &next));
    }

    #[test]
    fn ignores_single_shared_opening_line() {
        let earlier = section(
            "ハーデン侯爵は口の端を大きく吊り上げて、意地の悪い笑みを浮かべた。\n「そういった狙いでございましたか。感服いたしました」",
            "Marquis Harden gave a malicious smile.\n\n\"So that was your aim. I am in awe.\"",
        );
        let next = section(
            "ハーデン侯爵は意地悪く笑った。\n二次試験の結果が発表され、試験を受けた七四八二人中五五八人が合格した。",
            "Marquis Harden gave a malicious smile.\n\nThe results of the second exam were announced, and of the 7,482 people who took the exam, 558 passed.",
        );
        assert!(!head_overlap(&earlier, &next));
    }

    #[test]
    fn ignores_unrelated_sections() {
        let prev = section(
            "アイザス伯とエルマ君の関係をしっかりと見ておきたいのだ。",
            "I want to observe the relationship between Count Aizas and Erma-kun carefully.",
        );
        let next = section(
            "二次試験の結果が発表され、試験を受けた七四八二人中五五八人が合格した。",
            "The results of the second exam were announced, and of the 7,482 people who took the exam, 558 passed.",
        );
        assert!(!boundary_overlap(&prev, &next));
    }
}
