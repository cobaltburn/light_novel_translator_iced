use crate::text::markdown_sections;
use regex::Regex;
use std::{collections::HashSet, sync::LazyLock};

pub fn image_marker(n: usize) -> String {
    format!("[[IMG:{n}]]")
}

pub static IMAGE_MARKER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\?\[\\?\[\s*IMG\s*:\s*(\d+)\s*\\?\]\\?\]").unwrap());

pub fn has_translatable_text(text: &str) -> bool {
    !markdown_sections(&IMAGE_MARKER_RE.replace_all(text, "")).is_empty()
}

pub fn image_marker_indices(content: &str) -> HashSet<usize> {
    IMAGE_MARKER_RE
        .captures_iter(content)
        .filter_map(|caps| caps[1].parse().ok())
        .collect()
}
