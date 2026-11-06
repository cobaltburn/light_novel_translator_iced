//! Plain text helpers. Nothing here knows about pages, epubs or the ui.

mod cleanup;
mod image_marker;
mod japanese;
mod parts;

pub use cleanup::{clean_invisible_chars, remove_think_tags};
pub use image_marker::{
    IMAGE_MARKER_RE, has_translatable_text, image_marker, image_marker_indices,
};
pub use japanese::{contains_japanese, is_japanese_char, replace_jp_symbols};
pub use parts::{
    content_sections, join_partition, markdown_sections, part_tag, partition_text, split_parts,
};
