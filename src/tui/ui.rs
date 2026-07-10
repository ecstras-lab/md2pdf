//! Drawing the interface.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect, Size};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, List, ListItem, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Wrap,
};
use ratatui_image::Image;

use crate::theme::{self, Theme};
use crate::tui::app::{App, SPINNER};
use crate::tui::notes;

/// Wide enough for a note two folders deep, narrow enough to leave the page
/// most of the terminal.
const LIST_WIDTH: u16 = 32;

/// The skipped panel grows with what it has to say, up to this.
const SKIPPED_HEIGHT: u16 = 6;

/// The interface wears the colours of the document, so that choosing a theme
/// shows what the theme looks like instead of naming it.
struct Palette {
    background: Color,
    foreground: Color,
    muted: Color,
    border: Color,
    primary: Color,
    on_primary: Color,
    warning: Color,
    danger: Color,
}

impl Palette {
    fn of(theme: &Theme) -> Self {
        Self {
            background: rgb(theme.background),
            foreground: rgb(theme.foreground),
            muted: rgb(theme.muted_foreground),
            border: rgb(theme.rule),
            primary: rgb(theme::PRIMARY),
            on_primary: rgb(theme::PRIMARY_FOREGROUND),
            warning: rgb(theme::callout_color("warning")),
            danger: rgb(theme::callout_color("danger")),
        }
    }

    fn panel(
        &self,
        title: &'static str,
    ) -> Block<'static> {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(self.border))
            .title(Span::styled(
                format!(" {title} "),
                Style::new().fg(self.primary).bold(),
            ))
    }
}

fn rgb(color: theme::Color) -> Color {
    Color::Rgb(color.red, color.green, color.blue)
}

pub(super) fn draw(
    frame: &mut Frame,
    app: &mut App,
) {
    let palette = Palette::of(&app.theme.build());
    let background = Style::new().bg(palette.background).fg(palette.foreground);

    frame.render_widget(Block::new().style(background), frame.area());

    let skipped = app
        .page
        .as_ref()
        .map(|page| page.warnings.len())
        .unwrap_or_default();

    let panel = match skipped {
        0 => 0,
        count => u16::try_from(count)
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .min(SKIPPED_HEIGHT),
    };

    let [header, body, warnings, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(6),
        Constraint::Length(panel),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let [list, page] =
        Layout::horizontal([Constraint::Length(LIST_WIDTH), Constraint::Min(24)]).areas(body);

    draw_header(frame, header, app, &palette);
    draw_notes(frame, list, app, &palette);
    draw_page(frame, page, app, &palette);
    draw_skipped(frame, warnings, app, &palette);
    draw_footer(frame, footer, app, &palette);
}

/// The name on the left, the theme on the right, drawn as the two swatches it
/// switches between.
fn draw_header(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
) {
    let [name, themes] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(14)]).areas(area);

    frame.render_widget(
        Line::from(Span::styled(
            " md2pdf",
            Style::new().fg(palette.primary).bold(),
        )),
        name,
    );

    let swatch = |label: &'static str, chosen: bool| {
        let style = if chosen {
            Style::new()
                .bg(palette.primary)
                .fg(palette.on_primary)
                .bold()
        } else {
            Style::new().fg(palette.muted)
        };

        Span::styled(format!(" {label} "), style)
    };

    let light = app.theme == crate::cli::ThemeName::Light;

    frame.render_widget(
        Line::from(vec![swatch("light", light), swatch("dark", !light)]).right_aligned(),
        themes,
    );
}

fn draw_notes(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    palette: &Palette,
) {
    let block = palette
        .panel("notes")
        .title_bottom(search_line(app, palette));

    let items: Vec<ListItem> = app
        .matches
        .iter()
        .map(|&index| ListItem::new(notes::label(&app.notes[index])))
        .collect();

    let list = List::new(items)
        .block(block)
        .style(Style::new().fg(palette.muted))
        .highlight_symbol("› ")
        .highlight_style(Style::new().fg(palette.primary).bold());

    app.list.select(Some(app.cursor));
    frame.render_stateful_widget(list, area, &mut app.list);
}

/// The search sits on the bottom edge of the note list, where the query it
/// filters by is next to the notes it left behind.
fn search_line(
    app: &App,
    palette: &Palette,
) -> Line<'static> {
    let typed = Span::styled(app.query.clone(), Style::new().fg(palette.foreground));

    if app.searching {
        return Line::from(vec![
            Span::styled(" / ", Style::new().fg(palette.primary).bold()),
            typed,
            Span::styled("▏", Style::new().fg(palette.primary)),
            Span::raw(" "),
        ]);
    }

    if app.query.is_empty() {
        return Line::from(Span::styled(
            " / to search ",
            Style::new().fg(palette.muted),
        ));
    }

    Line::from(vec![
        Span::styled(" / ", Style::new().fg(palette.muted)),
        typed,
        Span::raw(" "),
    ])
}

fn draw_page(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    palette: &Palette,
) {
    let block = palette
        .panel("preview")
        .title_bottom(scroll_line(app, palette));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The pane is only known once it has been drawn into, and the page is scaled
    // to it, so the pane is measured here rather than before the draw.
    app.resize(Size::new(inner.width, inner.height));

    if app.page_ready() {
        let protocol = app.protocol.as_ref().expect("a ready page has a drawing");
        frame.render_widget(Image::new(protocol), inner);
    } else if let Some(failure) = &app.failure {
        let message = Paragraph::new(failure.as_str())
            .style(Style::new().fg(palette.danger))
            .wrap(Wrap { trim: true });

        frame.render_widget(message, inner.inner(Margin::new(2, 1)));
    } else if app.matches.is_empty() {
        draw_middle(
            frame,
            inner,
            "no notes here",
            Style::new().fg(palette.muted),
        );
    } else {
        let spinner = Span::styled(SPINNER[app.spinner], Style::new().fg(palette.primary));
        let label = Span::styled(" typesetting", Style::new().fg(palette.muted));

        draw_middle(frame, inner, Line::from(vec![spinner, label]), Style::new());
    }

    draw_scrollbar(frame, area, app, palette);
}

/// Centres one line in an area, for the states that have nothing but a word to
/// show, the loader among them.
fn draw_middle<'a>(
    frame: &mut Frame,
    area: Rect,
    content: impl Into<Line<'a>>,
    style: Style,
) {
    if area.height == 0 {
        return;
    }

    let middle = Rect {
        y: area.y + area.height / 2,
        height: 1,
        ..area
    };

    frame.render_widget(
        Paragraph::new(content.into()).style(style).centered(),
        middle,
    );
}

fn draw_scrollbar(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
) {
    let furthest = app.furthest_scroll();

    if furthest == 0 {
        return;
    }

    let mut state = ScrollbarState::new(furthest as usize).position(app.scroll as usize);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_symbol("┃")
        .track_style(Style::new().fg(palette.border))
        .thumb_style(Style::new().fg(palette.primary));

    frame.render_stateful_widget(scrollbar, area.inner(Margin::new(0, 1)), &mut state);
}

/// How far down the page the reader is, on the bottom edge of the pane.
fn scroll_line(
    app: &App,
    palette: &Palette,
) -> Line<'static> {
    let furthest = app.furthest_scroll();

    if furthest == 0 {
        return Line::default();
    }

    let place = if app.scroll == 0 {
        "top".to_owned()
    } else if app.scroll >= furthest {
        "end".to_owned()
    } else {
        format!("{}%", app.scroll * 100 / furthest)
    };

    Line::from(Span::styled(
        format!(" {place} "),
        Style::new().fg(palette.muted),
    ))
    .right_aligned()
}

/// Everything the converter could not draw. Each of these also leaves a marked
/// box on the page beside it.
fn draw_skipped(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
) {
    if area.height == 0 {
        return;
    }

    let Some(page) = &app.page else {
        return;
    };

    let lines: Vec<Line> = page
        .warnings
        .iter()
        .map(|warning| {
            Line::from(vec![
                Span::styled("! ", Style::new().fg(palette.warning).bold()),
                Span::styled(warning.as_str(), Style::new().fg(palette.muted)),
            ])
        })
        .collect();

    let block = palette
        .panel("skipped")
        .border_style(Style::new().fg(palette.border))
        .padding(Padding::horizontal(1));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// The keys, unless the interface has something to say, in which case it says
/// it where the keys were.
fn draw_footer(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
) {
    if let Some(notice) = &app.notice {
        let color = if notice.failed {
            palette.danger
        } else {
            palette.primary
        };

        let mark = if notice.failed { " ✗ " } else { " ✓ " };

        frame.render_widget(
            Line::from(vec![
                Span::styled(mark, Style::new().fg(color).bold()),
                Span::styled(notice.text.as_str(), Style::new().fg(palette.foreground)),
            ]),
            area,
        );

        return;
    }

    if app.writing {
        frame.render_widget(
            Line::from(vec![
                Span::styled(
                    format!(" {} ", SPINNER[app.spinner]),
                    Style::new().fg(palette.primary).bold(),
                ),
                Span::styled("writing the PDF", Style::new().fg(palette.foreground)),
            ]),
            area,
        );

        return;
    }

    let keys: &[(&str, &str)] = if app.searching {
        &[("type", "filter"), ("⏎", "accept"), ("esc", "clear")]
    } else {
        &[
            ("↑↓", "note"),
            ("t", "theme"),
            ("⏎", "export"),
            ("pgup/pgdn", "scroll"),
            ("/", "search"),
            ("q", "quit"),
        ]
    };

    let mut spans = vec![Span::raw(" ")];

    for (key, label) in keys {
        spans.push(Span::styled(
            *key,
            Style::new().fg(palette.foreground).bold(),
        ));
        spans.push(Span::styled(
            format!(" {label}   "),
            Style::new().fg(palette.muted),
        ));
    }

    frame.render_widget(Line::from(spans), area);
}
