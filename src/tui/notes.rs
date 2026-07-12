//! The files the picker offers, and the search that narrows them.

use std::path::{Path, PathBuf};

use crate::files;

/// Every markdown file beneath `root`, nearest first, then alphabetically.
/// The order is what the list shows, so a file in the working directory is
/// never buried under one four folders down.
pub(super) fn find(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();

    files::walk(root, files::SEARCH_DEPTH, &mut |path| {
        if files::is_markdown(&path) {
            found.push(path);
        }
    });

    found.sort_by_cached_key(|path| (path.components().count(), path.clone()));
    found
}

/// The labels worth showing for a query, as indices into `labels`.
pub(super) fn matching(
    labels: &[String],
    query: &str,
) -> Vec<usize> {
    labels
        .iter()
        .enumerate()
        .filter(|(_, label)| contains_in_order(label, query))
        .map(|(index, _)| index)
        .collect()
}

/// Every character of the query somewhere in the label, in the order typed.
/// Loose enough that `tsmd` finds `tests/test.md`, strict enough to be useful.
fn contains_in_order(
    label: &str,
    query: &str,
) -> bool {
    let mut haystack = label.chars().flat_map(char::to_lowercase);

    query
        .chars()
        .flat_map(char::to_lowercase)
        .all(|wanted| haystack.any(|letter| letter == wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_matches_scattered_letters_in_order() {
        assert!(contains_in_order("tests/test.md", "tsmd"));
        assert!(contains_in_order("tests/test.md", ""));
        assert!(contains_in_order("tests/Another Note.md", "another"));
    }

    #[test]
    fn a_query_out_of_order_matches_nothing() {
        assert!(!contains_in_order("tests/test.md", "dmts"));
        assert!(!contains_in_order("tests/test.md", "zebra"));
    }

    #[test]
    fn matching_is_indifferent_to_case() {
        assert!(contains_in_order("tests/Another Note.md", "ANOTHER"));
        assert!(contains_in_order("tests/TEST.md", "test"));
    }

    #[test]
    fn the_fixture_note_is_found_and_the_build_output_is_not() {
        let labels: Vec<String> = find(Path::new("."))
            .iter()
            .map(|path| files::display(path))
            .collect();

        assert!(labels.iter().any(|label| label == "tests/test.md"));
        assert!(!labels.iter().any(|label| label.contains("target/")));
    }

    /// A file beside the command is more likely to be the one wanted than a
    /// file buried in a folder, so it is offered first.
    #[test]
    fn shallower_files_are_offered_first() {
        let found = find(Path::new("."));
        let depths: Vec<usize> = found.iter().map(|path| path.components().count()).collect();

        assert!(depths.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}
