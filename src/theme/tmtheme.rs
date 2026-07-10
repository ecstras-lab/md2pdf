//! Renders the theme's `hljs-*` colors as a TextMate color scheme.
//!
//! Typst highlights code with syntect, which takes a `.tmTheme` plist rather
//! than CSS classes. Each rule below is the TextMate scope that the Sublime
//! grammars emit where highlight.js would have emitted the matching class.

use super::Theme;

enum Style {
    Plain,
    Bold,
    Italic,
}

pub fn build(theme: &Theme) -> String {
    let syntax = &theme.syntax;

    let rules = [
        ("comment", syntax.comment, Style::Italic),
        ("string", syntax.string, Style::Plain),
        ("constant.numeric", syntax.number, Style::Plain),
        ("keyword", syntax.keyword, Style::Bold),
        // Sublime's grammars scope Python's `def` and `class` as storage.type,
        // where highlight.js would have called them keywords.
        (
            "storage.type, storage.modifier",
            syntax.keyword,
            Style::Bold,
        ),
        // Deliberately not `meta.function-call`: it spans the argument
        // parentheses too, which highlight.js left unstyled.
        ("entity.name.function", syntax.function, Style::Plain),
        (
            "support.function, support.type, support.class",
            syntax.built_in,
            Style::Plain,
        ),
        (
            "entity.name.class, entity.name.type",
            syntax.class,
            Style::Plain,
        ),
        ("entity.name", syntax.title, Style::Plain),
        ("variable", syntax.variable, Style::Plain),
    ];

    let mut xml = String::new();
    xml.push_str(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" ",
        "\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
        "<plist version=\"1.0\">\n<dict>\n",
        "<key>name</key><string>md2pdf</string>\n",
        "<key>settings</key>\n<array>\n",
    ));

    // The leading entry carries no scope and sets the defaults.
    xml.push_str(&format!(
        concat!(
            "<dict><key>settings</key><dict>",
            "<key>background</key><string>{background}</string>",
            "<key>foreground</key><string>{foreground}</string>",
            "</dict></dict>\n",
        ),
        background = theme.accent.to_hex(),
        foreground = theme.code.to_hex(),
    ));

    for (scope, color, style) in rules {
        xml.push_str(&format!(
            "<dict><key>scope</key><string>{scope}</string><key>settings</key><dict>",
        ));

        match style {
            Style::Bold => xml.push_str("<key>fontStyle</key><string>bold</string>"),
            Style::Italic => xml.push_str("<key>fontStyle</key><string>italic</string>"),
            Style::Plain => {}
        }

        xml.push_str(&format!(
            "<key>foreground</key><string>{}</string></dict></dict>\n",
            color.to_hex(),
        ));
    }

    xml.push_str("</array>\n</dict>\n</plist>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use two_face::re_exports::syntect::highlighting::ThemeSet;

    /// The real contract: syntect must accept what we emit, because Typst
    /// hands this file straight to syntect at compile time.
    #[test]
    fn syntect_parses_the_generated_theme() {
        for theme in [Theme::light(), Theme::dark()] {
            let xml = build(&theme);
            let mut cursor = std::io::Cursor::new(xml.as_bytes());

            let parsed = ThemeSet::load_from_reader(&mut cursor)
                .expect("syntect rejected the generated tmTheme");

            assert_eq!(parsed.scopes.len(), 10);
        }
    }

    #[test]
    fn keyword_scope_carries_the_themes_keyword_color() {
        let theme = Theme::light();
        let xml = build(&theme);

        assert!(xml.contains("<string>keyword</string>"));
        assert!(xml.contains(&theme.syntax.keyword.to_hex()));
        assert!(xml.contains("<string>bold</string>"));
        assert!(xml.contains("<string>italic</string>"));
    }

    #[test]
    fn light_and_dark_themes_differ() {
        assert_ne!(build(&Theme::light()), build(&Theme::dark()));
    }
}
