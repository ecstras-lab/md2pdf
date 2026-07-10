//! Text becomes a Typst string literal, which Typst inserts verbatim. That is
//! the whole reason no character in a note can be mistaken for Typst syntax.

/// Wraps text in a Typst string literal. Typst inserts the contents verbatim,
/// so nothing inside can be mistaken for markup.
pub(super) fn literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');

    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(character),
        }
    }

    out.push('"');
    out
}

/// A string literal spliced into markup, e.g. `#("text")`.
pub(super) fn push_literal(
    out: &mut String,
    text: &str,
) {
    if !text.is_empty() {
        out.push_str(&format!("#({})", literal(text)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_escape_quotes_and_backslashes() {
        assert_eq!(literal(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(literal("line\nnext"), r#""line\nnext""#);
    }
}
