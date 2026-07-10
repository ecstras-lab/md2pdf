//! Compiles the generated Typst source, then exports it as a PDF or as pixels.

use anyhow::{Context, Result, anyhow};
use image::{Rgba, RgbaImage};
use typst_as_lib::{TypstAsLibError, TypstEngine};
use typst_layout::PagedDocument;

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

/// A laid out document, alongside anything Typst had to say while laying it out.
pub struct Typeset {
    pub document: PagedDocument,
    pub warnings: Vec<String>,
}

/// Lays out Typst source. `files` are read by virtual path from within the
/// source: the syntax theme, the icons and any embedded images.
pub fn typeset(
    source: &str,
    files: &[(String, Vec<u8>)],
) -> Result<Typeset> {
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

    Ok(Typeset { document, warnings })
}

/// Exports the document as PDF bytes.
pub fn to_pdf(document: &PagedDocument) -> Result<Vec<u8>> {
    let options = typst_pdf::PdfOptions::default();

    typst_pdf::pdf(document, &options).map_err(|diagnostics| {
        let messages: Vec<String> = diagnostics
            .iter()
            .map(|entry| entry.message.to_string())
            .collect();

        anyhow!("the PDF could not be written\n{}", messages.join("\n"))
    })
}

/// Draws the document at `pixels_per_point`. There is only ever one page, and
/// it is as tall as the note is long, so the caller gets one very tall image.
pub fn to_image(
    document: &PagedDocument,
    pixels_per_point: f64,
) -> Result<RgbaImage> {
    let page = document
        .pages()
        .first()
        .context("the document came out empty")?;

    let options = typst_render::RenderOptions {
        pixel_per_pt: pixels_per_point.into(),
        render_bleed: false,
    };

    let pixmap = typst_render::render(page, &options);
    let mut image = RgbaImage::new(pixmap.width(), pixmap.height());

    // tiny-skia holds each channel premultiplied by its alpha. `image` does not.
    for (target, source) in image.pixels_mut().zip(pixmap.pixels()) {
        let color = source.demultiply();

        *target = Rgba([color.red(), color.green(), color.blue(), color.alpha()]);
    }

    Ok(image)
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
        typeset(source, &[])
            .and_then(|typeset| to_pdf(&typeset.document))
            .is_ok()
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

    /// The preview scales the page by asking for pixels per point, so a page
    /// 200pt wide at two of them is 400 pixels across.
    #[test]
    fn a_page_is_drawn_at_the_scale_it_is_asked_for() {
        let typeset = typeset(&page("Hi"), &[]).unwrap();

        let image = to_image(&typeset.document, 2.0).unwrap();

        assert_eq!(image.width(), 400);
        assert!(image.height() > 0);
    }

    /// The page is opaque, and a preview drawn over a terminal cell has to be,
    /// because neither halfblocks nor sixels carry an alpha channel.
    #[test]
    fn a_drawn_page_is_opaque() {
        let typeset = typeset(&page("#set page(fill: white)\nHi"), &[]).unwrap();

        let image = to_image(&typeset.document, 1.0).unwrap();

        assert!(image.pixels().all(|pixel| pixel.0[3] == 0xff));
    }
}
