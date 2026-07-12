//! Obsidian's inline spans, and the fenced blocks that carry code or math.

use std::sync::LazyLock;

use pulldown_cmark::HeadingLevel;
use regex::Regex;
use two_face::re_exports::syntect::parsing::SyntaxSet;

use super::literal::{literal, push_literal};

/// The same syntax set Typst highlights with, so a tag that resolves here
/// is guaranteed to highlight in the compiled document. That guarantee only
/// holds while Cargo.toml pins two-face to the version inside typst-library,
/// since a newer set would resolve languages Typst cannot highlight.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

/// A hashtag, anchored to a word boundary the way the stylesheet's rule was.
/// Obsidian requires at least one character that is not a digit, so an issue
/// reference like `#42` stays plain text.
static HASHTAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|\s)#([A-Za-z0-9_/-]*[A-Za-z_/-][A-Za-z0-9_/-]*)").unwrap());

/// Expands Obsidian's `==highlight==` and `%%comment%%` spans, then hashtags.
/// Code never reaches here, since it arrives as its own event.
pub(super) fn write_inline(
    out: &mut String,
    text: &str,
) {
    let mut rest = text;

    while !rest.is_empty() {
        let opener = ["==", "%%"]
            .into_iter()
            .filter_map(|delimiter| rest.find(delimiter).map(|at| (at, delimiter)))
            .min();

        let Some((at, delimiter)) = opener else {
            write_hashtags(out, rest);
            return;
        };

        let after = at + delimiter.len();

        let Some(close) = rest[after..].find(delimiter) else {
            // An unpaired delimiter is literal text.
            write_hashtags(out, &rest[..after]);
            rest = &rest[after..];
            continue;
        };

        write_hashtags(out, &rest[..at]);

        let mut inner = String::new();
        write_hashtags(&mut inner, &rest[after..after + close]);

        match delimiter {
            "==" => out.push_str(&format!("#doc-highlight[{inner}]")),
            _ => out.push_str(&format!("#doc-comment[{inner}]")),
        }

        rest = &rest[after + close + delimiter.len()..];
    }
}

fn write_hashtags(
    out: &mut String,
    text: &str,
) {
    let mut last = 0;

    for captures in HASHTAG.captures_iter(text) {
        let whole = captures.get(0).unwrap();
        let lead = captures.get(1).unwrap().as_str();
        let name = captures.get(2).unwrap().as_str();

        push_literal(out, &text[last..whole.start()]);
        push_literal(out, lead);
        out.push_str(&format!("#doc-tag({})", literal(name)));

        last = whole.end();
    }

    push_literal(out, &text[last..]);
}

/// Resolves a fence's language the way highlight.js did. Exact match first,
/// then the part before the first hyphen, so `python-repl` becomes `python`.
fn resolve_language(tag: &str) -> Option<String> {
    if tag.is_empty() {
        return None;
    }

    if SYNTAXES.find_syntax_by_token(tag).is_some() {
        return Some(tag.to_owned());
    }

    let base = tag.split('-').next()?;

    if base != tag && SYNTAXES.find_syntax_by_token(base).is_some() {
        return Some(base.to_owned());
    }

    None
}

pub(super) fn code_block(
    tag: &str,
    code: &str,
) -> String {
    let resolved = resolve_language(tag);

    let language = match &resolved {
        Some(language) => format!("lang: {}, ", literal(language)),
        None => String::new(),
    };

    // The label keeps the author's spelling when no grammar claims it.
    let label = match (resolved.as_deref(), tag) {
        (_, "") => "none".to_owned(),
        (Some(language), _) => literal(language),
        (None, tag) => literal(tag),
    };

    format!(
        "#code-block({label}, raw(block: true, {language}{}))\n\n",
        literal(code),
    )
}

pub(super) fn inline_math(tex: &str) -> String {
    match tex2typst_rs::tex2typst(tex.trim()) {
        Ok(typst) => format!("${typst}$"),
        Err(_) => format!("#({})", literal(tex.trim())),
    }
}

pub(super) fn display_math(tex: &str) -> String {
    match tex2typst_rs::tex2typst(tex.trim()) {
        Ok(typst) => format!("#math-block($ {typst} $)"),
        Err(_) => format!("#math-block[#({})]", literal(tex.trim())),
    }
}

pub(super) fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_tags_resolve_the_way_highlight_js_did() {
        assert_eq!(resolve_language("python").as_deref(), Some("python"));
        assert_eq!(resolve_language("python-repl").as_deref(), Some("python"));
        assert_eq!(resolve_language("").as_deref(), None);
        assert_eq!(resolve_language("not-a-language").as_deref(), None);
    }
}
