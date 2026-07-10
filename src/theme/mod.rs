//! The document palette, ported from the stylesheet's CSS custom properties.

pub mod icons;
pub mod tmtheme;

mod color;

pub use color::Color;

/// Accent hues, shared by heading rules and list markers.
const LEVEL_1: Color = Color::hex(0xe95118);
const LEVEL_2: Color = Color::hex(0x8b5cf6);
const LEVEL_3: Color = Color::hex(0xf59e0b);
const LEVEL_4: Color = Color::hex(0x10b981);

pub const PRIMARY: Color = LEVEL_1;
pub const PRIMARY_FOREGROUND: Color = Color::hex(0xffffff);
pub const FOOTNOTE_ACCENT: Color = Color::hex(0x3b82f6);

/// Rule colors for headings `h3` through `h6`.
pub const HEADING_RULES: [Color; 4] = [LEVEL_1, LEVEL_2, LEVEL_3, LEVEL_4];

/// List marker colors by nesting depth. The stylesheet walks the accents backwards.
pub const MARKER_COLORS: [Color; 4] = [LEVEL_4, LEVEL_3, LEVEL_2, LEVEL_1];

/// Colors for the `hljs-*` classes the stylesheet targets.
pub struct SyntaxColors {
    pub keyword: Color,
    pub string: Color,
    pub comment: Color,
    pub number: Color,
    pub built_in: Color,
    pub function: Color,
    pub title: Color,
    pub class: Color,
    pub variable: Color,
}

pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub heading: Color,
    pub heading_h3: Color,
    pub heading_h4: Color,
    pub heading_h5: Color,
    pub card: Color,
    pub card_foreground: Color,
    pub secondary: Color,
    pub muted: Color,
    pub muted_foreground: Color,
    pub accent: Color,
    pub code_lang_background: Color,
    pub code_lang_foreground: Color,
    pub border: Color,
    /// The translucent hairline used for code, table and callout edges.
    pub border_style: Color,
    pub rule: Color,
    pub code: Color,
    pub highlight: Color,
    pub syntax: SyntaxColors,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            background: Color::hex(0xffffff),
            foreground: Color::hex(0x404040),
            heading: Color::hex(0x0f0f0f),
            heading_h3: Color::hex(0x222222),
            heading_h4: Color::hex(0x333333),
            heading_h5: Color::hex(0x444444),
            card: Color::hex(0xffffff),
            card_foreground: Color::hex(0x333333),
            secondary: Color::hex(0xf2f2f2),
            muted: Color::hex(0xe5e5e5),
            muted_foreground: Color::hex(0x6c6c6c),
            accent: Color::hex(0xf5f5f5),
            code_lang_background: Color::hex(0xeeeeee),
            code_lang_foreground: Color::hex(0x505050),
            border: Color::hex(0xe2e2e2),
            border_style: Color::hex(0x000000).with_opacity(0.1),
            rule: Color::hex(0xd4d4d8),
            code: Color::hex(0x333333),
            highlight: Color::hex(0xf97316).with_opacity(0.35),
            syntax: SyntaxColors {
                keyword: Color::hex(0x0550ae),
                string: Color::hex(0x15803d),
                comment: Color::hex(0x6b7280),
                number: Color::hex(0xb91c1c),
                built_in: Color::hex(0x6d28d9),
                function: Color::hex(0x7c3aed),
                title: Color::hex(0x7c3aed),
                class: Color::hex(0xca8a04),
                variable: Color::hex(0x6d28d9),
            },
        }
    }

    pub fn dark() -> Self {
        Self {
            background: Color::hex(0x0f0f0f),
            foreground: Color::hex(0xb4b4b4),
            heading: Color::hex(0xffffff),
            heading_h3: Color::hex(0xf0f0f0),
            heading_h4: Color::hex(0xe6e6e6),
            heading_h5: Color::hex(0xd9d9d9),
            card: Color::hex(0x0f0f0f),
            card_foreground: Color::hex(0xe0e0e0),
            secondary: Color::hex(0x1e1e20),
            muted: Color::hex(0x2a2a2d),
            muted_foreground: Color::hex(0xa3a3a3),
            accent: Color::hex(0x18181b),
            code_lang_background: Color::hex(0x27272a),
            code_lang_foreground: Color::hex(0xe2e8f0),
            border: Color::hex(0x27272a),
            border_style: Color::hex(0xffffff).with_opacity(0.1),
            rule: Color::hex(0x3f3f46),
            code: Color::hex(0xe0e0e0),
            highlight: Color::hex(0xf97316).with_opacity(0.40),
            syntax: SyntaxColors {
                keyword: Color::hex(0x93c5fd),
                string: Color::hsl(142.0, 0.76, 0.70),
                comment: Color::hsl(215.0, 0.16, 0.65),
                number: Color::hsl(6.0, 1.00, 0.77),
                built_in: Color::hex(0xa5b4fc),
                function: Color::hex(0xc4b5fd),
                title: Color::hex(0xc4b5fd),
                class: Color::hsl(35.0, 1.00, 0.70),
                variable: Color::hex(0xa5b4fc),
            },
        }
    }
}

/// Every Obsidian callout alias, mapped to the icon that represents it.
/// The icon in turn fixes the callout's color, so aliases sharing an icon
/// share a color, exactly as the stylesheet's grouped selectors did.
pub const CALLOUT_ALIASES: &[(&str, &str)] = &[
    ("note", "note"),
    ("info", "info"),
    ("todo", "todo"),
    ("tip", "tip"),
    ("important", "tip"),
    ("hint", "tip"),
    ("abstract", "abstract"),
    ("summary", "abstract"),
    ("tldr", "abstract"),
    ("success", "success"),
    ("check", "success"),
    ("done", "success"),
    ("warning", "warning"),
    ("caution", "warning"),
    ("attention", "warning"),
    ("question", "question"),
    ("faq", "question"),
    ("help", "question"),
    ("danger", "danger"),
    ("error", "danger"),
    ("bug", "bug"),
    ("fail", "fail"),
    ("failure", "fail"),
    ("missing", "fail"),
    ("example", "example"),
    ("quote", "quote"),
    ("cite", "quote"),
];

/// The accent each callout icon is tinted with. Its 10% wash becomes the background.
pub fn callout_color(icon: &str) -> Color {
    match icon {
        "note" | "info" | "todo" => Color::hex(0x086ddd),
        "tip" | "abstract" => Color::hex(0x08c1be),
        "success" => Color::hex(0x2dc368),
        "warning" | "question" => Color::hex(0xff9100),
        "danger" | "bug" | "fail" => Color::hex(0xff3232),
        "example" => Color::hex(0x8b5cf6),
        "quote" => Color::hex(0x64748b),
        _ => unreachable!("callout icon {icon} has no color"),
    }
}

/// The distinct icons referenced by [`CALLOUT_ALIASES`], deduplicated.
pub fn callout_icons() -> Vec<&'static str> {
    let mut icons: Vec<&str> = CALLOUT_ALIASES.iter().map(|(_, icon)| *icon).collect();
    icons.dedup();
    icons.sort_unstable();
    icons.dedup();
    icons
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_alias_resolves_to_a_colored_icon() {
        for (kind, icon) in CALLOUT_ALIASES {
            let color = callout_color(icon);
            assert_ne!(color.alpha, 0, "callout {kind} resolved to a blank color");
        }
    }

    #[test]
    fn aliases_cover_the_stylesheets_callout_groups() {
        // Guards against an alias silently going missing from the table.
        assert_eq!(CALLOUT_ALIASES.len(), 27);
        assert_eq!(callout_icons().len(), 13);
    }

    #[test]
    fn grouped_aliases_share_one_color() {
        let lookup = |kind: &str| {
            let (_, icon) = CALLOUT_ALIASES.iter().find(|(k, _)| *k == kind).unwrap();
            callout_color(icon)
        };

        assert_eq!(lookup("tip"), lookup("important"));
        assert_eq!(lookup("danger"), lookup("bug"));
        assert_eq!(lookup("quote"), lookup("cite"));
        assert_ne!(lookup("note"), lookup("success"));
    }
}
