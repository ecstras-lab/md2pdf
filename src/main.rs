//! Converts an Obsidian-flavoured Markdown file into a themed PDF.

mod cli;
mod convert;
mod document;
mod files;
mod markdown;
mod report;
mod theme;
mod tui;

use std::time::Instant;

use anyhow::Result;

use report::Failure;

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
    let outcome = convert::export(&source_path, &theme.build(), &output_path)?;
    let took = started.elapsed();

    // Quiet silences the report, never what went wrong. Typst's own warnings
    // are not marked in the PDF the way skipped embeds are, so hiding them
    // here would hide them entirely.
    if cli.quiet {
        for warning in &outcome.warnings {
            report::warning(warning);
        }

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
