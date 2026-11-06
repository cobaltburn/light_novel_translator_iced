pub const DEFAULT_STYLESHEET: &[u8] = include_bytes!("default_stylesheet.css");

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
