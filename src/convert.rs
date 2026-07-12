//! Reading a note and turning it into a PDF on disk. Both front ends come
//! through here, so a file is never built two different ways.

use std::path::Path;

use anyhow::{Context, Result};

use crate::document;
use crate::markdown::{self, frontmatter};
use crate::theme::Theme;

/// What a finished export has to say for itself.
pub(crate) struct Exported {
    /// The size of the PDF.
    pub(crate) bytes: usize,
    /// Everything that had to be skipped. What the converter skipped and what
    /// Typst warned about are one list to a reader, so they arrive as one.
    pub(crate) warnings: Vec<String>,
}

/// Reads, converts and writes a note. The one write path, whichever front end
/// asked for it.
pub(crate) fn export(
    source_path: &Path,
    theme: &Theme,
    output_path: &Path,
) -> Result<Exported> {
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
    let compiled = document::compile::to_pdf(&source, files)?;
    warnings.extend(compiled.warnings);

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }

    std::fs::write(output_path, &compiled.pdf)
        .with_context(|| format!("could not write {}", output_path.display()))?;

    Ok(Exported {
        bytes: compiled.pdf.len(),
        warnings,
    })
}
