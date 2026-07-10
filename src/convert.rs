//! Reading a note and turning it into a PDF. Both the command and the
//! interface come through here, so a file is never built two different ways.

use std::path::Path;

use anyhow::{Context, Result};

use crate::document;
use crate::markdown::{self, frontmatter};
use crate::theme::Theme;

/// A note, parsed and dressed in a theme, ready to be typeset.
pub(crate) struct Prepared {
    source: String,
    files: Vec<(String, Vec<u8>)>,
    warnings: Vec<String>,
}

pub(crate) fn prepare(
    source_path: &Path,
    theme: &Theme,
) -> Result<Prepared> {
    let markdown = std::fs::read_to_string(source_path)
        .with_context(|| format!("could not read {}", source_path.display()))?;

    let base_dir = source_path.parent().unwrap_or(Path::new("."));

    let parsed = frontmatter::split(&markdown);
    let rendered = markdown::render(&parsed.body, base_dir, &parsed.properties);

    let mut warnings = parsed.warnings;
    warnings.extend(rendered.warnings);

    let mut files = document::assets(theme);
    files.extend(rendered.files);

    Ok(Prepared {
        source: document::source(theme, &rendered.body),
        files,
        warnings,
    })
}

/// The PDF, and everything that had to be skipped to make it. What the
/// converter skipped and what Typst warned about are one list to a reader,
/// so they arrive as one.
pub(crate) struct Rendered {
    pub(crate) pdf: Vec<u8>,
    pub(crate) warnings: Vec<String>,
}

impl Prepared {
    pub(crate) fn render(self) -> Result<Rendered> {
        let compiled = document::compile::to_pdf(&self.source, &self.files)?;

        let mut warnings = self.warnings;
        warnings.extend(compiled.warnings);

        Ok(Rendered {
            pdf: compiled.pdf,
            warnings,
        })
    }
}
