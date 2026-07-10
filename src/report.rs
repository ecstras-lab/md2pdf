//! How the command talks to a person.

use std::io::IsTerminal;

use clap::builder::styling::{AnsiColor, Reset, Style};

/// The run report goes to stdout. Failures go to stderr. Each stream decides
/// on its own whether anyone is there to see the colour.
enum Stream {
    Out,
    Err,
}

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

    pub fn print(&self) {
        let mut lines = self.message.lines();
        let headline = lines.next().unwrap_or_default();

        eprintln!("{}: {headline}", bold("error", AnsiColor::Red, Stream::Err));

        for line in lines {
            eprintln!("       {}", dim(line));
        }

        for hint in &self.hints {
            eprintln!("  {}: {hint}", bold("help", AnsiColor::Cyan, Stream::Err));
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

/// Labels are padded before they are painted, since escape codes have width
/// to a formatter and none to an eye.
fn label_for(
    label: &str,
    color: AnsiColor,
) -> String {
    let padded = format!("{label:>LABEL_WIDTH$}");
    bold(&padded, color, Stream::Out)
}

fn bold(
    text: &str,
    color: AnsiColor,
    stream: Stream,
) -> String {
    let style = Style::new().bold().fg_color(Some(color.into()));
    paint(text, style, colored(stream))
}

fn dim(text: &str) -> String {
    paint(text, Style::new().dimmed(), colored(Stream::Err))
}

fn paint(
    text: &str,
    style: Style,
    enabled: bool,
) -> String {
    if !enabled {
        return text.to_owned();
    }

    format!("{}{text}{}", style.render(), Reset.render())
}

/// Colour is for a person at a terminal. A pipe or `NO_COLOR` gets plain text.
fn colored(stream: Stream) -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }

    match stream {
        Stream::Out => std::io::stdout().is_terminal(),
        Stream::Err => std::io::stderr().is_terminal(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn painting_wraps_the_text_and_puts_it_back() {
        let style = Style::new().bold().fg_color(Some(AnsiColor::Red.into()));
        let painted = paint("error", style, true);

        assert!(
            painted.starts_with('\u{1b}'),
            "no escape sequence: {painted:?}"
        );
        assert!(painted.contains("error"));
        assert!(painted.ends_with(&Reset.render().to_string()));
    }

    #[test]
    fn a_pipe_gets_the_text_and_nothing_else() {
        let painted = paint("error", Style::new().dimmed(), false);

        assert_eq!(painted, "error");
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
}
