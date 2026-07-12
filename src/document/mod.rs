//! Assembles the Typst source and the in-memory files it reads.

pub mod compile;

use crate::theme::{self, Theme, icons, tmtheme};

/// The style rules, written in Typst against the bindings [`preamble`] emits.
const STYLESHEET: &str = include_str!("../../assets/theme.typ");

pub const SYNTAX_THEME_PATH: &str = "/syntax.tmTheme";

/// Typst bindings for everything that varies between the light and dark themes.
fn preamble(theme: &Theme) -> String {
    let palette = [
        ("background", theme.background),
        ("foreground", theme.foreground),
        ("heading", theme.heading),
        ("heading-h3", theme.heading_h3),
        ("heading-h4", theme.heading_h4),
        ("heading-h5", theme.heading_h5),
        ("card", theme.card),
        ("card-foreground", theme.card_foreground),
        ("secondary", theme.secondary),
        ("muted", theme.muted),
        ("muted-foreground", theme.muted_foreground),
        ("accent", theme.accent),
        ("code-lang-background", theme.code_lang_background),
        ("code-lang-foreground", theme.code_lang_foreground),
        ("border", theme.border),
        ("border-style", theme.border_style),
        ("rule", theme.rule),
        ("code", theme.code),
        ("highlight", theme.highlight),
        ("primary", theme::PRIMARY),
        ("primary-foreground", theme::PRIMARY_FOREGROUND),
        ("footnote-accent", theme::FOOTNOTE_ACCENT),
    ];

    let entries: String = palette
        .iter()
        .map(|(name, color)| format!("  {name}: {},\n", color.to_typst()))
        .collect();

    let markers: String = theme::MARKER_COLORS
        .iter()
        .map(|color| format!("{}, ", color.to_typst()))
        .collect();

    let rules: String = theme::HEADING_RULES
        .iter()
        .map(|color| format!("{}, ", color.to_typst()))
        .collect();

    let callouts: String = theme::CALLOUT_ALIASES
        .iter()
        .map(|(kind, icon)| {
            let color = theme::callout_color(icon);

            format!(
                "  \"{kind}\": (color: {}, background: {}, icon: \"{}\"),\n",
                color.to_typst(),
                color.with_opacity(0.1).to_typst(),
                icon_path(icon),
            )
        })
        .collect();

    format!(
        concat!(
            "#let palette = (\n{entries})\n\n",
            "#let marker-colors = ({markers})\n",
            "#let heading-rules = ({rules})\n\n",
            "#let callouts = (\n{callouts})\n\n",
            "#let syntax-theme = \"{syntax}\"\n\n",
        ),
        entries = entries,
        markers = markers,
        rules = rules,
        callouts = callouts,
        syntax = SYNTAX_THEME_PATH,
    )
}

fn icon_path(icon: &str) -> String {
    format!("/icons/callout-{icon}.svg")
}

/// The complete Typst source: bindings, style rules, then the document body.
pub fn source(
    theme: &Theme,
    body: &str,
) -> String {
    format!("{}{STYLESHEET}\n\n{body}", preamble(theme))
}

/// Binary files the Typst source reads by virtual path.
pub fn assets(theme: &Theme) -> Vec<(String, Vec<u8>)> {
    let mut files = vec![(
        SYNTAX_THEME_PATH.to_owned(),
        tmtheme::build(theme).into_bytes(),
    )];

    for icon in theme::callout_icons() {
        let color = theme::callout_color(icon);
        files.push((
            icon_path(icon),
            icons::callout(icon).to_svg(color).into_bytes(),
        ));
    }

    files.push((
        "/icons/check-on-primary.svg".to_owned(),
        icons::CHECK.to_svg(theme::PRIMARY_FOREGROUND).into_bytes(),
    ));
    files.push((
        "/icons/cross-muted.svg".to_owned(),
        icons::CROSS.to_svg(theme.muted_foreground).into_bytes(),
    ));
    files.push((
        "/icons/calendar.svg".to_owned(),
        icons::CALENDAR.to_svg(theme.muted_foreground).into_bytes(),
    ));
    files.push((
        "/icons/notes.svg".to_owned(),
        icons::notes_mark(theme::PRIMARY).into_bytes(),
    ));
    files.push((
        "/icons/missing.svg".to_owned(),
        icons::callout("warning")
            .to_svg(theme.muted_foreground)
            .into_bytes(),
    ));

    files
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Exercises every helper `assets/theme.typ` defines. A Typst syntax
    /// error or a renamed binding fails here rather than at the user's shell.
    const EVERY_ELEMENT: &str = r#"---
title: Fixture
tags:
  - one
  - two
created: 2024-04-05T15:30:00
home: https://example.com
flag: true
blank:
---

# Title

## Second

### Third
#### Fourth
##### Fifth
###### Sixth

Plain text with *emph*, **strong**, ~~struck~~, ==highlight==, %%comment%%,
a #tag/nested, `inline code`, a [link](https://example.com) and a footnote[^a].

- bullet
  - nested
    - deeper
      - deepest

1. first
   1. inner
      1. innermost

- [ ] open
- [x] done

> plain quote

> [!warning] Careful
> body text

> [!quote] Title only

```python
def greet():
    print("hi")
```

```
no language
```

| A | B |
| - | :-: |
| 1 | 2 |

---

Inline $E = mc^2$ and a block:

$$
\int_0^1 x^2 dx
$$

[^a]: the note
"#;

    #[test]
    fn the_stylesheet_compiles_every_element_it_styles() {
        let parsed = crate::markdown::frontmatter::split(EVERY_ELEMENT);
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);

        let rendered = crate::markdown::render(&parsed.body, Path::new("."), &parsed.properties);

        assert!(rendered.warnings.is_empty(), "{:?}", rendered.warnings);

        for theme in [Theme::light(), Theme::dark()] {
            let mut files = assets(&theme);
            files.extend(rendered.files.clone());

            let source = source(&theme, &rendered.body);

            let pdf = compile::to_pdf(&source, files);

            assert!(pdf.is_ok(), "{:?}", pdf.err());
        }
    }

    #[test]
    fn every_callout_alias_points_at_an_emitted_icon() {
        let theme = Theme::light();
        let paths: Vec<String> = assets(&theme).into_iter().map(|(path, _)| path).collect();
        let preamble = preamble(&theme);

        for (kind, icon) in theme::CALLOUT_ALIASES {
            let path = icon_path(icon);

            assert!(
                paths.contains(&path),
                "callout {kind} references a missing {path}"
            );
            assert!(preamble.contains(&format!("\"{kind}\":")));
        }
    }

    #[test]
    fn assets_carry_no_unsubstituted_placeholders() {
        for theme in [Theme::light(), Theme::dark()] {
            for (path, bytes) in assets(&theme) {
                let content = String::from_utf8(bytes).unwrap();
                assert!(!content.contains("{color}"), "{path} kept a placeholder");
            }
        }
    }

    #[test]
    fn the_syntax_theme_is_registered_where_the_stylesheet_looks_for_it() {
        let theme = Theme::light();
        let paths: Vec<String> = assets(&theme).into_iter().map(|(path, _)| path).collect();

        assert!(paths.contains(&SYNTAX_THEME_PATH.to_owned()));
        assert!(preamble(&theme).contains(SYNTAX_THEME_PATH));
    }
}
