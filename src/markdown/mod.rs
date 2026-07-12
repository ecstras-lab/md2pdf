//! Converts markdown into the Typst markup consumed by `assets/theme.typ`.
//!
//! The note is rewritten by [`preprocess`], parsed by `pulldown-cmark`, then
//! walked once by [`renderer::Renderer`]. The body it emits never styles
//! anything itself. It only calls the helpers the stylesheet defines.

pub mod frontmatter;

mod images;
mod inline;
mod literal;
mod preprocess;
mod properties;
mod renderer;

use std::path::Path;

use pulldown_cmark::{Options, Parser};

use frontmatter::Property;
use preprocess::preprocess;
use renderer::Renderer;

/// Uppercases the first letter, for labels and titles derived from lowercase
/// keys. Shared by the properties block and the callout titles.
fn capitalize(text: &str) -> String {
    let mut characters = text.chars();

    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

pub struct Rendered {
    pub body: String,
    /// Virtual path to bytes, registered with the Typst engine.
    pub files: Vec<(String, Vec<u8>)>,
    pub warnings: Vec<String>,
}

pub fn render(
    markdown: &str,
    base_dir: &Path,
    properties: &[Property],
) -> Rendered {
    let prepared = preprocess(markdown);
    let events = Parser::new_ext(&prepared, extensions()).collect();

    Renderer::new(events, base_dir, properties).run()
}

/// GFM alerts stay off. They know five callout kinds, Obsidian has 27, and the
/// parser leaves the marker as text for every kind it does not recognise.
fn extensions() -> Options {
    let mut options = Options::empty();

    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_MATH);

    options
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    use super::frontmatter::PropertyValue;
    use super::literal::literal;
    use super::preprocess::preprocess;
    use super::*;

    fn body(markdown: &str) -> String {
        render(markdown, Path::new("."), &[]).body
    }

    /// Obsidian attachments carry spaces. A bare destination would make
    /// `![a](my file.png)` parse as text, never as an image.
    #[test]
    fn attachment_names_with_spaces_still_parse_as_images() {
        assert_eq!(
            preprocess("![[Pasted image 20260326170345.png]]"),
            "![Pasted image 20260326170345.png](<Pasted image 20260326170345.png>)",
        );

        let rendered = render("![[Pasted image 1.png]]\n", Path::new("."), &[]);
        assert!(
            rendered
                .warnings
                .iter()
                .any(|w| w.contains("image not found")),
            "a spaced attachment must reach the image resolver, got {:?}",
            rendered.warnings,
        );
    }

    /// The load-bearing safety property: nothing from the source text can land
    /// in the output as live Typst syntax. Strip every `#("..")` literal and
    /// no metacharacter may remain.
    #[test]
    fn typst_metacharacters_stay_inside_string_literals() {
        // Escaped in markdown, so these reach the renderer as ordinary text.
        // `C#` is deliberate: a hash that starts no tag must stay literal.
        let out = body(r"a \$dollar\$ and \[bracket\] and \\ slash and C\# sharp");

        let literals = Regex::new(r#"#\("(?:[^"\\]|\\.)*"\)"#).unwrap();
        let outside = literals.replace_all(&out, "");

        for metacharacter in ['$', '[', ']', '\\', '#'] {
            assert!(
                !outside.contains(metacharacter),
                "a bare `{metacharacter}` escaped into Typst markup: {outside:?}",
            );
        }

        assert!(out.contains("dollar") && out.contains("bracket") && out.contains("slash"));
        assert!(!out.contains("#math-block"));
    }

    #[test]
    fn currency_is_not_treated_as_math() {
        let out = body("Should not be latex: $10 to $20\n");
        assert!(
            out.contains(r#"#("$")"#),
            "the dollar sign should be literal text"
        );
        assert!(!out.contains("math-block"));
    }

    #[test]
    fn code_fences_keep_obsidian_syntax_verbatim() {
        let out = body("```\n==no==\n%%no%%\n$no$\n```\n");
        assert!(!out.contains("doc-highlight"));
        assert!(!out.contains("doc-comment"));
        assert!(out.contains(r#"raw(block: true, "==no==\n%%no%%\n$no$")"#));
    }

    #[test]
    fn highlights_and_comments_become_helpers() {
        assert!(body("==hot==\n").contains(r#"#doc-highlight[#("hot")]"#));
        assert!(body("%% quiet %%\n").contains(r#"#doc-comment[#(" quiet ")]"#));
    }

    #[test]
    fn an_unpaired_delimiter_stays_literal() {
        let out = body("a == b\n");
        assert!(!out.contains("doc-highlight"));
        assert!(out.contains("=="));
    }

    #[test]
    fn hashtags_become_pills_but_headings_do_not() {
        assert!(body("#tag/miaow\n").contains(r#"#doc-tag("tag/miaow")"#));
        assert!(!body("C# is a language\n").contains("doc-tag"));
        assert!(!body("\\# Not a heading\n").contains("doc-tag"));
    }

    #[test]
    fn callout_markers_are_parsed_across_split_text_events() {
        assert!(body("> [!note]\n> body\n").contains(r#"#callout("note", "Note")"#));
        assert!(body("> plain\n").contains("#note-quote["));
    }

    #[test]
    fn a_callout_with_only_a_title_carries_no_body() {
        assert!(body("> [!QUOTE] A title\n").contains(r#"#callout("quote", "A title", none)"#));
    }

    /// The byte range of the callout's content block, found by matching
    /// brackets. Safe here because the fixture holds no bracket in its text.
    fn callout_content_span(out: &str) -> (usize, usize) {
        let open = out.find("#callout(").expect("no callout emitted");
        let start = open + out[open..].find('[').expect("no content block");

        let mut depth = 0usize;
        for (offset, character) in out[start..].char_indices() {
            match character {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return (start, start + offset);
                    }
                }
                _ => {}
            }
        }

        panic!("the callout content block is never closed");
    }

    /// Everything after the marker's paragraph belongs to the callout, not to
    /// the document. The marker is consumed mid-paragraph, so the blockquote
    /// has to be drained in two passes.
    #[test]
    fn a_callout_keeps_every_block_that_follows_its_marker() {
        let out = body(
            "> [!note]\n> **Important:**\n>\n> - Item 1\n>\n> ```js\n> log();\n> ```\n> > nested\n",
        );
        let (start, end) = callout_content_span(&out);

        for fragment in ["#strong[", "#list(", "#code-block(", "#note-quote["] {
            let at = out
                .find(fragment)
                .unwrap_or_else(|| panic!("{fragment} missing"));
            assert!(start < at && at < end, "{fragment} escaped the callout");
        }
    }

    #[test]
    fn an_unknown_language_still_labels_the_block() {
        let out = body("```wat\ncode\n```\n");
        assert!(out.contains(r#"#code-block("wat", raw(block: true, "code"))"#));
    }

    #[test]
    fn task_lists_render_as_checkboxes() {
        let out = body("- [ ] todo\n- [x] done\n");
        assert!(out.contains("#task-list(true, ((false, ["));
        assert!(out.contains("(true, ["));
    }

    /// A loose task list wraps each item in a paragraph, with the marker
    /// inside it. The checkboxes must survive the wrapping.
    #[test]
    fn loose_task_lists_keep_their_checkboxes() {
        let out = body("- [ ] todo\n\n- [x] done\n");

        assert!(out.contains("#task-list("), "no task list emitted: {out}");
        assert!(out.contains("(false, ["));
        assert!(out.contains("(true, ["));
    }

    /// `[!note]-` folds the callout in Obsidian. A PDF has nothing to fold,
    /// and the sign must not leak into the title.
    #[test]
    fn foldable_callouts_keep_the_sign_out_of_the_title() {
        assert!(
            body("> [!note]- Collapsed\n> body\n").contains(r#"#callout("note", "Collapsed")"#)
        );
        assert!(body("> [!tip]+\n> body\n").contains(r#"#callout("tip", "Tip")"#));
    }

    /// Inline markup nests inside an image's alt. The capture must survive it
    /// whole, leaking nothing into the body.
    #[test]
    fn image_alt_text_survives_inline_markup() {
        let out = render(
            "![an *italic* caption](<Pasted image 20260326170345.png>)\n",
            Path::new("tests"),
            &[],
        )
        .body;

        assert!(
            out.contains(r#""an italic caption""#),
            "the alt was truncated: {out}",
        );
        assert!(
            !out.contains(r#"#(" caption")"#),
            "alt text leaked into the body: {out}",
        );
    }

    /// Obsidian requires a tag to carry at least one non-digit, so an issue
    /// reference stays plain text.
    #[test]
    fn numeric_references_are_not_tags() {
        assert!(!body("fixed #42 today\n").contains("doc-tag"));
        assert!(body("release #v2 today\n").contains(r#"#doc-tag("v2")"#));
    }

    #[test]
    fn ordered_lists_keep_their_start_number() {
        assert!(body("3. three\n4. four\n").contains("#enum(tight: true, start: 3,"));
    }

    #[test]
    fn footnotes_are_numbered_by_reference_order() {
        let out = body("Text[^b] more[^a]\n\n[^a]: first\n[^b]: second\n");

        assert!(
            out.contains("#fn-ref(1, true)"),
            "the first reference is footnote 1"
        );
        assert!(out.contains("#fn-ref(2, true)"));
        assert!(out.contains("(number: 1, backref: true, body: [#(\"second\")"));
    }

    #[test]
    fn a_repeated_reference_anchors_only_once() {
        let out = body("a[^x] b[^x]\n\n[^x]: note\n");

        assert!(out.contains("#fn-ref(1, true)"));
        assert!(
            out.contains("#fn-ref(1, false)"),
            "the second reference must not re-label"
        );
    }

    /// An undefined reference must never emit `fn-ref`, which would link to a
    /// Typst label that was never placed and fail the compile.
    #[test]
    fn a_reference_without_a_definition_never_emits_a_link() {
        let rendered = render("dangling[^gone]\n", Path::new("."), &[]);

        assert!(!rendered.body.contains("#fn-ref"));
        assert!(!rendered.body.contains("#footnotes-section"));
        assert!(
            rendered.body.contains("^gone"),
            "the text should survive verbatim"
        );
    }

    #[test]
    fn an_unreferenced_definition_still_appears_without_a_backlink() {
        let out = body("text\n\n[^loose]: orphan\n");
        assert!(out.contains("(number: 1, backref: false,"));
    }

    /// An HTML block is one opaque event, so all of it goes. Inline tags are
    /// separate events from the words they wrap, so only the tags go.
    #[test]
    fn raw_html_is_dropped_but_inline_text_survives() {
        let out = body("<div style=\"color: red\">gone</div>\n\ntext <b>bold</b>\n");

        assert!(
            !out.contains("div"),
            "the html block should vanish entirely"
        );
        assert!(
            !out.contains("gone"),
            "text inside an html block goes with it"
        );
        assert!(!out.contains("<b>"));
        assert!(out.contains(r#"#("text ")"#));
        assert!(
            out.contains(r#"#("bold")"#),
            "words between inline tags are real text"
        );
    }

    #[test]
    fn a_missing_image_leaves_a_placeholder_rather_than_a_broken_reference() {
        let rendered = render("![[no-such-file.png]]\n", Path::new("."), &[]);

        assert!(
            rendered
                .body
                .contains(r#"#doc-missing("image not found: no-such-file.png")"#)
        );
        assert!(
            rendered
                .warnings
                .iter()
                .any(|w| w.contains("image not found"))
        );
        assert!(rendered.files.is_empty());
    }

    /// Every embed the converter turns down says so in the document, and says
    /// why. Silence would read as "there was nothing here".
    #[test]
    fn an_unrenderable_embed_names_its_reason_in_the_document() {
        let cases = [
            (
                "![[clip.mp4]]\n",
                "video embeds are not supported: clip.mp4",
            ),
            (
                "![[song.mp3]]\n",
                "audio embeds are not supported: song.mp3",
            ),
            (
                "![[Another Note]]\n",
                "note embeds are not supported: Another Note",
            ),
            (
                "![[sheet.xlsx]]\n",
                ".xlsx is not an embeddable image: sheet.xlsx",
            ),
            (
                "![a](https://x.test/a.png)\n",
                "remote image not embedded: https://x.test/a.png",
            ),
        ];

        for (source, reason) in cases {
            let rendered = render(source, Path::new("."), &[]);

            assert!(
                rendered
                    .body
                    .contains(&format!("#doc-missing({})", literal(reason))),
                "{source} produced {:?}",
                rendered.body,
            );
            assert_eq!(rendered.warnings, vec![reason.to_owned()]);
        }
    }

    #[test]
    fn math_is_converted_to_typst() {
        assert!(body("Inline: $E = mc^2$\n").contains("$E = m c^2$"));
        assert!(
            body("$$\n\\int_0^1 x^2 dx\n$$\n").contains("#math-block($ integral_0^1 x^2 d x $)"),
            "the LaTeX integral should become Typst's `integral`",
        );
    }

    #[test]
    fn unconvertible_math_falls_back_to_literal_text() {
        let out = body("$\\undefinedmacro{x}$\n");
        assert!(
            !out.contains("$\\"),
            "raw LaTeX must not reach Typst as math"
        );
    }

    #[test]
    fn properties_land_after_the_first_heading() {
        let properties = [Property {
            label: "Title".to_owned(),
            value: PropertyValue::Text("T".to_owned()),
        }];

        let out = render("# Head\n\nbody\n", Path::new("."), &properties).body;
        let heading = out.find("#heading(level: 1)").unwrap();
        let block = out.find("#properties-block").unwrap();
        let text = out.find(r#"#("body")"#).unwrap();

        assert!(heading < block && block < text);
    }

    #[test]
    fn properties_lead_a_note_that_has_no_heading() {
        let properties = [Property {
            label: "Title".to_owned(),
            value: PropertyValue::Text("T".to_owned()),
        }];

        let out = render("body\n", Path::new("."), &properties).body;
        assert!(out.starts_with("#properties-block"));
    }
}
