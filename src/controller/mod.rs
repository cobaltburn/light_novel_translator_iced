use rbook::Epub;
use std::path::PathBuf;

mod builder;
mod client;
mod html_writer;
mod parse;
mod prompts;
mod resources;
mod toc;
mod xml;

pub use builder::*;
pub use client::*;
pub use html_writer::*;
pub use parse::*;
pub use prompts::*;
pub use resources::*;
pub use toc::*;
pub use xml::*;

pub fn get_ordered_path(epub: &Epub) -> Vec<PathBuf> {
    epub.spine()
        .iter()
        .filter_map(|e| e.manifest_entry())
        .map(|e| e.href().decode().as_ref().into())
        .collect()
}

pub fn part_tag(n: usize) -> String {
    format!("\n\n<part>{}</part>\n\n", n)
}

pub const DEFAULT_STYLESHEET: &[u8] = br#"
/* Default stylesheet for e-readers.
 * Fonts, colors, margins and line spacing are left to the reader so its
 * settings (font choice, night and sepia modes) keep working. */

body {
    hyphens: auto;
    -webkit-hyphens: auto;
    overflow-wrap: break-word;
}

/* Headings */
h1, h2, h3, h4, h5, h6 {
    line-height: 1.2;
    margin: 1.5em 0 0.5em 0;
    page-break-after: avoid;
    break-after: avoid;
}

h1 {
    font-size: 1.6em;
    margin-top: 0;
    text-align: center;
    page-break-before: always;
    break-before: page;
}

h2 {
    font-size: 1.4em;
}

h3 {
    font-size: 1.2em;
}

h4, h5, h6 {
    font-size: 1em;
}

/* Paragraphs */
p {
    margin: 0 0 1em 0;
    text-indent: 1.2em;
    orphans: 2;
    widows: 2;
}

p:first-child,
h1 + p,
h2 + p,
h3 + p,
h4 + p,
h5 + p,
h6 + p,
hr + p,
.illustration + p {
    text-indent: 0;
}

/* Links keep the text color so they stay readable in night mode */
a {
    color: inherit;
    text-decoration: underline;
}

/* Lists */
ul, ol {
    margin: 1em 0;
    padding-left: 2em;
}

li {
    margin: 0.5em 0;
}

/* Blockquotes */
blockquote {
    margin: 1em 2em;
}

blockquote p {
    text-indent: 0;
}

/* Tables */
table {
    border-collapse: collapse;
    margin: 1em auto;
}

th, td {
    border: 1px solid rgba(128, 128, 128, 0.5);
    padding: 0.3em 0.5em;
}

/* Images */
img {
    max-width: 100%;
    height: auto;
}

.illustration {
    margin: 1em 0;
    text-align: center;
    text-indent: 0;
    page-break-inside: avoid;
    break-inside: avoid;
}

.illustration img {
    display: block;
    margin: 0 auto;
}

/* Scene breaks */
hr {
    border: none;
    border-top: 1px solid rgba(128, 128, 128, 0.5);
    margin: 2em 25%;
}

.section-break {
    margin: 2em 0;
    text-align: center;
    text-indent: 0;
}

/* Footnotes */
.footnote-definition {
    font-size: 0.85em;
    margin-top: 1em;
}

.footnote-definition p {
    text-indent: 0;
}

sup.footnote-reference {
    font-size: 0.75em;
    line-height: 0;
}
"#;

#[cfg(test)]
mod tests {
    use super::DEFAULT_STYLESHEET;

    #[test]
    fn stylesheet_leaves_fonts_and_colors_to_the_reader() {
        let css = str::from_utf8(DEFAULT_STYLESHEET).unwrap();
        for property in [
            "font-family",
            "background",
            "color: #",
            "padding: 1em",
            "@media",
        ] {
            assert!(!css.contains(property), "stylesheet sets {property}");
        }
    }
}
