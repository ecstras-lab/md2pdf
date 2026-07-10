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

/// Fenced blocks, then inline spans. Order matters: the longest fence wins.
static CODE_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```.*?```|~~~.*?~~~|``.*?``|`[^`\n]*`").unwrap());

/// A sentinel that cannot occur in markdown, used to park code while
/// wikilinks are rewritten around it.
const CODE_SENTINEL: char = '\u{e000}';

/// Rewrites Obsidian's embed syntaxes into plain markdown images.
///
/// pulldown-cmark shatters `![[name]]` into loose bracket tokens, so this has
/// to happen on the source text. Code is parked first: a fence containing
/// `![[..]]` must survive untouched.
pub(super) fn preprocess(markdown: &str) -> String {
    let mut parked = Vec::new();

    let guarded = CODE_SPAN.replace_all(markdown, |captures: &regex::Captures| {
        parked.push(captures[0].to_owned());
        format!("{CODE_SENTINEL}{}{CODE_SENTINEL}", parked.len() - 1)
    });

    // The quoted form runs first; the general pattern would swallow its quotes.
    let rewritten = QUOTED_EMBED.replace_all(&guarded, |captures: &regex::Captures| {
        let source = urlencoding::decode(&captures[2]).unwrap_or_else(|_| captures[2].into());
        format!("![{}]({})", &captures[1], destination(&source))
    });

    let rewritten = WIKILINK_QUOTED.replace_all(&rewritten, |captures: &regex::Captures| {
        format!("![{}]({})", &captures[1], destination(&captures[1]))
    });

    let rewritten = WIKILINK.replace_all(&rewritten, |captures: &regex::Captures| {
        let target = &captures[1];
        let name = target.rsplit(['/', '\\']).next().unwrap_or(target);
        format!("![{name}]({})", destination(target))
    });

    let mut restored = rewritten.into_owned();
    for (index, code) in parked.iter().enumerate() {
        restored = restored.replace(&format!("{CODE_SENTINEL}{index}{CODE_SENTINEL}"), code);
    }

    restored
}

/// Wraps an image path as a CommonMark angle-bracket destination.
/// Obsidian attachments routinely contain spaces, which a bare destination
/// cannot hold: `![a](my file.png)` is not an image at all, just text.
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
}
