//! The stylesheet's inline SVG icons.
//!
//! Typst renders SVG through resvg, which has no notion of `currentColor`, so
//! each icon is emitted with its color already substituted.

use super::Color;

pub struct Icon {
    pub stroke_width: &'static str,
    /// Shapes on a 24x24 canvas. `{color}` marks a solid fill.
    pub body: &'static str,
}

impl Icon {
    pub fn to_svg(
        &self,
        color: Color,
    ) -> String {
        let hex = color.to_hex();

        format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" "#,
                r#"fill="none" stroke="{stroke}" stroke-width="{width}" "#,
                r#"stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#,
            ),
            stroke = hex,
            width = self.stroke_width,
            body = self.body.replace("{color}", &hex,),
        )
    }
}

/// The mark on the footnotes divider, a six spoke asterisk.
///
/// It carries its own canvas, cropped to the ink, rather than the 24 by 24 one
/// the callout icons share. A shared canvas would surround it with side
/// bearings, and the word it sits against would be pushed away by them.
pub fn notes_mark(color: Color) -> String {
    format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="5.938 5 12.124 14" "#,
            r#"fill="none" stroke="{stroke}" stroke-width="2" stroke-linecap="round">"#,
            r#"<path d="M12 6v12"/>"#,
            r#"<path d="M17.196 9 6.804 15"/>"#,
            r#"<path d="m6.804 9 10.392 6"/>"#,
            r#"</svg>"#,
        ),
        stroke = color.to_hex(),
    )
}

/// The icon for a callout, keyed by the names in [`crate::theme::CALLOUT_ALIASES`].
pub fn callout(icon: &str) -> &'static Icon {
    match icon {
        "note" => &NOTE,
        "todo" => &TODO,
        "tip" => &TIP,
        "abstract" => &ABSTRACT,
        "success" => &SUCCESS,
        "warning" => &WARNING,
        "question" => &QUESTION,
        "danger" => &DANGER,
        "bug" => &BUG,
        "fail" => &FAIL,
        "example" => &EXAMPLE,
        "quote" => &QUOTE,
        // The stylesheet falls back to the info circle for anything unrecognised.
        _ => &INFO,
    }
}

pub const CHECK: Icon = Icon {
    stroke_width: "3",
    body: r#"<polyline points="20 6 9 17 4 12"/>"#,
};

pub const CROSS: Icon = Icon {
    stroke_width: "3",
    body: r#"<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>"#,
};

pub const CALENDAR: Icon = Icon {
    stroke_width: "2",
    body: concat!(
        r#"<rect width="18" height="18" x="3" y="4" rx="2" ry="2"/>"#,
        r#"<line x1="16" x2="16" y1="2" y2="6"/>"#,
        r#"<line x1="8" x2="8" y1="2" y2="6"/>"#,
        r#"<line x1="3" x2="21" y1="10" y2="10"/>"#,
    ),
};

const NOTE: Icon = Icon {
    stroke_width: "2.2",
    body: concat!(
        r#"<path d="M13 21h8"/>"#,
        r#"<path d="m15 5 4 4"/>"#,
        r#"<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/>"#,
    ),
};

const INFO: Icon = Icon {
    stroke_width: "2.2",
    body: concat!(
        r#"<circle cx="12" cy="12" r="10"/>"#,
        r#"<path d="M12 16v-4"/>"#,
        r#"<circle cx="12" cy="8" r="1.2" fill="{color}" stroke="none"/>"#,
    ),
};

const TODO: Icon = Icon {
    stroke_width: "2.2",
    body: concat!(
        r#"<circle cx="12" cy="12" r="10"/>"#,
        r#"<path d="m9 12 2 2 4-4"/>"#,
    ),
};

const TIP: Icon = Icon {
    stroke_width: "2.2",
    body: r#"<path d="M12 3q1 4 4 6.5t3 5.5a1 1 0 0 1-14 0 5 5 0 0 1 1-3 1 1 0 0 0 5 0c0-2-1.5-3-1.5-5q0-2 2.5-4"/>"#,
};

const DANGER: Icon = Icon {
    stroke_width: "2.4",
    body: r#"<path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>"#,
};

const BUG: Icon = Icon {
    stroke_width: "2.4",
    body: concat!(
        r#"<path d="M12 20v-9"/>"#,
        r#"<path d="M14 7a4 4 0 0 1 4 4v3a6 6 0 0 1-12 0v-3a4 4 0 0 1 4-4z"/>"#,
        r#"<path d="M14.12 3.88 16 2"/>"#,
        r#"<path d="M21 21a4 4 0 0 0-3.81-4"/>"#,
        r#"<path d="M21 5a4 4 0 0 1-3.55 3.97"/>"#,
        r#"<path d="M22 13h-4"/>"#,
        r#"<path d="M3 21a4 4 0 0 1 3.81-4"/>"#,
        r#"<path d="M3 5a4 4 0 0 0 3.55 3.97"/>"#,
        r#"<path d="M6 13H2"/>"#,
        r#"<path d="m8 2 1.88 1.88"/>"#,
        r#"<path d="M9 7.13V6a3 3 0 1 1 6 0v1.13"/>"#,
    ),
};

const FAIL: Icon = Icon {
    stroke_width: "2.6",
    body: concat!(r#"<path d="M18 6 6 18"/>"#, r#"<path d="m6 6 12 12"/>"#),
};

const QUESTION: Icon = Icon {
    stroke_width: "2.2",
    body: concat!(
        r#"<circle cx="12" cy="12" r="10"/>"#,
        r#"<path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/>"#,
        r#"<circle cx="12" cy="17" r="1.2" fill="{color}" stroke="none"/>"#,
    ),
};

const WARNING: Icon = Icon {
    stroke_width: "2.4",
    body: concat!(
        r#"<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/>"#,
        r#"<path d="M12 9v4"/>"#,
        r#"<circle cx="12" cy="17" r="1.2" fill="{color}" stroke="none"/>"#,
    ),
};

const SUCCESS: Icon = Icon {
    stroke_width: "2.6",
    body: r#"<path d="M20 6 9 17l-5-5"/>"#,
};

const QUOTE: Icon = Icon {
    stroke_width: "2.2",
    body: concat!(
        r#"<path d="M16 3a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2 1 1 0 0 1 1 1v1a2 2 0 0 1-2 2 1 1 0 0 0-1 1v2a1 1 0 0 0 1 1 6 6 0 0 0 6-6V5a2 2 0 0 0-2-2z"/>"#,
        r#"<path d="M5 3a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2 1 1 0 0 1 1 1v1a2 2 0 0 1-2 2 1 1 0 0 0-1 1v2a1 1 0 0 0 1 1 6 6 0 0 0 6-6V5a2 2 0 0 0-2-2z"/>"#,
    ),
};

const EXAMPLE: Icon = Icon {
    stroke_width: "2.2",
    body: concat!(
        r#"<circle cx="4" cy="6" r="1.2" fill="{color}" stroke="none"/>"#,
        r#"<circle cx="4" cy="12" r="1.2" fill="{color}" stroke="none"/>"#,
        r#"<circle cx="4" cy="18" r="1.2" fill="{color}" stroke="none"/>"#,
        r#"<line x1="8" y1="6" x2="21" y2="6"/>"#,
        r#"<line x1="8" y1="12" x2="21" y2="12"/>"#,
        r#"<line x1="8" y1="18" x2="21" y2="18"/>"#,
    ),
};

const ABSTRACT: Icon = Icon {
    stroke_width: "2.2",
    body: concat!(
        r#"<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/>"#,
        r#"<path d="M14 2v5a1 1 0 0 0 1 1h5"/>"#,
        r#"<path d="M10 9H8"/>"#,
        r#"<path d="M16 13H8"/>"#,
        r#"<path d="M16 17H8"/>"#,
    ),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_replaces_both_stroke_and_solid_fills() {
        let svg = INFO.to_svg(Color::hex(0x086ddd));

        assert!(svg.contains(r##"stroke="#086ddd""##));
        assert!(svg.contains(r##"fill="#086ddd""##));
        assert!(
            !svg.contains("{color}"),
            "an unsubstituted placeholder remains"
        );
        assert!(
            !svg.contains("currentColor"),
            "resvg cannot resolve currentColor"
        );
    }

    /// A 24 by 24 canvas would frame the mark in side bearings, and the word
    /// it sits against would be pushed away by them.
    #[test]
    fn the_notes_mark_is_coloured_and_cropped_to_its_own_ink() {
        let svg = notes_mark(Color::hex(0xe11d48));

        assert!(svg.contains(r##"stroke="#e11d48""##));
        assert!(
            !svg.contains("0 0 24 24"),
            "the mark took the shared canvas"
        );
        assert!(
            !svg.contains("{color}"),
            "an unsubstituted placeholder remains"
        );
    }

    #[test]
    fn unknown_callout_names_fall_back_to_the_info_circle() {
        assert_eq!(callout("no-such-callout").body, INFO.body);
        assert_eq!(callout("info").body, INFO.body);
        assert_ne!(callout("bug").body, INFO.body);
    }
}
