//! What the interface knows, and what a keypress does to it.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;

use crate::cli::ThemeName;
use crate::convert;
use crate::report;
use crate::tui::notes;

/// How long a message stays on the footer after a write.
const LINGER: Duration = Duration::from_secs(6);

/// Turns once per tick while a PDF is being written.
pub(super) const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Where PDFs land when the save folder is left as it starts.
const DEFAULT_SAVE_DIR: &str = "PDF";

/// A written PDF.
pub(super) struct Export {
    path: PathBuf,
    bytes: usize,
    elapsed: Duration,
    warnings: Vec<String>,
}

/// What the export worker sends back: a written PDF, or why it could not be.
type Written = Result<Export, String>;

/// A word from the interface, shown on the footer until it goes stale.
pub(super) struct Notice {
    pub(super) text: String,
    pub(super) failed: bool,
    raised: Instant,
}

pub(super) struct App {
    pub(super) notes: Vec<PathBuf>,
    pub(super) matches: Vec<usize>,
    pub(super) cursor: usize,
    /// The list scrolls itself around the cursor, and remembers how far.
    pub(super) list: ListState,
    pub(super) query: String,
    pub(super) searching: bool,
    pub(super) theme: ThemeName,
    /// The folder a PDF is written into. The name comes from the note.
    pub(super) save_dir: String,
    pub(super) editing_save: bool,
    /// A PDF is being written.
    pub(super) writing: bool,
    pub(super) spinner: usize,
    pub(super) notice: Option<Notice>,
    /// What the last write had to skip. Each of these is marked in the PDF too.
    pub(super) skipped: Vec<String>,
    pub(super) quit: bool,

    /// The save folder as it was before an edit began, to fall back to on cancel.
    save_before_edit: String,
    done: (Sender<Written>, Receiver<Written>),
}

impl App {
    pub(super) fn new(
        theme: ThemeName,
        output: Option<PathBuf>,
        start: Option<&Path>,
    ) -> Self {
        let notes = notes::find(Path::new("."));
        let matches = (0..notes.len()).collect();
        let cursor = start
            .and_then(|wanted| position_of(&notes, wanted))
            .unwrap_or(0);

        // `--output` names a file. The folder it sits in is where the rest go.
        let save_dir = output
            .as_deref()
            .and_then(Path::parent)
            .map(|parent| parent.display().to_string())
            .filter(|shown| !shown.is_empty())
            .unwrap_or_else(|| DEFAULT_SAVE_DIR.to_owned());

        Self {
            notes,
            matches,
            cursor,
            list: ListState::default(),
            query: String::new(),
            searching: false,
            theme,
            save_dir,
            editing_save: false,
            writing: false,
            spinner: 0,
            notice: None,
            skipped: Vec::new(),
            quit: false,
            save_before_edit: String::new(),
            done: mpsc::channel(),
        }
    }

    pub(super) fn selected(&self) -> Option<&Path> {
        let index = *self.matches.get(self.cursor)?;

        Some(&self.notes[index])
    }

    /// Where the selected note would be written, folder and all.
    pub(super) fn output_path(&self) -> Option<PathBuf> {
        let stem = self.selected()?.file_stem()?;

        Some(Path::new(&self.save_dir).join(stem).with_extension("pdf"))
    }

    /// That path, written the same way on every platform.
    pub(super) fn output_display(&self) -> Option<String> {
        Some(self.output_path()?.display().to_string().replace('\\', "/"))
    }

    pub(super) fn on_key(
        &mut self,
        key: KeyEvent,
    ) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }

        if self.editing_save {
            self.on_save_key(key);
            return;
        }

        if self.searching {
            self.on_search_key(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('t') | KeyCode::Tab => self.toggle_theme(),
            KeyCode::Char('e') => self.begin_editing_save(),
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Home => self.move_to(0),
            KeyCode::End => self.move_to(self.matches.len().saturating_sub(1)),
            KeyCode::Enter => self.write(),
            _ => {}
        }
    }

    fn on_search_key(
        &mut self,
        key: KeyEvent,
    ) {
        match key.code {
            KeyCode::Esc => {
                self.searching = false;
                self.query.clear();
                self.refilter();
            }
            KeyCode::Enter => self.searching = false,
            KeyCode::Backspace => {
                self.query.pop();
                self.refilter();
            }
            KeyCode::Char(letter) => {
                self.query.push(letter);
                self.refilter();
            }
            KeyCode::Up => self.move_cursor(-1),
            KeyCode::Down => self.move_cursor(1),
            _ => {}
        }
    }

    fn on_save_key(
        &mut self,
        key: KeyEvent,
    ) {
        match key.code {
            KeyCode::Esc => {
                self.save_dir = std::mem::take(&mut self.save_before_edit);
                self.editing_save = false;
            }
            KeyCode::Enter => {
                if self.save_dir.trim().is_empty() {
                    self.save_dir = DEFAULT_SAVE_DIR.to_owned();
                }
                self.editing_save = false;
            }
            KeyCode::Backspace => {
                self.save_dir.pop();
            }
            KeyCode::Char(letter) => self.save_dir.push(letter),
            _ => {}
        }
    }

    /// Called once per turn of the loop, whether or not anything happened.
    /// Answers whether the interface has to be drawn again.
    pub(super) fn tick(&mut self) -> bool {
        let mut changed = false;

        while let Ok(result) = self.done.1.try_recv() {
            self.on_written(result);
            changed = true;
        }

        if self.writing {
            self.spinner = (self.spinner + 1) % SPINNER.len();
            changed = true;
        }

        let stale = self
            .notice
            .as_ref()
            .is_some_and(|notice| notice.raised.elapsed() > LINGER);

        if stale {
            self.notice = None;
            changed = true;
        }

        changed
    }

    fn on_written(
        &mut self,
        result: Written,
    ) {
        self.writing = false;

        match result {
            Ok(export) => {
                self.skipped = export.warnings;

                let detail = format!(
                    "{} in {}",
                    report::size(export.bytes),
                    report::duration(export.elapsed),
                );
                let shown = export.path.display().to_string().replace('\\', "/");

                self.say(format!("wrote {shown} ({detail})"), false);
            }
            Err(message) => {
                self.skipped.clear();
                self.say(message, true);
            }
        }
    }

    fn move_cursor(
        &mut self,
        step: isize,
    ) {
        if self.matches.is_empty() {
            return;
        }

        let last = self.matches.len() - 1;
        self.cursor = self.cursor.saturating_add_signed(step).min(last);
    }

    fn move_to(
        &mut self,
        cursor: usize,
    ) {
        self.cursor = cursor.min(self.matches.len().saturating_sub(1));
    }

    fn toggle_theme(&mut self) {
        self.theme = match self.theme {
            ThemeName::Light => ThemeName::Dark,
            ThemeName::Dark => ThemeName::Light,
        };
    }

    fn begin_editing_save(&mut self) {
        self.save_before_edit = self.save_dir.clone();
        self.editing_save = true;
    }

    fn refilter(&mut self) {
        let selected = self.matches.get(self.cursor).copied();

        self.matches = notes::matching(&self.notes, &self.query);

        // Keep the note the reader had selected, if the query has not filtered
        // it away, rather than jumping the cursor on every letter typed.
        let held = selected.and_then(|note| self.matches.iter().position(|&found| found == note));

        self.cursor = held.unwrap_or(0);
    }

    /// Writes the selected note to its output path, on a worker so the interface
    /// stays live while the PDF is put together.
    fn write(&mut self) {
        if self.writing {
            return;
        }

        let (Some(source), Some(output_path)) = (self.selected(), self.output_path()) else {
            self.say("there is no note to write", true);
            return;
        };

        let source = source.to_path_buf();
        let theme = self.theme;
        let done = self.done.0.clone();

        self.writing = true;

        std::thread::spawn(move || {
            let _ = done.send(export(&source, theme, output_path));
        });
    }

    fn say(
        &mut self,
        text: impl Into<String>,
        failed: bool,
    ) {
        self.notice = Some(Notice {
            text: text.into(),
            failed,
            raised: Instant::now(),
        });
    }
}

/// Reads, converts and writes a note, away from the interface thread.
fn export(
    source_path: &Path,
    theme: ThemeName,
    output_path: PathBuf,
) -> Written {
    let started = Instant::now();

    let rendered = convert::prepare(source_path, &theme.build())
        .and_then(|prepared| prepared.render())
        .map_err(|error| error.to_string())?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let bytes = rendered.pdf.len();
    std::fs::write(&output_path, rendered.pdf).map_err(|error| error.to_string())?;

    Ok(Export {
        path: output_path,
        bytes,
        elapsed: started.elapsed(),
        warnings: rendered.warnings,
    })
}

/// `tests/test.md` and `./tests/test.md` name one note. Ask the filesystem
/// rather than the strings, and fall back to the strings when it will not say.
fn position_of(
    notes: &[PathBuf],
    wanted: &Path,
) -> Option<usize> {
    let canonical = std::fs::canonicalize(wanted);

    notes.iter().position(|note| match &canonical {
        Ok(wanted) => std::fs::canonicalize(note).is_ok_and(|note| note == *wanted),
        Err(_) => note == wanted,
    })
}
