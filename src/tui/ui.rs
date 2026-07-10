//! Drawing the interface.
//!
//! The look is borderless on purpose. Rather than box every section, it leans
//! on three hairline rules, quiet uppercase eyebrows that a reader cannot
//! mistake for content, and one accent, the document's own rose, spent only on
//! the things you act on: the selected note, the live theme, the export button.

use std::path::Path;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Padding, Paragraph};

use crate::cli::ThemeName;
use crate::theme::{self, Theme};
use crate::tui::app::{App, SPINNER};
use crate::tui::notes;

/// The skipped band grows with what it has to say, up to this.
const SKIPPED_HEIGHT: u16 = 5;

/// The interface wears the colours of the document, so that choosing a theme
/// shows what the theme looks like instead of naming it.
struct Palette {
    background: Color,
    foreground: Color,
    heading: Color,
    muted: Color,
    rule: Color,
    accent: Color,
    on_accent: Color,
    warning: Color,
    danger: Color,
}

impl Palette {
    fn of(theme: &Theme) -> Self {
        Self {
            background: rgb(theme.background),
            foreground: rgb(theme.foreground),
            heading: rgb(theme.heading),
            muted: rgb(theme.muted_foreground),
            rule: rgb(theme.rule),
            accent: rgb(theme::PRIMARY),
            on_accent: rgb(theme::PRIMARY_FOREGROUND),
            warning: rgb(theme::callout_color("warning")),
            danger: rgb(theme::callout_color("danger")),
        }
    }
}

fn rgb(color: theme::Color) -> Color {
    Color::Rgb(color.red, color.green, color.blue)
}

/// An uppercase section label. Quiet and tracked, so it reads as a heading and
/// never as a line of content.
fn eyebrow(
    text: &str,
    palette: &Palette,
) -> Span<'static> {
    let tracked: String = text
        .chars()
        .flat_map(|letter| [letter, ' '])
        .collect::<String>()
        .trim_end()
        .to_owned();

    Span::styled(tracked, Style::new().fg(palette.muted).bold())
}

/// A full width hairline across the top of its row.
fn rule(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
) {
    let hairline = Block::new()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(palette.rule));

    frame.render_widget(hairline, area);
}

pub(super) fn draw(
    frame: &mut Frame,
    app: &mut App,
) {
    let palette = Palette::of(&app.theme.build());
    frame.render_widget(
        Block::new().style(Style::new().bg(palette.background).fg(palette.foreground)),
        frame.area(),
    );

    let band = match app.skipped.len() {
        0 => 0,
        count => u16::try_from(count)
            .unwrap_or(u16::MAX)
            .saturating_add(1)
            .min(SKIPPED_HEIGHT),
    };

    let [masthead, top_rule, body, skipped, bottom_rule, keybar] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(4),
        Constraint::Length(band),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_masthead(frame, masthead, app, &palette);
    rule(frame, top_rule, &palette);
    draw_body(frame, body, app, &palette);
    draw_skipped(frame, skipped, app, &palette);
    rule(frame, bottom_rule, &palette);
    draw_keybar(frame, keybar, app, &palette);
}

/// The wordmark, and the theme shown as the two swatches it toggles between.
fn draw_masthead(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
) {
    let [mark, themes] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(16)]).areas(area);

    frame.render_widget(
        Line::from(vec![
            Span::styled(" md2", Style::new().fg(palette.heading).bold()),
            Span::styled("pdf", Style::new().fg(palette.accent).bold()),
        ]),
        mark,
    );

    let swatch = |label: &'static str, live: bool| {
        let style = if live {
            Style::new().bg(palette.accent).fg(palette.on_accent).bold()
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

fn draw_body(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    palette: &Palette,
) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)]).areas(area);

    // The one vertical hairline, drawn as the note column's right edge.
    let divider = Block::new()
        .borders(Borders::RIGHT)
        .border_style(Style::new().fg(palette.rule))
        .padding(Padding::new(1, 2, 1, 0));
    let notes = divider.inner(left);
    frame.render_widget(divider, left);

    let export = Block::new().padding(Padding::new(3, 1, 1, 0)).inner(right);

    draw_notes(frame, notes, app, palette);
    draw_export(frame, export, app, palette);
}

fn draw_notes(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    palette: &Palette,
) {
    let [head, list_area, filter] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);

    frame.render_widget(
        Line::from(vec![
            eyebrow("NOTES", palette),
            Span::styled(
                format!("   {}", app.matches.len()),
                Style::new().fg(palette.muted),
            ),
        ]),
        head,
    );

    let items: Vec<ListItem> = app
        .matches
        .iter()
        .map(|&index| ListItem::new(notes::label(&app.notes[index])))
        .collect();

    let list = List::new(items)
        .style(Style::new().fg(palette.muted))
        .highlight_symbol("  ")
        .highlight_style(Style::new().bg(palette.accent).fg(palette.on_accent).bold());

    app.list.select(Some(app.cursor));
    frame.render_stateful_widget(list, list_area, &mut app.list);

    frame.render_widget(search_line(app, palette), filter);
}

/// The filter, on its own line under the list.
fn search_line(
    app: &App,
    palette: &Palette,
) -> Line<'static> {
    if app.searching {
        return Line::from(vec![
            Span::styled("/ ", Style::new().fg(palette.accent).bold()),
            Span::styled(app.query.clone(), Style::new().fg(palette.foreground)),
            Span::styled("▏", Style::new().fg(palette.accent)),
        ]);
    }

    if app.query.is_empty() {
        return Line::from(Span::styled("/ filter", Style::new().fg(palette.muted)));
    }

    Line::from(vec![
        Span::styled("/ ", Style::new().fg(palette.muted)),
        Span::styled(app.query.clone(), Style::new().fg(palette.foreground)),
    ])
}

/// The note, flowing down into the PDF it becomes.
fn draw_export(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
) {
    let [head, _, note, arrow, dest, _, action] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);

    let status = if app.writing { "WRITING" } else { "EXPORT" };
    frame.render_widget(Line::from(eyebrow(status, palette)), head);

    let name = app
        .selected()
        .map(notes::label)
        .unwrap_or_else(|| "no note selected".to_owned());
    frame.render_widget(
        Line::from(Span::styled(name, Style::new().fg(palette.heading).bold())),
        note,
    );

    frame.render_widget(
        Line::from(Span::styled("↓", Style::new().fg(palette.muted))),
        arrow,
    );

    frame.render_widget(destination(app, palette), dest);

    draw_action(frame, action, app, palette);
}

/// Where the PDF lands, with the folder editable in place. While it is typed the
/// folder carries a cursor and the name the note lends it trails behind, muted,
/// so it is plain only the folder is changing.
fn destination(
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
            Span::styled(app.save_dir.clone(), Style::new().fg(palette.foreground)),
            Span::styled("▏", Style::new().fg(palette.accent)),
            Span::styled(filename, Style::new().fg(palette.muted)),
        ]);
    }

    let shown = app
        .output_display()
        .unwrap_or_else(|| format!("{}{filename}", app.save_dir));

    Line::from(Span::styled(shown, Style::new().fg(palette.foreground)))
}

/// The button under the flow, or what a write or an edit is asking for.
fn draw_action(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
) {
    let line = if app.writing {
        Line::from(vec![
            Span::styled(
                format!("{} ", SPINNER[app.spinner]),
                Style::new().fg(palette.accent).bold(),
            ),
            Span::styled("writing the PDF", Style::new().fg(palette.foreground)),
        ])
    } else if app.editing_save {
        Line::from(Span::styled(
            "enter to set    esc to cancel",
            Style::new().fg(palette.muted),
        ))
    } else {
        Line::from(Span::styled(
            "  export  ⏎  ",
            Style::new().bg(palette.accent).fg(palette.on_accent).bold(),
        ))
    };

    frame.render_widget(Paragraph::new(line), area);
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

    let mut lines = vec![Line::from(eyebrow("SKIPPED", palette))];

    lines.extend(app.skipped.iter().map(|warning| {
        Line::from(vec![
            Span::styled("· ", Style::new().fg(palette.warning)),
            Span::styled(warning.as_str(), Style::new().fg(palette.muted)),
        ])
    }));

    frame.render_widget(
        Paragraph::new(lines).block(Block::new().padding(Padding::horizontal(1))),
        area,
    );
}

/// The keys, unless the interface has something to say, in which case it says it
/// where the keys were.
fn draw_keybar(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
) {
    if let Some(notice) = &app.notice {
        let accent = if notice.failed {
            palette.danger
        } else {
            palette.accent
        };
        let mark = if notice.failed { "✗" } else { "✓" };

        frame.render_widget(
            Line::from(vec![
                Span::styled(format!(" {mark} "), Style::new().fg(accent).bold()),
                Span::styled(notice.text.as_str(), Style::new().fg(palette.foreground)),
            ]),
            area,
        );

        return;
    }

    let keys: &[(&str, &str)] = if app.searching {
        &[("type", "filter"), ("⏎", "done"), ("esc", "clear")]
    } else if app.editing_save {
        &[("type", "folder"), ("⏎", "set"), ("esc", "cancel")]
    } else {
        &[
            ("↑↓", "move"),
            ("t", "theme"),
            ("e", "folder"),
            ("⏎", "export"),
            ("/", "filter"),
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
            format!(" {label}    "),
            Style::new().fg(palette.muted),
        ));
    }

    frame.render_widget(Line::from(spans), area);
}
