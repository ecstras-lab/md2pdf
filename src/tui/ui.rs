//! Drawing the interface.

use std::path::Path;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, List, ListItem, Padding, Paragraph};

use crate::cli::ThemeName;
use crate::theme::{self, Theme};
use crate::tui::app::{App, SPINNER};
use crate::tui::notes;

/// The skipped panel grows with what it has to say, up to this.
const SKIPPED_HEIGHT: u16 = 6;

/// How wide the labels in the export panel are, so their values line up. One
/// past the longest label, `save to`, so every value keeps a gap.
const LABEL_WIDTH: usize = 8;

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

    let panel = match app.skipped.len() {
        0 => 0,
        count => u16::try_from(count)
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .min(SKIPPED_HEIGHT),
    };

    let [header, body, skipped, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(6),
        Constraint::Length(panel),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let [notes, export] =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(body);

    draw_header(frame, header, app, &palette);
    draw_notes(frame, notes, app, &palette);
    draw_export(frame, export, app, &palette);
    draw_skipped(frame, skipped, app, &palette);
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

    let light = app.theme == ThemeName::Light;

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

/// What will be written, and where. The one panel that used to hold the render.
fn draw_export(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
) {
    let block = palette.panel("export").padding(Padding::horizontal(2));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let source = app
        .selected()
        .map(notes::label)
        .unwrap_or_else(|| "no note selected".to_owned());

    let lines = vec![
        Line::default(),
        row(
            "source",
            Span::styled(source, Style::new().fg(palette.foreground)),
            palette,
        ),
        row(
            "theme",
            Span::styled(app.theme.label(), Style::new().fg(palette.foreground)),
            palette,
        ),
        save_row(app, palette),
        Line::default(),
        action_line(app, palette),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

/// One `label  value` line of the export panel.
fn row<'a>(
    label: &'static str,
    value: Span<'a>,
    palette: &Palette,
) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{label:LABEL_WIDTH$}"),
            Style::new().fg(palette.muted),
        ),
        value,
    ])
}

/// The save line, editable in place. While it is being typed the folder carries
/// a cursor, and the file name the note lends it trails behind in muted text so
/// it is plain that only the folder is being changed.
fn save_row(
    app: &App,
    palette: &Palette,
) -> Line<'static> {
    let filename = app
        .selected()
        .and_then(Path::file_stem)
        .map(|stem| format!("/{}.pdf", stem.to_string_lossy()))
        .unwrap_or_default();

    if app.editing_save {
        return Line::from(vec![
            Span::styled(
                format!("{:LABEL_WIDTH$}", "save to"),
                Style::new().fg(palette.muted),
            ),
            Span::styled(app.save_dir.clone(), Style::new().fg(palette.foreground)),
            Span::styled("▏", Style::new().fg(palette.primary)),
            Span::styled(filename, Style::new().fg(palette.muted)),
        ]);
    }

    let shown = app
        .output_display()
        .unwrap_or_else(|| format!("{}{filename}", app.save_dir));

    row(
        "save to",
        Span::styled(shown, Style::new().fg(palette.foreground)),
        palette,
    )
}

/// The line under the fields, which says what pressing enter will do, or that a
/// write is under way.
fn action_line(
    app: &App,
    palette: &Palette,
) -> Line<'static> {
    if app.writing {
        return Line::from(vec![
            Span::styled(
                format!("{} ", SPINNER[app.spinner]),
                Style::new().fg(palette.primary).bold(),
            ),
            Span::styled("writing the PDF", Style::new().fg(palette.foreground)),
        ]);
    }

    if app.editing_save {
        return Line::from(Span::styled(
            "⏎ set folder    esc cancel",
            Style::new().fg(palette.muted),
        ));
    }

    Line::from(vec![
        Span::styled("⏎", Style::new().fg(palette.primary).bold()),
        Span::styled(" to export", Style::new().fg(palette.muted)),
    ])
}

/// Everything the last write could not draw. Each of these also left a marked
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

    let lines: Vec<Line> = app
        .skipped
        .iter()
        .map(|warning| {
            Line::from(vec![
                Span::styled("! ", Style::new().fg(palette.warning).bold()),
                Span::styled(warning.as_str(), Style::new().fg(palette.muted)),
            ])
        })
        .collect();

    let block = palette.panel("skipped").padding(Padding::horizontal(1));

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

    let keys: &[(&str, &str)] = if app.searching {
        &[("type", "filter"), ("⏎", "accept"), ("esc", "clear")]
    } else if app.editing_save {
        &[("type", "folder"), ("⏎", "set"), ("esc", "cancel")]
    } else {
        &[
            ("↑↓", "note"),
            ("t", "theme"),
            ("e", "save to"),
            ("⏎", "export"),
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
