//! Compiles the generated Typst source into a PDF.

use anyhow::{Result, anyhow};
use typst_as_lib::{TypstAsLibError, TypstEngine};

/// Enough of a typesetting failure to diagnose it, without a wall of text.
const MAX_DIAGNOSTICS: usize = 5;

static FONTS: [&[u8]; 11] = [
    include_bytes!("../../assets/fonts/Montserrat-Regular.ttf"),
    include_bytes!("../../assets/fonts/Montserrat-Italic.ttf"),
    include_bytes!("../../assets/fonts/Montserrat-Medium.ttf"),
    include_bytes!("../../assets/fonts/Montserrat-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/Montserrat-Bold.ttf"),
    include_bytes!("../../assets/fonts/Montserrat-BoldItalic.ttf"),
    include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/JetBrainsMono-Bold.ttf"),
    include_bytes!("../../assets/fonts/JetBrainsMono-Italic.ttf"),
    // Backs Montserrat for the ornaments and list markers it does not carry.
    include_bytes!("../../assets/fonts/DejaVuSans.ttf"),
    // Typst lays equations out from a font's OpenType MATH table, which none
    // of the text faces above carry.
    include_bytes!("../../assets/fonts/NewCMMath-Book.otf"),
];

/// A finished PDF, alongside anything Typst had to say while typesetting it.
pub struct Compiled {
    pub pdf: Vec<u8>,
    pub warnings: Vec<String>,
}

/// Renders Typst source to PDF bytes. `files` are read by virtual path from
/// within the source: the syntax theme, the icons and any embedded images.
pub fn to_pdf(
    source: &str,
    files: &[(String, Vec<u8>)],
) -> Result<Compiled> {
    let resolved: Vec<(&str, Vec<u8>)> = files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.clone()))
        .collect();

    let engine = TypstEngine::builder()
        .main_file(source)
        .fonts(FONTS)
        .with_static_file_resolver(resolved)
        .build();

    let compiled = engine.compile();

    let warnings = compiled
        .warnings
        .iter()
        .map(|warning| warning.message.to_string())
        .collect();

    // A failure here is a defect in the stylesheet, not in the user's note, so
    // report the diagnostics plainly instead of dumping the error struct.
    let document = compiled
        .output
        .map_err(|error| anyhow!("the document could not be typeset\n{}", describe(&error)))?;

    let options = typst_pdf::PdfOptions::default();

    let pdf = typst_pdf::pdf(&document, &options).map_err(|diagnostics| {
        let messages: Vec<String> = diagnostics
            .iter()
            .map(|entry| entry.message.to_string())
            .collect();

        anyhow!("the PDF could not be written\n{}", messages.join("\n"))
    })?;

    Ok(Compiled { pdf, warnings })
}

/// `TypstAsLibError` renders its diagnostics with `{:?}`, which puts the whole
/// struct on the terminal. Only the messages and their hints are worth reading.
/// The caller lays them out, so they come back as plain lines.
fn describe(error: &TypstAsLibError) -> String {
    let TypstAsLibError::TypstSource(diagnostics) = error else {
        return error.to_string();
    };

    let mut lines = Vec::new();

    for diagnostic in diagnostics.iter().take(MAX_DIAGNOSTICS) {
        lines.push(diagnostic.message.to_string());

        for hint in &diagnostic.hints {
            lines.push(format!("hint: {}", hint.v));
        }
    }

    if let Some(hidden) = diagnostics
        .len()
        .checked_sub(MAX_DIAGNOSTICS)
        .filter(|n| *n > 0)
    {
        lines.push(format!("... and {hidden} more"));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(body: &str) -> String {
        format!("#set page(width: 200pt, height: auto)\n{body}")
    }

    /// Every test here asks the same question, which is whether the source
    /// reaches a PDF at all.
    fn exports(source: &str) -> bool {
        to_pdf(source, &[]).is_ok()
    }

    #[test]
    fn every_family_the_stylesheet_names_is_embedded() {
        for family in ["Montserrat", "JetBrains Mono", "DejaVu Sans"] {
            let source = page(&format!("#set text(font: \"{family}\")\nHi"));

            assert!(exports(&source), "{family} is missing");
        }
    }

    /// Typst lays equations out from an OpenType MATH table. Without a math
    /// font it fails with a bare "no font could be found".
    #[test]
    fn equations_find_a_math_font() {
        let source = page("#set text(font: \"Montserrat\")\n$ integral_0^1 x^2 d x $");

        assert!(exports(&source));
    }

    #[test]
    fn the_ornaments_montserrat_lacks_resolve_through_the_fallback() {
        let source = page("#set text(font: (\"Montserrat\", \"DejaVu Sans\"))\n✦ ⚙ ↩ ◦ ▪");

        assert!(exports(&source));
    }

    #[test]
    fn bold_and_italic_select_real_faces() {
        let source = page("#set text(font: \"Montserrat\")\n*Bold* _Italic_ *_Both_*");

        assert!(exports(&source));
    }
}
