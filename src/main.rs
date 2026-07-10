//! Converts an Obsidian-flavoured Markdown file into a themed PDF.

mod cli;
mod document;
mod markdown;
mod report;
mod theme;

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};

use markdown::frontmatter;
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
    let source_path = cli::with_markdown_extension(cli.file.clone());

    if !source_path.is_file() {
        return Err(cli::missing_source(&source_path));
    }

    let output_path = match cli.output {
        Some(path) => path,
        None => cli::default_output_path(&source_path)?,
    };

    let started = Instant::now();
    let outcome = convert(&source_path, &output_path, &cli.theme.build())?;
    let took = started.elapsed();

    if cli.quiet {
        return Ok(());
    }

    report::line("theme", cli.theme.label());
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

fn convert(
    source_path: &Path,
    output_path: &Path,
    theme: &Theme,
) -> Result<Outcome> {
    let markdown = std::fs::read_to_string(source_path)
        .with_context(|| format!("could not read {}", source_path.display()))?;

    let base_dir = source_path.parent().unwrap_or(Path::new("."));

    let parsed = frontmatter::split(&markdown);
    let rendered = markdown::render(&parsed.body, base_dir, &parsed.properties);

    let mut warnings = parsed.warnings;
    warnings.extend(rendered.warnings);

    let mut files = document::assets(theme);
    files.extend(rendered.files);

    let source = document::source(theme, &rendered.body);

    let pdf = document::compile::to_pdf(&source, &files)?;
    let bytes = pdf.len();

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }

    std::fs::write(output_path, pdf)
        .with_context(|| format!("could not write {}", output_path.display()))?;

    Ok(Outcome { warnings, bytes })
}
