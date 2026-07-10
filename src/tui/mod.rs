//! An interactive front end. Pick a note, choose where it lands, write the PDF.

mod app;
mod logo;
mod notes;
mod ui;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ratatui::crossterm::event::{self, Event};

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
    let mut app = App::new(theme, output, start);
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
/// before the next draw keeps the cursor from lagging behind the keyboard.
/// Answers whether anything happened.
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

    /// Writing the fixture note takes a moment, and it happens on a thread.
    const PATIENCE: Duration = Duration::from_secs(90);

    fn fixture() -> App {
        App::new(ThemeName::Dark, None, Some(Path::new("tests/test.md")))
    }

    fn text(buffer: &Buffer) -> String {
        buffer
            .content()
            .chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn draw(
        app: &mut App,
        terminal: &mut Terminal<TestBackend>,
    ) -> String {
        terminal.draw(|frame| ui::draw(frame, app)).unwrap();
        text(terminal.backend().buffer())
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

        let drawn = draw(&mut app, &mut terminal);

        // The wordmark is a block-art logo now, not the literal text, so the
        // masthead is checked by its theme swatches instead.
        for expected in [
            "light",
            "dark",
            "tests/test.md",
            "PDF/test.pdf",
            "export",
            "quit",
        ] {
            assert!(drawn.contains(expected), "no {expected} on screen\n{drawn}");
        }
    }

    #[test]
    fn the_export_panel_shows_where_the_pdf_will_land() {
        let app = fixture();

        assert_eq!(
            app.output_display().as_deref(),
            Some("PDF/test.pdf"),
            "the default save folder is not shown",
        );
    }

    /// Typing a folder changes where the PDF will land, and escaping puts the
    /// old folder back.
    #[test]
    fn the_save_folder_can_be_edited_and_the_edit_undone() {
        let mut app = fixture();

        retype_save_folder(&mut app, "out");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.output_display().as_deref(), Some("out/test.pdf"));

        retype_save_folder(&mut app, "reports");
        press(&mut app, KeyCode::Esc);
        // Escape restores the folder from before this edit.
        assert_eq!(app.output_display().as_deref(), Some("out/test.pdf"));
    }

    /// Enters the save field, clears it, and types a new folder, the way a
    /// reader replacing the path would.
    fn retype_save_folder(
        app: &mut App,
        folder: &str,
    ) {
        press(app, KeyCode::Char('e'));
        for _ in 0..64 {
            press(app, KeyCode::Backspace);
        }
        for letter in folder.chars() {
            press(app, KeyCode::Char(letter));
        }
    }

    #[test]
    fn the_theme_toggles() {
        let mut app = fixture();
        assert_eq!(app.theme, ThemeName::Dark);

        press(&mut app, KeyCode::Char('t'));
        assert_eq!(app.theme, ThemeName::Light);
    }

    /// Typing a query narrows the list but keeps the note that was selected, so
    /// the export panel does not jump around while the reader searches.
    #[test]
    fn narrowing_the_search_keeps_the_selection() {
        let mut app = fixture();
        assert_eq!(app.selected(), Some(Path::new("./tests/test.md")));

        press(&mut app, KeyCode::Char('/'));
        for letter in "tsmd".chars() {
            press(&mut app, KeyCode::Char(letter));
            assert_eq!(app.selected(), Some(Path::new("./tests/test.md")));
        }

        press(&mut app, KeyCode::Char('z'));
        assert!(app.matches.is_empty());
        assert!(app.selected().is_none());
    }

    /// The whole point. Enter hands the write to a worker, the interface stays
    /// live, and a PDF reaches the disk with the skipped embeds reported.
    #[test]
    fn pressing_enter_writes_a_pdf_off_the_interface_thread() {
        let output = std::env::temp_dir().join("md2pdf-tui-test");
        let _ = std::fs::remove_dir_all(&output);

        let mut app = App::new(
            ThemeName::Dark,
            Some(output.join("x.pdf")),
            Some(Path::new("tests/test.md")),
        );

        press(&mut app, KeyCode::Enter);
        assert!(app.writing, "the write was not handed off");

        let started = Instant::now();
        while app.writing && started.elapsed() < PATIENCE {
            app.tick();
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(!app.writing, "the write never finished");
        assert!(output.join("test.pdf").is_file(), "no PDF reached the disk");

        // The fixture embeds a video, a missing image and another note.
        assert_eq!(app.skipped.len(), 3, "{:?}", app.skipped);

        let notice = app.notice.as_ref().expect("the write said nothing");
        assert!(!notice.failed, "the write failed: {}", notice.text);
        assert!(notice.text.contains("wrote"));

        let _ = std::fs::remove_dir_all(&output);
    }

    /// The interface is tinted from the same palette as the document, so the
    /// light theme is drawn on white and the dark theme on near black.
    #[test]
    fn the_interface_wears_the_theme() {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();

        for (theme, expected) in [
            (ThemeName::Light, Theme::light().background),
            (ThemeName::Dark, Theme::dark().background),
        ] {
            let mut app = App::new(theme, None, None);
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
