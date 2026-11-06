use quick_xml::{events::BytesStart, name::QName};

pub const HEADER_TAGS: &[&str] = &["h1", "h2", "h3", "h4"];
pub const RUBY_TAG: &str = "ruby";
pub const HEAD_TAG: &str = "head";
pub const IMG_TAG: &str = "img";
pub const IMAGE_TAG: &str = "image";
pub const LINK_TAG: &str = "link";
pub const SCRIPT_TAG: &str = "script";
pub const ANCHOR_TAG: &str = "a";
pub const PARAGRAPH_TAG: &str = "p";
pub const BR: &str = "br";

pub const SRC: &str = "src";
pub const XLINK: &str = "xlink:href";

pub fn is_header(name: QName<'_>) -> bool {
    HEADER_TAGS.contains(&name.as_ref())
}

pub fn is_ruby(name: QName<'_>) -> bool {
    name.as_ref() == RUBY_TAG
}

pub fn is_image(tag: &BytesStart<'_>) -> bool {
    let name = tag.name();
    name.as_ref() == IMG_TAG || name.as_ref() == IMAGE_TAG
}
