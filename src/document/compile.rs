//! Compiles the generated Typst source into a PDF.

use std::sync::LazyLock;

use anyhow::{Result, anyhow};
use typst::foundations::Bytes;
use typst::text::Font;
use typst_as_lib::{TypstAsLibError, TypstEngine};

/// Enough of a typesetting failure to diagnose it, without a wall of text.
const MAX_DIAGNOSTICS: usize = 5;

static FONT_DATA: [&[u8]; 11] = [
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

/// The embedded faces, parsed once. Handing the engine raw bytes would make
/// it copy and re-parse all eleven files on every compile, where a `Font` is
/// shared and clones for free.
static FONTS: LazyLock<Vec<Font>> = LazyLock::new(|| {
    FONT_DATA
        .iter()
        .flat_map(|data| Font::iter(Bytes::new(*data)))
        .collect()
});

/// A finished PDF, alongside anything Typst had to say while typesetting it.
pub struct Compiled {
    pub pdf: Vec<u8>,
    pub warnings: Vec<String>,
}

/// Renders Typst source to PDF bytes. `files` are read by virtual path from
/// within the source, meaning the syntax theme, the icons and any embedded
/// images.
/// They are taken by value, so embedded images move into the engine instead
/// of being copied.
pub fn to_pdf(
    source: &str,
    mut files: Vec<(String, Vec<u8>)>,
) -> Result<Compiled> {
    let resolved = files
        .iter_mut()
        .map(|(path, bytes)| (path.as_str(), std::mem::take(bytes)));

    let engine = TypstEngine::builder()
        .main_file(source)
        .fonts(FONTS.iter().cloned())
        .with_static_file_resolver(resolved)
        .build();

    let compiled = engine.compile();

    let warnings: Vec<String> = compiled
        .warnings
        .iter()
        .map(|warning| warning.message.to_string())
        .collect();

    // A failure here is a defect in the stylesheet, not in the user's note, so
    // report the diagnostics plainly instead of dumping the error struct. The
    // warnings ride along, since they are often the clue to the failure.
    let document = match compiled.output {
        Ok(document) => document,
        Err(error) => {
            let mut description = describe(&error);

            for warning in &warnings {
                description.push_str("\nwarning: ");
                description.push_str(warning);
            }

            return Err(anyhow!("the document could not be typeset\n{description}"));
        }
    };

    let options = typst_pdf::PdfOptions::default();

    let pdf = typst_pdf::pdf(&document, &options).map_err(|diagnostics| {
        let messages = diagnostics
            .iter()
            .map(|entry| entry.message.to_string())
            .collect();

        anyhow!("the PDF could not be written\n{}", capped(messages))
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

    if diagnostics.len() > MAX_DIAGNOSTICS {
        lines.push(format!(
            "... and {} more",
            diagnostics.len() - MAX_DIAGNOSTICS
        ));
    }

    lines.join("\n")
}

/// Caps a list of diagnostic lines the way [`describe`] does, so no error
/// path prints the wall of text `MAX_DIAGNOSTICS` exists to prevent.
fn capped(mut lines: Vec<String>) -> String {
    if lines.len() > MAX_DIAGNOSTICS {
        let hidden = lines.len() - MAX_DIAGNOSTICS;
        lines.truncate(MAX_DIAGNOSTICS);
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
        to_pdf(source, Vec::new()).is_ok()
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
