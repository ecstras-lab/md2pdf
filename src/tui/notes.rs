//! The notes the picker offers, and the search that narrows them.

use std::path::{Path, PathBuf};

/// How far below the working directory a note may sit and still be offered.
const DEPTH: usize = 4;

/// Nothing worth converting lives in any of these.
const SKIPPED: [&str; 4] = ["target", "PDF", ".git", "node_modules"];

/// Every note beneath `root`, nearest first, then alphabetically. The order is
/// what the list shows, so a note in the working directory is never buried
/// under one four folders down.
pub(super) fn find(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, DEPTH, &mut found);

    found.sort_by_key(|path| (path.components().count(), path.clone()));
    found
}

fn walk(
    root: &Path,
    depth: usize,
    found: &mut Vec<PathBuf>,
) {
    if depth == 0 {
        return;
    }

    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() {
            let is_note = path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));

            if is_note {
                found.push(path);
            }
            continue;
        }

        let skip = path
            .file_name()
            .and_then(|folder| folder.to_str())
            .is_some_and(|folder| SKIPPED.contains(&folder) || folder.starts_with('.'));

        if !skip {
            walk(&path, depth - 1, found);
        }
    }
}

/// The notes worth showing for a query, as indices into `notes`.
pub(super) fn matching(
    notes: &[PathBuf],
    query: &str,
) -> Vec<usize> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, path)| contains_in_order(&label(path), query))
        .map(|(index, _)| index)
        .collect()
}

/// How a note is written in the list, and the text a query is matched against.
pub(super) fn label(path: &Path) -> String {
    let shown = path.strip_prefix(".").unwrap_or(path);

    shown.display().to_string().replace('\\', "/")
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
    fn a_label_reads_the_same_on_every_platform() {
        let path = Path::new(".").join("tests").join("test.md");

        assert_eq!(label(&path), "tests/test.md");
    }

    #[test]
    fn the_fixture_note_is_found_and_the_build_output_is_not() {
        let notes = find(Path::new("."));
        let labels: Vec<String> = notes.iter().map(|path| label(path)).collect();

        assert!(labels.iter().any(|label| label == "tests/test.md"));
        assert!(!labels.iter().any(|label| label.contains("target/")));
    }

    /// A note beside the command is more likely to be the one wanted than a
    /// note buried in a folder, so it is offered first.
    #[test]
    fn shallower_notes_are_offered_first() {
        let notes = find(Path::new("."));
        let depths: Vec<usize> = notes.iter().map(|path| path.components().count()).collect();

        assert!(depths.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}
