//! Converts an Obsidian-flavoured Markdown file into a themed PDF.

mod cli;
mod document;
mod markdown;
mod report;
mod theme;

use std::path::Path;

use anyhow::{Context, Result};
use clap::Parser;

use cli::Cli;
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
    let cli = Cli::parse();
    let source_path = cli::with_markdown_extension(cli.file.clone());

    if !source_path.is_file() {
        return Err(cli::missing_source(&source_path));
    }

    let output_path = match cli.output {
        Some(path) => path,
        None => cli::default_output_path(&source_path)?,
    };

    let warnings = convert(&source_path, &output_path, &cli.theme.build())?;

    if cli.verbose {
        println!("theme:  {}", cli.theme.label());
        println!("source: {}", source_path.display());

        for warning in &warnings {
            println!("skipped: {warning}");
        }

        println!("output: {}", output_path.display());
    } else if !warnings.is_empty() {
        let plural = if warnings.len() == 1 { "" } else { "s" };
        let verb = if warnings.len() == 1 { "is" } else { "are" };

        report::note(&format!(
            "{} embed{plural} could not be drawn, and {verb} marked in the PDF. \
             Run with --verbose to list them.",
            warnings.len(),
        ));
    }

    Ok(())
}

/// Converts the file and returns everything that had to be skipped.
fn convert(
    source_path: &Path,
    output_path: &Path,
    theme: &Theme,
) -> Result<Vec<String>> {
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

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }

    std::fs::write(output_path, pdf)
        .with_context(|| format!("could not write {}", output_path.display()))?;

    Ok(warnings)
}
