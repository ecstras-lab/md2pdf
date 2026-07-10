//! What the interface knows, and what a keypress does to it.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Size;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;

use crate::cli::{self, ThemeName};
use crate::report;
use crate::tui::notes;
use crate::tui::preview::{self, Done, EncodeReply, EncodeRequest, Page, Request};

/// A keypress is often the first of several. Waiting this long before laying the
/// note out again means holding an arrow key down costs one render rather than
/// one for every note it travelled past.
const SETTLE: Duration = Duration::from_millis(130);

/// How long a message stays on the footer after a write.
const LINGER: Duration = Duration::from_secs(5);

/// Turns once per tick while the interface is working.
pub(super) const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// A word from the interface, shown on the footer until it goes stale.
pub(super) struct Notice {
    pub(super) text: String,
    pub(super) failed: bool,
    raised: Instant,
}

pub(super) struct App {
    pub(super) picker: Picker,
    pub(super) notes: Vec<PathBuf>,
    pub(super) matches: Vec<usize>,
    pub(super) cursor: usize,
    /// The list scrolls itself around the cursor, and remembers how far.
    pub(super) list: ListState,
    pub(super) query: String,
    pub(super) searching: bool,
    pub(super) theme: ThemeName,
    pub(super) page: Option<Page>,
    pub(super) protocol: Option<Protocol>,
    pub(super) failure: Option<String>,
    /// A note is being typeset.
    pub(super) working: bool,
    /// A PDF is being written.
    pub(super) writing: bool,
    pub(super) spinner: usize,
    pub(super) notice: Option<Notice>,
    pub(super) scroll: u32,
    pub(super) pane: Size,
    pub(super) quit: bool,

    /// Where `--output` said to write, if it said anything.
    output: Option<PathBuf>,
    /// Names the typeset in flight. A reply carrying any other number answers a
    /// question nobody is asking any more.
    token: u64,
    /// When the last keypress landed, and so when to start laying out again.
    settling: Option<Instant>,
    /// The view last handed to the encoder, and the newest one drawn back. A
    /// drawing older than what is on screen is stale and dropped.
    view: u64,
    drawn: u64,
    done: (Sender<Done>, Receiver<Done>),
    encoder: (Sender<EncodeRequest>, Receiver<EncodeReply>),
}

impl App {
    pub(super) fn new(
        picker: Picker,
        theme: ThemeName,
        output: Option<PathBuf>,
        start: Option<&Path>,
    ) -> Self {
        let notes = notes::find(Path::new("."));
        let matches = (0..notes.len()).collect();
        let cursor = start
            .and_then(|wanted| position_of(&notes, wanted))
            .unwrap_or(0);

        Self {
            encoder: preview::spawn_encoder(picker.clone()),
            picker,
            notes,
            matches,
            cursor,
            list: ListState::default(),
            query: String::new(),
            searching: false,
            theme,
            page: None,
            protocol: None,
            failure: None,
            working: false,
            writing: false,
            spinner: 0,
            notice: None,
            scroll: 0,
            pane: Size::new(0, 0),
            quit: false,
            output,
            token: 0,
            settling: Some(Instant::now()),
            view: 0,
            drawn: 0,
            done: mpsc::channel(),
        }
    }

    pub(super) fn selected(&self) -> Option<&Path> {
        let index = *self.matches.get(self.cursor)?;

        Some(&self.notes[index])
    }

    /// The terminal's cell, in pixels. Halfblocks quote one too, so the pane is
    /// measured the same way whichever protocol the terminal turned out to have.
    pub(super) fn font(&self) -> Size {
        let font = self.picker.font_size();

        Size::new(font.width, font.height)
    }

    /// How far the page can still be pushed up.
    pub(super) fn furthest_scroll(&self) -> u32 {
        self.page
            .as_ref()
            .map(|page| preview::furthest_scroll(&page.image, self.pane, self.font()))
            .unwrap_or_default()
    }

    /// Whether there is a drawing to show that belongs to the note and the theme
    /// on screen right now. While a theme is being switched the page still held
    /// is the old one, and showing it under the new chrome is the mismatch a
    /// reader would notice, so this reads false until the new page arrives.
    pub(super) fn page_ready(&self) -> bool {
        self.protocol.is_some()
            && self
                .page
                .as_ref()
                .is_some_and(|page| page.theme == self.theme)
    }

    pub(super) fn on_key(
        &mut self,
        key: KeyEvent,
    ) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            let half = i64::from(self.pane.height) / 2;

            match key.code {
                KeyCode::Char('c') => self.quit = true,
                KeyCode::Char('d') => self.scroll_by(half),
                KeyCode::Char('u') => self.scroll_by(-half),
                _ => {}
            }
            return;
        }

        if self.searching {
            self.on_search_key(key);
            return;
        }

        let screen = i64::from(self.pane.height).saturating_sub(2);

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('t') | KeyCode::Tab => self.toggle_theme(),
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::PageUp => self.scroll_by(-screen),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_by(screen),
            KeyCode::Home => self.scroll_to(0),
            KeyCode::End => self.scroll_to(self.furthest_scroll()),
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

    /// Called once per turn of the loop, whether or not anything happened.
    /// Answers whether the interface has to be drawn again.
    pub(super) fn tick(&mut self) -> bool {
        let mut changed = false;

        while let Ok(done) = self.done.1.try_recv() {
            self.on_done(done);
            changed = true;
        }

        // Only the newest drawing is worth taking. Anything the encoder sends
        // for a view already left behind is dropped.
        while let Ok(reply) = self.encoder.1.try_recv() {
            if reply.generation >= self.drawn {
                self.drawn = reply.generation;
                self.protocol = reply.protocol;
                changed = true;
            }
        }

        if self.working || self.writing {
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

        let settled = self.settling.is_some_and(|since| since.elapsed() >= SETTLE);

        // The pane's width sets the scale the page is drawn at, and until the
        // first draw there is no pane to ask.
        if settled && self.pane.width > 0 {
            self.settling = None;
            self.lay_out();
            changed = true;
        }

        changed
    }

    fn on_done(
        &mut self,
        done: Done,
    ) {
        match done {
            Done::Typeset { token, page } => {
                if token != self.token {
                    return;
                }

                self.working = false;

                match page {
                    Ok(page) => {
                        self.failure = None;
                        self.page = Some(page);
                        // The old drawing is of the old page. Drop it and ask
                        // for one of the new page, so a stale image is never
                        // shown under fresh chrome.
                        self.protocol = None;
                        self.request_encode();
                    }
                    Err(message) => {
                        self.page = None;
                        self.protocol = None;
                        self.failure = Some(message);
                    }
                }
            }
            Done::Export(result) => {
                self.writing = false;

                match result {
                    Ok(export) => {
                        let detail = format!(
                            "{} in {}",
                            report::size(export.bytes),
                            report::duration(export.elapsed),
                        );

                        self.say(format!("wrote {} ({detail})", export.path.display()), false);
                    }
                    Err(message) => self.say(message, true),
                }
            }
        }
    }

    /// The pane is only known once the interface has been drawn into it, so this
    /// is called from the draw. A wider pane scales the page differently and
    /// needs a fresh render. A taller one only shows more of what is there, and
    /// a re-encode is enough.
    pub(super) fn resize(
        &mut self,
        pane: Size,
    ) {
        if self.pane == pane {
            return;
        }

        let rescaled = self.pane.width != pane.width;
        self.pane = pane;

        if rescaled {
            self.settling = Some(Instant::now());
        }

        self.request_encode();
    }

    /// Hands the encoder the current view. The drawing comes back through the
    /// tick, so this never blocks the interface, however slow the terminal is
    /// to paint.
    fn request_encode(&mut self) {
        let Some(page) = &self.page else {
            self.protocol = None;
            return;
        };

        self.scroll = self.scroll.min(self.furthest_scroll());
        self.view += 1;

        let request = EncodeRequest {
            generation: self.view,
            image: page.image.clone(),
            pane: self.pane,
            font: self.font(),
            scroll: self.scroll,
        };

        let _ = self.encoder.0.send(request);
    }

    fn lay_out(&mut self) {
        let Some(path) = self.selected().map(Path::to_path_buf) else {
            self.working = false;
            return;
        };

        self.token += 1;
        self.working = true;

        preview::typeset_in_background(
            Request {
                token: self.token,
                path,
                theme: self.theme,
                pane: self.pane,
                font: self.font(),
            },
            self.done.0.clone(),
        );
    }

    /// A different note is a different page, so the reader starts at its top.
    fn move_cursor(
        &mut self,
        step: isize,
    ) {
        if self.matches.is_empty() {
            return;
        }

        let last = self.matches.len() - 1;
        let moved = self.cursor.saturating_add_signed(step).min(last);

        if moved == self.cursor {
            return;
        }

        self.cursor = moved;
        self.show_fresh_note();
    }

    /// The same note in the other theme. The reader keeps their place in it, and
    /// the interface shows it loading rather than the old page under new colours.
    fn toggle_theme(&mut self) {
        self.theme = match self.theme {
            ThemeName::Light => ThemeName::Dark,
            ThemeName::Dark => ThemeName::Light,
        };

        self.settling = Some(Instant::now());
    }

    fn refilter(&mut self) {
        let selected = self.matches.get(self.cursor).copied();

        self.matches = notes::matching(&self.notes, &self.query);

        // Keep the note the reader was looking at, if the query has not filtered
        // it away. Otherwise the preview would be torn down on every letter
        // typed, and typeset again from scratch on the next one.
        let held = selected.and_then(|note| self.matches.iter().position(|&found| found == note));

        match held {
            Some(cursor) => self.cursor = cursor,
            None => {
                self.cursor = 0;
                self.show_fresh_note();
            }
        }
    }

    /// Clears the page and starts laying the newly chosen note out.
    fn show_fresh_note(&mut self) {
        self.page = None;
        self.protocol = None;
        self.failure = None;
        self.scroll = 0;
        self.settling = Some(Instant::now());
    }

    fn scroll_by(
        &mut self,
        rows: i64,
    ) {
        let step = rows * i64::from(preview::line_height(self.font()));
        let moved = i64::from(self.scroll).saturating_add(step);

        self.scroll_to(moved.clamp(0, i64::from(self.furthest_scroll())) as u32);
    }

    fn scroll_to(
        &mut self,
        scroll: u32,
    ) {
        let scroll = scroll.min(self.furthest_scroll());

        if scroll != self.scroll {
            self.scroll = scroll;
            self.request_encode();
        }
    }

    /// Writes the pages the reader is looking at, rather than a fresh compile
    /// that might not agree with them. The write happens on a worker, so the
    /// interface stays live while the PDF is put together.
    fn write(&mut self) {
        if self.writing {
            return;
        }

        let (document, source_path) = match (&self.page, self.selected()) {
            (Some(page), Some(source)) => (page.document.clone(), source.to_path_buf()),
            (None, _) => {
                self.say("there is no page to write yet", true);
                return;
            }
            _ => return,
        };

        let output_path = match &self.output {
            Some(path) => path.clone(),
            None => match cli::default_output_path(&source_path) {
                Ok(path) => path,
                Err(error) => {
                    self.say(error.to_string(), true);
                    return;
                }
            },
        };

        self.writing = true;
        preview::export_in_background(document, output_path, self.done.0.clone());
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
