//! What the interface knows, and what a keypress does to it.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;
use unicode_segmentation::UnicodeSegmentation;

use crate::cli::ThemeName;
use crate::convert;
use crate::files;
use crate::report;
use crate::tui::notes;

/// How long a message stays on the footer after a write.
const LINGER: Duration = Duration::from_secs(6);

/// How often the spinner turns. Time based, so a burst of keypresses cannot
/// spin it faster than an idle wait would.
const SPIN: Duration = Duration::from_millis(80);

/// How long the key that crossed a mode boundary is ignored on the other
/// side. Auto repeat delivers a held Esc or Enter as a burst, and the tail of
/// the burst must not quit the app or fire an export.
const MODE_COOLDOWN: Duration = Duration::from_millis(400);

/// The spinner frames, one per turn.
pub(super) const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// A written PDF.
pub(super) struct Export {
    path: PathBuf,
    bytes: usize,
    elapsed: Duration,
    warnings: Vec<String>,
}

/// What the export worker sends back, a written PDF or why it could not be.
type Written = Result<Export, String>;

/// A word from the interface, shown on the footer until it goes stale.
pub(super) struct Notice {
    pub(super) text: String,
    pub(super) failed: bool,
    raised: Instant,
}

pub(super) struct App {
    pub(super) notes: Vec<PathBuf>,
    /// How each file is shown and searched, computed once beside `notes`.
    pub(super) labels: Vec<String>,
    pub(super) matches: Vec<usize>,
    pub(super) cursor: usize,
    /// The list scrolls itself around the cursor, and remembers how far.
    pub(super) list: ListState,
    pub(super) query: String,
    pub(super) searching: bool,
    pub(super) theme: ThemeName,
    /// The folder a PDF is written into. The file keeps its own name, and its
    /// folders under the working directory, so two same named notes cannot
    /// land on each other.
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
    /// The key that last crossed a mode boundary, and when. Its auto repeat
    /// tail is swallowed rather than dispatched into the new mode.
    mode_switch: Option<(KeyCode, Instant)>,
    /// When the spinner last turned.
    last_spin: Instant,
    /// The export worker, held so quitting can wait for the file to finish.
    writer: Option<JoinHandle<()>>,
    done: (Sender<Written>, Receiver<Written>),
}

impl App {
    pub(super) fn new(
        theme: ThemeName,
        output: Option<PathBuf>,
        start: Option<&Path>,
    ) -> Self {
        let notes = notes::find(Path::new("."));
        let labels = notes.iter().map(|path| files::display(path)).collect();
        let matches = (0..notes.len()).collect();
        let cursor = start
            .and_then(|wanted| position_of(&notes, wanted))
            .unwrap_or(0);

        // `--output` names a file. The folder it sits in is where the rest
        // go, and a bare file name means the working directory itself.
        let save_dir = output
            .as_deref()
            .and_then(Path::parent)
            .map(|parent| match parent.as_os_str().is_empty() {
                true => ".".to_owned(),
                false => parent.display().to_string(),
            })
            .unwrap_or_else(|| files::DEFAULT_OUTPUT_DIR.to_owned());

        Self {
            notes,
            labels,
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
            mode_switch: None,
            last_spin: Instant::now(),
            writer: None,
            done: mpsc::channel(),
        }
    }

    pub(super) fn selected(&self) -> Option<&Path> {
        let index = *self.matches.get(self.cursor)?;

        Some(&self.notes[index])
    }

    /// How the selected file is shown, in the list and the export panel.
    pub(super) fn selected_label(&self) -> Option<&str> {
        let index = *self.matches.get(self.cursor)?;

        Some(&self.labels[index])
    }

    /// The selected file's PDF, relative to the save folder: its folders
    /// under the working directory, then its own name with the pdf extension.
    fn relative_output(&self) -> Option<PathBuf> {
        let source = self.selected()?;

        let relative = source
            .parent()
            .map(|parent| parent.strip_prefix(".").unwrap_or(parent))
            .unwrap_or(Path::new(""));

        Some(relative.join(files::pdf_file_name(source)?))
    }

    /// What the selected file appends to the save folder, as the edit-in-place
    /// preview shows it.
    pub(super) fn output_suffix(&self) -> Option<String> {
        Some(format!("/{}", files::display(&self.relative_output()?)))
    }

    /// Where the selected note would be written. The source tree is mirrored
    /// beneath the save folder, exactly as the command mirrors it under PDF/.
    pub(super) fn output_path(&self) -> Option<PathBuf> {
        Some(Path::new(&self.save_dir).join(self.relative_output()?))
    }

    /// That path, written the same way on every platform.
    pub(super) fn output_display(&self) -> Option<String> {
        Some(files::display(&self.output_path()?))
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

        if self.repeated_boundary_key(key.code) {
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
            KeyCode::Char('/') => {
                self.searching = true;
                self.cross_mode(key.code);
            }
            KeyCode::Char('t') | KeyCode::Tab => self.toggle_theme(),
            KeyCode::Char('e') => {
                self.begin_editing_save();
                self.cross_mode(key.code);
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Home => self.move_to(0),
            KeyCode::End => self.move_to(self.matches.len().saturating_sub(1)),
            KeyCode::Enter => self.write(),
            _ => {}
        }
    }

    /// Whether this key is the auto repeat tail of the press that crossed the
    /// last mode boundary. Swallowing it keeps a held Esc from quitting the
    /// app after it closed the search, and a held `/` from typing slashes
    /// into the query it just opened.
    fn repeated_boundary_key(
        &mut self,
        code: KeyCode,
    ) -> bool {
        let Some((boundary, at)) = self.mode_switch else {
            return false;
        };

        if boundary == code && at.elapsed() < MODE_COOLDOWN {
            // Still held. Push the window along until it is released.
            self.mode_switch = Some((boundary, Instant::now()));
            return true;
        }

        self.mode_switch = None;
        false
    }

    /// Records the key that just moved the interface into another mode.
    fn cross_mode(
        &mut self,
        code: KeyCode,
    ) {
        self.mode_switch = Some((code, Instant::now()));
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
                self.cross_mode(key.code);
            }
            KeyCode::Enter => {
                self.searching = false;
                self.cross_mode(key.code);
            }
            KeyCode::Backspace => {
                pop_grapheme(&mut self.query);
                self.refilter();
            }
            KeyCode::Char(letter) if plain(key) => {
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
                self.cross_mode(key.code);
            }
            KeyCode::Enter => {
                if self.save_dir.trim().is_empty() {
                    self.save_dir = files::DEFAULT_OUTPUT_DIR.to_owned();
                }
                self.editing_save = false;
                self.cross_mode(key.code);
            }
            KeyCode::Backspace => {
                pop_grapheme(&mut self.save_dir);
            }
            KeyCode::Char(letter) if plain(key) => self.save_dir.push(letter),
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

        // The spinner turns with the clock, never with the event rate.
        if self.writing && self.last_spin.elapsed() >= SPIN {
            self.spinner = (self.spinner + 1) % SPINNER.len();
            self.last_spin = Instant::now();
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

    /// How long the event loop may sleep before this state needs another look.
    /// An idle interface has nothing to animate and nothing to expire, so it
    /// only wakes for input.
    pub(super) fn tick_rate(&self) -> Duration {
        if self.writing {
            SPIN
        } else if self.notice.is_some() {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(30)
        }
    }

    /// Waits for an in flight write to reach the disk. Called on the way out,
    /// because killing the worker mid write would leave a truncated PDF.
    pub(super) fn finish_write(&mut self) {
        let Some(writer) = self.writer.take() else {
            return;
        };

        if self.writing {
            report::warning("finishing the export before closing");
        }

        let _ = writer.join();
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
                let shown = files::display(&export.path);

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

        self.matches = notes::matching(&self.labels, &self.query);

        // Keep the note the reader had selected, if the query has not filtered
        // it away, rather than jumping the cursor on every letter typed.
        let held = selected.and_then(|note| self.matches.iter().position(|&found| found == note));

        self.cursor = held.unwrap_or(0);
    }

    /// Writes the selected note to its output path, on a worker so the
    /// interface stays live while the PDF is put together.
    fn write(&mut self) {
        if self.writing {
            self.say("still writing the last export", false);
            return;
        }

        let (Some(source), Some(output_path)) = (self.selected(), self.output_path()) else {
            self.say("there is no file to write", true);
            return;
        };

        let source = source.to_path_buf();
        let theme = self.theme;
        let done = self.done.0.clone();

        self.writing = true;
        self.writer = Some(std::thread::spawn(move || {
            let _ = done.send(export(&source, theme, output_path));
        }));
    }

    fn say(
        &mut self,
        text: impl Into<String>,
        failed: bool,
    ) {
        // The footer is one row tall, and a multi line diagnostic would mash
        // its lines together unreadably.
        let text = text.into().lines().collect::<Vec<_>>().join(" · ");

        self.notice = Some(Notice {
            text,
            failed,
            raised: Instant::now(),
        });
    }
}

/// Removes the last grapheme, not the last scalar, so an accent or an emoji
/// modifier never sheds only half of itself.
fn pop_grapheme(text: &mut String) {
    if let Some((offset, _)) = text.grapheme_indices(true).next_back() {
        text.truncate(offset);
    }
}

/// A keypress with no chord modifier. Ctrl+V and friends must not type their
/// base letter into a text field.
fn plain(key: KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// Runs the shared export away from the interface thread.
fn export(
    source_path: &Path,
    theme: ThemeName,
    output_path: PathBuf,
) -> Written {
    let started = Instant::now();

    let exported = convert::export(source_path, &theme.build(), &output_path)
        .map_err(|error| report::error_line(&error))?;

    Ok(Export {
        path: output_path,
        bytes: exported.bytes,
        elapsed: started.elapsed(),
        warnings: exported.warnings,
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
