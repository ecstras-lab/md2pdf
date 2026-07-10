//! Reading a note and turning it into a document, once for the file on disk
//! and once for the preview beside it.

use std::path::Path;

use anyhow::{Context, Result};

use crate::document::{self, compile::Typeset};
use crate::markdown::{self, frontmatter};
use crate::theme::Theme;

/// A note, parsed and dressed in a theme, waiting to be laid out.
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

impl Prepared {
    /// Lays the note out. Whatever the converter had to skip is carried through
    /// alongside whatever Typst had to say, because to a reader they are one
    /// list of things that did not go to plan.
    pub(crate) fn typeset(self) -> Result<Typeset> {
        let mut typeset = document::compile::typeset(&self.source, &self.files)?;

        let mut warnings = self.warnings;
        warnings.extend(typeset.warnings);
        typeset.warnings = warnings;

        Ok(typeset)
    }
}
