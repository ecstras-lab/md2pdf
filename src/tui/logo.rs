//! The wordmark, drawn in quadrant blocks from the project's SVG logo.
//!
//! The logo is a 50 by 5 pixel grid whose cells stand twice as tall as wide.
//! A quadrant character carries a 2 by 2 block of those pixels, and since each
//! quadrant of a terminal cell is itself about twice as tall as wide, the
//! wordmark keeps the aspect ratio of the SVG at half the width half blocks
//! would need.
//!
//! Each row pairs the glyphs with a colour mask of the same length. In the
//! mask `2` marks the accent, which is the 2, `1` the heading colour that the
//! letters take, and `0` a gap.

use std::iter;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

const ROWS: [(&str, &str); 3] = [
    ("█▙ ▟█▐▛▀▙▝▀▀▙▐▛▀▙▐▛▀▙▐▛▀▀", "1101111112222111111111111"),
    ("█▝█▘█▐▌ █▗▛▀▘▐▛▀▘▐▌ █▐▛▀ ", "1111111012222111111011110"),
    ("▀   ▀▝▀▀▘▝▀▀▀▝▘  ▝▀▀▘▝▘  ", "1000111112222110011111100"),
];

/// How many rows the logo stands.
pub(super) const HEIGHT: u16 = ROWS.len() as u16;

/// The logo as coloured lines: the letters in `heading`, the 2 in `accent`.
pub(super) fn lines(
    heading: Color,
    accent: Color,
) -> Vec<Line<'static>> {
    ROWS.iter()
        .map(|(glyphs, mask)| row(glyphs, mask, heading, accent))
        .collect()
}

/// One row, with each run of same-coloured glyphs gathered into a span. The
/// mask drives it, so a glyph string that lost its trailing spaces still lines
/// up, the missing cells falling back to gaps.
fn row(
    glyphs: &str,
    mask: &str,
    heading: Color,
    accent: Color,
) -> Line<'static> {
    let mut spans = Vec::new();
    let mut run = String::new();
    let mut role = '0';

    for (glyph, cell) in glyphs.chars().chain(iter::repeat(' ')).zip(mask.chars()) {
        if cell != role && !run.is_empty() {
            spans.push(paint(std::mem::take(&mut run), role, heading, accent));
        }

        role = cell;
        run.push(glyph);
    }

    spans.push(paint(run, role, heading, accent));

    Line::from(spans)
}

fn paint(
    text: String,
    role: char,
    heading: Color,
    accent: Color,
) -> Span<'static> {
    let style = match role {
        '1' => Style::new().fg(heading),
        '2' => Style::new().fg(accent),
        _ => Style::new(),
    };

    Span::styled(text, style)
}
