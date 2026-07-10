//! An interactive front end. Pick a note, watch it typeset, write the PDF.

mod app;
mod notes;
mod preview;
mod ui;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ratatui::crossterm::event::{self, Event};
use ratatui_image::picker::Picker;

use crate::cli::ThemeName;
use crate::report::Failure;
use app::App;

/// Long enough that the loop is not a spin, short enough that the spinner turns
/// smoothly and a keypress feels answered at once.
const TICK: Duration = Duration::from_millis(80);

pub(crate) fn run(
    theme: ThemeName,
    output: Option<PathBuf>,
    start: Option<&Path>,
) -> Result<(), Failure> {
    if !std::io::stdout().is_terminal() {
        return Err(
            Failure::new("there is no terminal to draw the interface on")
                .hint("run `md2pdf -i` from a terminal, or drop the flag to convert a file"),
        );
    }

    let mut terminal = ratatui::init();

    // Asking a terminal what graphics it can draw means writing to it and
    // reading the answer back, which works only once raw mode is on and the
    // alternate screen is up. A terminal that will not answer gets half blocks,
    // which every terminal can draw.
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    let mut app = App::new(picker, theme, output, start);
    let mut dirty = true;

    let outcome = loop {
        if dirty {
            if let Err(error) = terminal.draw(|frame| ui::draw(frame, &mut app)) {
                break Err(error);
            }
            dirty = false;
        }

        match pump(&mut app) {
            Ok(handled) => dirty |= handled,
            Err(error) => break Err(error),
        }

        dirty |= app.tick();

        if app.quit {
            break Ok(());
        }
    };

    ratatui::restore();

    outcome.map_err(|error| Failure::new(format!("the interface stopped\n{error}")))
}

/// Waits up to a tick for input, then takes every keypress already waiting
/// behind it. A held key arrives as a burst, and answering the whole burst
/// before the next draw is what keeps a long scroll from lagging one frame
/// behind the keyboard. Answers whether anything happened.
fn pump(app: &mut App) -> std::io::Result<bool> {
    if !event::poll(TICK)? {
        return Ok(false);
    }

    loop {
        if let Event::Key(key) = event::read()? {
            app.on_key(key);
        }

        if !event::poll(Duration::ZERO)? {
            return Ok(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::theme::Theme;

    /// Typesetting the fixture note takes a moment, and it happens on a thread.
    const PATIENCE: Duration = Duration::from_secs(90);

    fn fixture() -> App {
        App::new(
            Picker::halfblocks(),
            ThemeName::Light,
            None,
            Some(Path::new("tests/test.md")),
        )
    }

    fn text(buffer: &Buffer) -> String {
        buffer
            .content()
            .chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Drives the loop the way `run` does, drawing and ticking, until the page
    /// has been typeset and a drawing of it has come back from the encoder.
    fn settle(
        app: &mut App,
        terminal: &mut Terminal<TestBackend>,
    ) {
        let started = Instant::now();

        while !app.page_ready() && app.failure.is_none() && started.elapsed() < PATIENCE {
            terminal.draw(|frame| ui::draw(frame, app)).unwrap();
            app.tick();
            std::thread::sleep(Duration::from_millis(10));
        }

        terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    }

    fn press(
        app: &mut App,
        code: KeyCode,
    ) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn every_panel_reaches_the_screen() {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut app = fixture();

        terminal.draw(|frame| ui::draw(frame, &mut app)).unwrap();
        let drawn = text(terminal.backend().buffer());

        for expected in [
            "md2pdf", "light", "dark", "notes", "preview", "test.md", "quit",
        ] {
            assert!(drawn.contains(expected), "no {expected} on screen\n{drawn}");
        }
    }

    /// The whole chain, from the note on disk to the pixels in the pane.
    #[test]
    fn a_note_is_laid_out_and_drawn_into_the_pane() {
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let mut app = fixture();

        settle(&mut app, &mut terminal);

        let page = app.page.as_ref().expect("the note was never laid out");

        assert!(app.failure.is_none(), "{:?}", app.failure);
        assert!(app.protocol.is_some(), "the page was never drawn");
        assert!(page.image.width() > 0 && page.image.height() > page.image.width());

        // The fixture embeds a video, a missing image and another note.
        assert_eq!(page.warnings.len(), 3, "{:?}", page.warnings);
    }

    /// A page this long has somewhere to scroll to, and scrolling it draws a
    /// different slice.
    #[test]
    fn the_page_scrolls() {
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let mut app = fixture();

        settle(&mut app, &mut terminal);
        let top = text(terminal.backend().buffer());

        assert!(app.furthest_scroll() > 0, "the page fits the pane");

        press(&mut app, KeyCode::End);
        terminal.draw(|frame| ui::draw(frame, &mut app)).unwrap();
        let end = text(terminal.backend().buffer());

        assert_ne!(top, end, "the pane drew the same slice twice");
        assert!(top.contains("top"), "the pane never said where it was");
        assert!(end.contains("end"));

        press(&mut app, KeyCode::Home);
        terminal.draw(|frame| ui::draw(frame, &mut app)).unwrap();

        assert_eq!(app.scroll, 0);
    }

    /// Toggling the theme keeps the reader's place. While the new page is being
    /// laid out the interface shows it loading rather than the old page under
    /// the new colours, so the two never disagree on screen.
    #[test]
    fn a_theme_toggle_keeps_the_place_and_never_mismatches() {
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let mut app = fixture();

        settle(&mut app, &mut terminal);
        press(&mut app, KeyCode::End);

        let scroll = app.scroll;
        press(&mut app, KeyCode::Tab);

        assert_eq!(app.theme, ThemeName::Dark);
        assert_eq!(app.scroll, scroll);
        // The page in hand is still the light one, so nothing is shown for it.
        assert!(!app.page_ready(), "a stale-theme page was left on screen");

        settle(&mut app, &mut terminal);

        // Once the dark page arrives the reader is still where they were.
        assert!(app.page_ready());
        assert_eq!(app.scroll, scroll);
    }

    /// Pressing enter hands the write to a worker and returns at once, rather
    /// than blocking the interface while the PDF is put together.
    #[test]
    fn exporting_does_not_block_the_interface() {
        let output = std::env::temp_dir().join("md2pdf-export-test.pdf");
        let _ = std::fs::remove_file(&output);

        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let mut app = App::new(
            Picker::halfblocks(),
            ThemeName::Light,
            Some(output.clone()),
            Some(Path::new("tests/test.md")),
        );

        settle(&mut app, &mut terminal);
        press(&mut app, KeyCode::Enter);

        assert!(app.writing, "the write was not handed off");

        let started = Instant::now();
        while app.writing && started.elapsed() < PATIENCE {
            app.tick();
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(!app.writing, "the write never finished");
        assert!(output.is_file(), "no PDF reached the disk");

        let notice = app.notice.as_ref().expect("the write said nothing");
        assert!(!notice.failed, "the write failed: {}", notice.text);
        assert!(notice.text.contains("wrote"));

        let _ = std::fs::remove_file(&output);
    }

    /// Typing a query that still matches the open note leaves it open, so the
    /// preview is not torn down and typeset again on every letter.
    #[test]
    fn narrowing_the_search_holds_on_to_the_open_note() {
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let mut app = fixture();

        settle(&mut app, &mut terminal);
        press(&mut app, KeyCode::Char('/'));

        for letter in "tsmd".chars() {
            press(&mut app, KeyCode::Char(letter));

            assert_eq!(app.selected(), Some(Path::new("./tests/test.md")));
            assert!(app.page.is_some(), "the page was dropped on `{letter}`");
        }

        // A query that matches nothing leaves nothing selected.
        press(&mut app, KeyCode::Char('z'));

        assert!(app.matches.is_empty());
        assert!(app.selected().is_none());
    }

    /// The interface is tinted from the same palette as the page it shows, so
    /// the light theme is drawn on white and the dark theme on near black.
    #[test]
    fn the_interface_wears_the_theme() {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();

        for (theme, expected) in [
            (ThemeName::Light, Theme::light().background),
            (ThemeName::Dark, Theme::dark().background),
        ] {
            let mut app = App::new(Picker::halfblocks(), theme, None, None);
            terminal.draw(|frame| ui::draw(frame, &mut app)).unwrap();

            let corner = terminal.backend().buffer()[(0, 0)].style().bg.unwrap();
            let wanted = ratatui::style::Color::Rgb(expected.red, expected.green, expected.blue);

            assert_eq!(
                corner,
                wanted,
                "{} is not on the right background",
                theme.label()
            );
        }
    }
}
