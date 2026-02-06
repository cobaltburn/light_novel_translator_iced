use epub::doc::EpubDoc;
use std::{io::Cursor, path::PathBuf};

pub mod builder;
pub mod client;
pub mod parse;
pub mod xml;

pub fn get_ordered_path(epub: &EpubDoc<Cursor<Vec<u8>>>) -> Vec<PathBuf> {
    epub.spine
        .iter()
        .map(|e| epub.resources.get(&e.idref).unwrap().path.clone())
        .collect()
}

pub fn part_tag(n: usize) -> String {
    format!("\n\n<part>{}</part>\n\n", n)
}

pub fn check_english_chars(text: &str, min: f32) -> bool {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let ascsii_count = text.chars().filter(|c| c.is_ascii_alphanumeric()).count() as f32;
    let total = text.chars().count() as f32;
    let percent = (ascsii_count / total) * 100.0;
    percent > min
}

pub const HEADER: &str = r#"
<head>
    <meta charset="UTF-8" />
    <link rel="stylesheet" type="text/css" href="../stylesheet.css" />
</head>
"#;

pub const DEFAULT_STYLESHEET: &[u8] = br#"
        /* EPUB Default Stylesheet */

    /* Reset and base styles */
    * {
        box-sizing: border-box;
    }

    html {
        font-size: 100%;
        line-height: 1.6;
    }

    body {
        font-family: "Times New Roman", Times, serif;
        font-size: 1em;
        line-height: 1.6;
        margin: 0;
        padding: 1em;
        color: #333;
        background-color: #fff;
        text-align: left;
        word-wrap: break-word;
        -webkit-hyphens: auto;
        -moz-hyphens: auto;
        -ms-hyphens: auto;
        hyphens: auto;
    }

    /* Headings */
    h1, h2, h3, h4, h5, h6 {
        font-weight: bold;
        line-height: 1.2;
        margin: 1.5em 0 0.5em 0;
        page-break-after: avoid;
        break-after: avoid;
        orphans: 3;
        widows: 3;
    }

    h1 {
        font-size: 2em;
        margin-top: 0;
        text-align: center;
        page-break-before: always;
        break-before: page;
    }

    h2 {
        font-size: 1.5em;
        page-break-before: auto;
        break-before: auto;
    }

    h3 {
        font-size: 1.3em;
    }

    h4 {
        font-size: 1.1em;
    }

    h5, h6 {
        font-size: 1em;
    }

    /* Paragraphs */
    p {
        margin: 0 0 1em 0;
        text-indent: 1.2em;
        orphans: 2;
        widows: 2;
    }

    p.no-indent,
    p:first-child,
    h1 + p,
    h2 + p,
    h3 + p,
    h4 + p,
    h5 + p,
    h6 + p {
        text-indent: 0;
    }

    /* Lists */
    ul, ol {
        margin: 1em 0;
        padding-left: 2em;
    }

    li {
        margin: 0.5em 0;
    }

    /* Text formatting */
    em, i {
        font-style: italic;
    }

    strong, b {
        font-weight: bold;
    }

    small {
        font-size: 0.875em;
    }

    sup, sub {
        font-size: 0.75em;
        line-height: 0;
        position: relative;
        vertical-align: baseline;
    }

    sup {
        top: -0.5em;
    }

    sub {
        bottom: -0.25em;
    }

    /* Links */
    a {
        color: #0066cc;
        text-decoration: underline;
    }

    a:visited {
        color: #800080;
    }

    /* Blockquotes */
    blockquote {
        margin: 1.5em 2em;
        padding: 0 1em;
        border-left: 3px solid #ccc;
        font-style: italic;
    }

    blockquote p {
        text-indent: 0;
    }

    /* Code */
    code {
        font-family: "Courier New", Courier, monospace;
        font-size: 0.9em;
        background-color: #f5f5f5;
        padding: 0.1em 0.3em;
        border-radius: 3px;
    }

    pre {
        font-family: "Courier New", Courier, monospace;
        font-size: 0.85em;
        background-color: #f5f5f5;
        padding: 1em;
        border-radius: 5px;
        overflow-x: auto;
        white-space: pre-wrap;
        word-wrap: break-word;
    }

    pre code {
        background-color: transparent;
        padding: 0;
    }

    /* Tables */
    table {
        border-collapse: collapse;
        width: 100%;
        margin: 1em 0;
    }

    th, td {
        border: 1px solid #ddd;
        padding: 0.5em;
        text-align: left;
    }

    th {
        background-color: #f5f5f5;
        font-weight: bold;
    }

    /* Images */
    img {
        max-width: 100%;
        height: auto;
        display: block;
        margin: 1em auto;
    }

    figure {
        margin: 1.5em 0;
        text-align: center;
    }

    figcaption {
        font-size: 0.9em;
        font-style: italic;
        margin-top: 0.5em;
        text-align: center;
    }

    /* Horizontal rules */
    hr {
        border: none;
        border-top: 1px solid #ccc;
        margin: 2em 0;
        height: 0;
    }

    /* Special elements */
    .center, .text-center {
        text-align: center;
        text-indent: 0;
    }

    .right, .text-right {
        text-align: right;
        text-indent: 0;
    }

    .justify, .text-justify {
        text-align: justify;
    }

    .no-break {
        page-break-inside: avoid;
        break-inside: avoid;
    }

    .page-break {
        page-break-before: always;
        break-before: page;
    }

    /* Chapter and section breaks */
    .chapter {
        page-break-before: always;
        break-before: page;
    }

    .section-break {
        margin: 3em 0;
        text-align: center;
    }

    .section-break:before {
        content: "* * *";
        font-size: 1.2em;
        letter-spacing: 0.5em;
    }

    /* Drop caps */
    .drop-cap:first-letter {
        float: left;
        font-size: 3.5em;
        line-height: 0.8;
        margin: 0.1em 0.1em 0 0;
        font-weight: bold;
    }

    /* Title page */
    .title-page {
        text-align: center;
        page-break-after: always;
        break-after: page;
    }

    .title {
        font-size: 2.5em;
        font-weight: bold;
        margin: 2em 0 1em 0;
    }

    .subtitle {
        font-size: 1.5em;
        margin: 0 0 2em 0;
    }

    .author {
        font-size: 1.2em;
        margin: 1em 0;
    }

    /* Table of contents */
    .toc {
        page-break-before: always;
        break-before: page;
    }

    .toc ul {
        list-style: none;
        padding-left: 0;
    }

    .toc li {
        margin: 0.5em 0;
        text-indent: 0;
    }

    .toc a {
        text-decoration: none;
        border-bottom: 1px dotted;
    }

    /* Footnotes */
    .footnote {
        font-size: 0.85em;
        margin-top: 2em;
        padding-top: 1em;
        border-top: 1px solid #ccc;
    }

    .footnote-ref {
        font-size: 0.75em;
        vertical-align: super;
        text-decoration: none;
    }

    /* Media queries for different screen sizes */
    @media screen and (max-width: 600px) {
        body {
            padding: 0.5em;
            font-size: 0.9em;
        }
    
        h1 {
            font-size: 1.8em;
        }
    
        h2 {
            font-size: 1.4em;
        }
    
        blockquote {
            margin: 1em 1em;
        }
    }

    /* Print styles */
    @media print {
        body {
            font-size: 12pt;
            line-height: 1.4;
        }
    
        h1, h2, h3, h4, h5, h6 {
            page-break-after: avoid;
        }
    
        img {
            page-break-inside: avoid;
        }
    
        blockquote, table, pre {
            page-break-inside: avoid;
        }
    }

    /* Accessibility improvements */
    @media (prefers-reduced-motion: reduce) {
        * {
            animation-duration: 0.01ms !important;
            animation-iteration-count: 1 !important;
            transition-duration: 0.01ms !important;
        }
    }
"#;
