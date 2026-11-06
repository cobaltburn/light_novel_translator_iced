//! Reading and rewriting epub xhtml.

mod images;
mod markdown;
mod resource_links;
mod strip;
mod tags;
mod text;

pub use images::image_anchors;
pub use markdown::{
    html_to_markdown, html_to_marked_markdown, parse_links, to_html, toc_to_markdown,
};
pub use resource_links::{update_image_paths, update_resource_paths, update_resource_tag};
pub use strip::{strip_syosetu_tags, strip_tags};
pub use tags::{ANCHOR_TAG, BR, HEAD_TAG, PARAGRAPH_TAG, is_header};
pub use text::{TextRun, text_content};
