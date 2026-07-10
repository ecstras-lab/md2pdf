//! How the command talks to a person.

use std::io::IsTerminal;

use clap::builder::styling::{AnsiColor, Reset, Style};

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

        eprintln!("{}", paint("error", AnsiColor::Red) + ": " + headline);

        for line in lines {
            eprintln!("       {}", dim(line));
        }

        for hint in &self.hints {
            eprintln!("  {}: {hint}", paint("help", AnsiColor::Cyan));
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

/// A run that produced a PDF, with something the reader should know about.
pub fn note(message: &str) {
    eprintln!("{}: {message}", paint("note", AnsiColor::Yellow));
}

fn paint(
    text: &str,
    color: AnsiColor,
) -> String {
    if !colored() {
        return text.to_owned();
    }

    let style = Style::new().bold().fg_color(Some(color.into()));
    format!("{}{text}{}", style.render(), Reset.render())
}

fn dim(text: &str) -> String {
    if !colored() {
        return text.to_owned();
    }

    let style = Style::new().dimmed();
    format!("{}{text}{}", style.render(), Reset.render())
}

/// Colour is for a person at a terminal. A pipe or `NO_COLOR` gets plain text.
fn colored() -> bool {
    std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}
