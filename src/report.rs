//! How the command talks to a person.

use std::time::Duration;

use anstream::{eprintln, println};
use anstyle::{AnsiColor, Style};

/// Wide enough for the longest label the report prints.
const LABEL_WIDTH: usize = 7;

/// Something went wrong, said in a way that suggests what to do about it.
pub struct Failure {
    message: String,
    hints: Vec<String>,
}

impl Failure {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hints: Vec::new(),
        }
    }

    pub fn hint(
        mut self,
        hint: impl Into<String>,
    ) -> Self {
        self.hints.push(hint.into());
        self
    }

    #[cfg(test)]
    pub fn hints(&self) -> &[String] {
        &self.hints
    }

    /// The shape rustc and cargo use, down to the colon inside the colour and
    /// the seven spaces that carry a wrapped message under the headline.
    pub fn print(&self) {
        let mut lines = self.message.lines();
        let headline = lines.next().unwrap_or_default();

        eprintln!("{} {headline}", bold("error:", AnsiColor::Red));

        for line in lines {
            eprintln!("       {}", dim(line));
        }

        for hint in &self.hints {
            eprintln!("  {} {hint}", bold("help:", AnsiColor::Cyan));
        }
    }
}

/// Anything that reached the top without being turned into advice.
impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        let mut message = error.to_string();

        for cause in error.chain().skip(1) {
            message.push('\n');
            message.push_str(&cause.to_string());
        }

        Self::new(message)
    }
}

/// One `label value` line of the run report.
pub fn line(
    label: &str,
    value: &str,
) {
    println!("{} {value}", label_for(label, AnsiColor::Cyan));
}

/// An embed the converter could not draw. It is marked in the PDF as well.
pub fn skipped(reason: &str) {
    println!("{} {reason}", label_for("skipped", AnsiColor::Yellow));
}

/// A warning that must survive `--quiet`, which silences the report but not
/// what went wrong. It goes to stderr, keeping quiet stdout clean for pipes.
pub fn warning(reason: &str) {
    eprintln!("{} {reason}", label_for("warning", AnsiColor::Yellow));
}

/// The line that closes a run. The path is the part a person reaches for, so
/// it is the only value the report emphasises.
pub fn wrote(
    path: &str,
    bytes: usize,
    took: Duration,
) {
    let label = label_for("output", AnsiColor::Cyan);
    let path = paint(path, Style::new().bold());
    let detail = dim(&format!("({} in {})", size(bytes), duration(took)));

    println!("{label} {path} {detail}");
}

/// A size a person can hold in their head, rather than a byte count. The unit
/// is chosen after rounding, so a value just under a boundary cannot print as
/// the contradiction `1024 KB`.
pub fn size(bytes: usize) -> String {
    const UNIT: f64 = 1024.0;

    let bytes = bytes as f64;
    let kilobytes = bytes / UNIT;

    if bytes < UNIT {
        format!("{bytes:.0} B")
    } else if kilobytes.round() < UNIT {
        format!("{kilobytes:.0} KB")
    } else {
        format!("{:.1} MB", kilobytes / UNIT)
    }
}

/// Milliseconds below a second, seconds above it. A run reported as `0.0s`
/// tells nobody anything.
pub fn duration(took: Duration) -> String {
    if took.as_secs() == 0 {
        format!("{}ms", took.as_millis())
    } else {
        format!("{:.1}s", took.as_secs_f64())
    }
}

/// Labels are padded before they are painted, since escape codes have width
/// to a formatter and none to an eye.
fn label_for(
    label: &str,
    color: AnsiColor,
) -> String {
    let padded = format!("{label:>LABEL_WIDTH$}");
    bold(&padded, color)
}

fn bold(
    text: &str,
    color: AnsiColor,
) -> String {
    paint(text, Style::new().bold().fg_color(Some(color.into())))
}

fn dim(text: &str) -> String {
    paint(text, Style::new().dimmed())
}

/// Every line this module prints is painted, and every line leaves through
/// `anstream`, which decides what survives. It reads `NO_COLOR`, `CLICOLOR`
/// and `CLICOLOR_FORCE`, asks whether a terminal is on the far end of the
/// stream it is writing to, and on Windows either turns on escape sequence
/// handling or falls back to the console API. Text that reaches a pipe or a
/// file reaches it plain.
fn paint(
    text: &str,
    style: Style,
) -> String {
    format!("{}{text}{}", style.render(), style.render_reset())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn painting_wraps_the_text_and_puts_it_back() {
        let painted = paint("error", Style::new().bold());

        assert!(
            painted.starts_with('\u{1b}'),
            "no escape sequence: {painted:?}"
        );
        assert!(painted.contains("error"));
        assert!(painted.ends_with("\u{1b}[0m"));
    }

    /// Whatever the escape codes, `anstream` strips them back to this.
    #[test]
    fn the_text_survives_the_paint_unchanged() {
        let painted = paint("error", Style::new().dimmed());
        let plain = anstream::adapter::strip_str(&painted).to_string();

        assert_eq!(plain, "error");
    }

    /// The labels line up in a column, and the escape codes must not count
    /// toward the width.
    #[test]
    fn labels_are_padded_before_they_are_painted() {
        for label in ["theme", "source", "skipped", "output"] {
            let padded = format!("{label:>LABEL_WIDTH$}");

            assert_eq!(padded.chars().count(), LABEL_WIDTH);
            assert!(padded.ends_with(label));
        }
    }

    #[test]
    fn a_size_is_scaled_to_the_largest_unit_it_fills() {
        assert_eq!(size(512), "512 B");
        assert_eq!(size(2048), "2 KB");
        assert_eq!(size(612 * 1024), "612 KB");
        assert_eq!(size(3 * 1024 * 1024 / 2), "1.5 MB");
    }

    /// A value just under a boundary must promote, never print `1024 KB`.
    #[test]
    fn a_size_at_a_unit_boundary_promotes() {
        assert_eq!(size(1_048_570), "1.0 MB");
        assert_eq!(size(1_048_576), "1.0 MB");
    }

    #[test]
    fn a_run_under_a_second_is_reported_in_milliseconds() {
        assert_eq!(duration(Duration::from_millis(40)), "40ms");
        assert_eq!(duration(Duration::from_millis(999)), "999ms");
        assert_eq!(duration(Duration::from_millis(1200)), "1.2s");
    }
}
