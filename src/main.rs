//! Converts an Obsidian-flavoured Markdown file into a themed PDF.

mod cli;
mod convert;
mod document;
mod markdown;
mod report;
mod theme;
mod tui;

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};

use report::Failure;
use theme::Theme;

fn main() {
    let Err(failure) = run() else {
        return;
    };

    failure.print();
    std::process::exit(1);
}

fn run() -> Result<(), Failure> {
    let cli = cli::parse();
    let theme = cli.chosen_theme();

    let source_path = cli.file.clone().map(cli::with_markdown_extension);

    if let Some(path) = &source_path
        && !path.is_file()
    {
        return Err(cli::missing_source(path));
    }

    if cli.interactive {
        return tui::run(theme, cli.output, source_path.as_deref());
    }

    // Only `--interactive` lets the file be left out, and it has been handled.
    let source_path = source_path.expect("clap asks for a file unless the interface is wanted");

    let output_path = match cli.output {
        Some(path) => path,
        None => cli::default_output_path(&source_path)?,
    };

    let started = Instant::now();
    let outcome = write_pdf(&source_path, &output_path, &theme.build())?;
    let took = started.elapsed();

    if cli.quiet {
        return Ok(());
    }

    report::line("theme", theme.label());
    report::line("source", &source_path.display().to_string());

    for warning in &outcome.warnings {
        report::skipped(warning);
    }

    report::wrote(&output_path.display().to_string(), outcome.bytes, took);

    Ok(())
}

/// What a run has to say for itself once the file is on disk.
struct Outcome {
    /// Everything that had to be skipped.
    warnings: Vec<String>,
    /// The size of the PDF.
    bytes: usize,
}

fn write_pdf(
    source_path: &Path,
    output_path: &Path,
    theme: &Theme,
) -> Result<Outcome> {
    let typeset = convert::prepare(source_path, theme)?.typeset()?;
    let pdf = document::compile::to_pdf(&typeset.document)?;
    let bytes = pdf.len();

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }

    std::fs::write(output_path, pdf)
        .with_context(|| format!("could not write {}", output_path.display()))?;

    Ok(Outcome {
        warnings: typeset.warnings,
        bytes,
    })
}
