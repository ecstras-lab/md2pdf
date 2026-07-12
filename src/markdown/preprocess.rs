//! Rewrites Obsidian's embed syntaxes into plain CommonMark, before the
//! parser ever sees them.

use std::sync::LazyLock;

use regex::Regex;

/// `![["note"]]`, the quoted wikilink embed. Must run before [`WIKILINK`],
/// whose broader pattern would otherwise swallow the quotes.
static WIKILINK_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"!\[\["([^"]+)"\]\]"#).unwrap());

/// `![[note]]`, Obsidian's wikilink embed.
static WIKILINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[\[([^\]]+)\]\]").unwrap());

/// `!["alt"]("percent%20encoded")`, the quoted embed form.
static QUOTED_EMBED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"!\["([^"]+)"\]\("([^"]+)"\)"#).unwrap());

/// Fenced blocks, then inline spans. Order matters, the longest fence wins.
static CODE_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```.*?```|~~~.*?~~~|``.*?``|`[^`\n]*`").unwrap());

/// The marker used to park code while wikilinks are rewritten around it. It is
/// a private use character, and any copy already in the note is removed first,
/// so the marker really cannot collide with user text.
const CODE_SENTINEL: char = '\u{e000}';

/// Rewrites Obsidian's embed syntaxes into plain markdown images.
///
/// pulldown-cmark shatters `![[name]]` into loose bracket tokens, so this has
/// to happen on the source text. Code is parked first, because a fence
/// containing `![[..]]` must survive untouched.
pub(super) fn preprocess(markdown: &str) -> String {
    // A private use character carries no meaning in a note, and stripping any
    // stray copy up front is what lets the sentinel below be trusted.
    let markdown = markdown.replace(CODE_SENTINEL, "");

    let mut parked = Vec::new();

    let guarded = CODE_SPAN.replace_all(&markdown, |captures: &regex::Captures| {
        let matched = &captures[0];

        // CommonMark ends an inline span at a blank line. Parking a pair of
        // stray backtick runs from different paragraphs would hide everything
        // between them from the rewrite, so those are left for the parser.
        let fence = matched.starts_with("```") || matched.starts_with("~~~");
        let blank_line = matched.contains("\n\n") || matched.contains("\n\r\n");
        if !fence && blank_line {
            return matched.to_owned();
        }

        parked.push(matched.to_owned());
        format!("{CODE_SENTINEL}{}{CODE_SENTINEL}", parked.len() - 1)
    });

    // The quoted form runs first, or the general pattern would swallow its
    // quotes.
    let rewritten = QUOTED_EMBED.replace_all(&guarded, |captures: &regex::Captures| {
        let source = urlencoding::decode(&captures[2]).unwrap_or_else(|_| captures[2].into());
        format!("![{}]({})", alt(&captures[1]), destination(&source))
    });

    let rewritten = WIKILINK_QUOTED.replace_all(&rewritten, |captures: &regex::Captures| {
        let target = target(&captures[1]);
        format!("![{}]({})", alt(target), destination(target))
    });

    let rewritten = WIKILINK.replace_all(&rewritten, |captures: &regex::Captures| {
        let target = target(&captures[1]);
        let name = target.rsplit(['/', '\\']).next().unwrap_or(target);
        format!("![{}]({})", alt(name), destination(target))
    });

    restore(&rewritten, &parked)
}

/// Splices the parked code back in, in one pass over the text. Sentinels only
/// exist where the parking wrote them, so the pieces between them alternate
/// between document text and a parked index.
fn restore(
    text: &str,
    parked: &[String],
) -> String {
    let mut restored = String::with_capacity(text.len());

    for (position, piece) in text.split(CODE_SENTINEL).enumerate() {
        if position % 2 == 0 {
            restored.push_str(piece);
        } else if let Some(code) = piece.parse::<usize>().ok().and_then(|i| parked.get(i)) {
            restored.push_str(code);
        }
    }

    restored
}

/// The embed target without Obsidian's `|` suffix, which carries a display
/// alias or an image size and is never part of the path.
fn target(raw: &str) -> &str {
    raw.split('|').next().unwrap_or(raw).trim()
}

/// Escapes the characters that would end a CommonMark image's alt text early.
fn alt(text: &str) -> String {
    text.replace('\\', r"\\")
        .replace('[', r"\[")
        .replace(']', r"\]")
}

/// Wraps an image path as a CommonMark angle-bracket destination.
/// Obsidian attachments routinely contain spaces, which a bare destination
/// cannot hold. `![a](my file.png)` is not an image at all, just text.
fn destination(target: &str) -> String {
    let escaped = target
        .replace('\\', r"\\")
        .replace('<', r"\<")
        .replace('>', r"\>");

    format!("<{escaped}>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wikilinks_become_markdown_images() {
        assert_eq!(preprocess("![[a/b/c.png]]"), "![c.png](<a/b/c.png>)");
        assert_eq!(preprocess(r#"![["file.png"]]"#), "![file.png](<file.png>)");
        assert_eq!(preprocess(r#"!["alt"]("a%20b.png")"#), "![alt](<a b.png>)");
    }

    #[test]
    fn wikilinks_inside_code_are_left_alone() {
        let source = "```\n![[keep.png]]\n```\n";
        assert_eq!(preprocess(source), source);
        assert_eq!(preprocess("`![[keep.png]]`"), "`![[keep.png]]`");
    }

    /// `![[img.png|300]]` sizes the embed in Obsidian. The suffix is display
    /// advice, never part of the path.
    #[test]
    fn size_and_alias_suffixes_stay_out_of_the_path() {
        assert_eq!(preprocess("![[img.png|300]]"), "![img.png](<img.png>)");
        assert_eq!(preprocess("![[img.png|300x200]]"), "![img.png](<img.png>)");
        assert_eq!(preprocess("![[Note|nice name]]"), "![Note](<Note>)");
    }

    /// A bracket in the alt would end the image early and shatter the embed.
    #[test]
    fn brackets_in_alt_text_are_escaped() {
        assert_eq!(
            preprocess(r#"!["a]b"]("x.png")"#),
            r"![a\]b](<x.png>)".to_owned()
        );
    }

    /// Two stray double-backtick runs in different paragraphs are not a code
    /// span, and must not hide the embed between them from the rewrite.
    #[test]
    fn spans_do_not_pair_across_blank_lines() {
        let source = "a `` b\n\n![[img.png]]\n\nc `` d";

        assert!(preprocess(source).contains("![img.png](<img.png>)"));
    }

    /// The sentinel is a private use character. A stray copy in the note is
    /// stripped rather than left to collide with a parked span.
    #[test]
    fn a_literal_sentinel_in_the_note_cannot_impersonate_parked_code() {
        let source = "`code`\n\n\u{e000}0\u{e000}";

        assert_eq!(preprocess(source), "`code`\n\n0");
    }
}
