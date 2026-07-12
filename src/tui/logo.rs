//! The wordmark, drawn in quadrant blocks from the project's SVG logo.
//!
//! The logo is a 50 by 5 pixel grid. The SVG separates its letters by a single
//! pixel, which is plenty at print size and nothing at all in a terminal, so a
//! second blank column is added to every gap and the letters land on their own
//! cells. Only quadrant characters are used, because they sit in the Block
//! Elements range that every terminal font carries. Nothing denser exists
//! there. The sextants that would halve the height rendered as empty boxes on
//! fonts without Unicode's legacy computing symbols.
//!
//! Each row pairs the glyphs with a colour mask of the same length. In the
//! mask `2` marks the accent, which is the 2, `1` the heading colour that the
//! letters take, and `0` a gap.

use std::iter;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

const ROWS: [(&str, &str); 3] = [
    (
        "█▙ ▟█ █▀▜▖▝▀▀▙ █▀▜▖▐▛▀▙ █▀▀▘",
        "1101101111222201111111101111",
    ),
    (
        "█▝█▘█ █ ▐▌▗▛▀▘ █▀▀ ▐▌ █ █▀▘ ",
        "1111101011222201110110101110",
    ),
    (
        "▀   ▀ ▀▀▀ ▝▀▀▀ ▀   ▝▀▀▘ ▀   ",
        "1000101110222201000111101000",
    ),
];

/// How many rows the logo stands.
pub(super) const HEIGHT: u16 = ROWS.len() as u16;

/// The logo as coloured lines, the letters in `heading` and the 2 in `accent`.
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
