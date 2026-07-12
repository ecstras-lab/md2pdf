//! The filesystem vocabulary both front ends share. One walker, one way to
//! show a path, one way to name a PDF, so the command and the interface can
//! never drift apart on any of them.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Where PDFs land when nothing chooses otherwise.
pub const DEFAULT_OUTPUT_DIR: &str = "PDF";

/// How far below the working directory the walkers look. The picker and the
/// did-you-mean hint share it, so they always see the same files.
pub const SEARCH_DEPTH: usize = 4;

/// Folders nothing worth converting lives in. Dot folders are skipped by the
/// walker as well, so `.obsidian` and `.git` alike stay out of every search.
const SKIPPED: [&str; 3] = ["target", DEFAULT_OUTPUT_DIR, "node_modules"];

/// Walks `root` at most `depth` levels deep, handing every file to `visit`.
pub fn walk(
    root: &Path,
    depth: usize,
    visit: &mut impl FnMut(PathBuf),
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
            visit(path);
            continue;
        }

        let skip = path
            .file_name()
            .and_then(|folder| folder.to_str())
            .is_some_and(|folder| SKIPPED.contains(&folder) || folder.starts_with('.'));

        if !skip {
            walk(&path, depth - 1, visit);
        }
    }
}

/// Whether the path names a markdown file.
pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

/// How a path is shown, spelled the same on every platform.
pub fn display(path: &Path) -> String {
    let shown = path.strip_prefix(".").unwrap_or(path);

    shown.display().to_string().replace('\\', "/")
}

/// The file a source's PDF is named. The extension is appended rather than
/// swapped in, because `with_extension` would eat the last dotted segment of
/// a stem like `notes.v1` and collide it with its siblings.
pub fn pdf_file_name(source: &Path) -> Option<OsString> {
    let mut name = source.file_stem()?.to_os_string();
    name.push(".pdf");

    Some(name)
}

/// Folds `.` and `..` components away without touching the filesystem, so a
/// path assembled around them cannot wander out of the folder it is shown in.
pub fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `notes.v1.md` and `notes.v2.md` are different notes, and must not both
    /// land on `notes.pdf`.
    #[test]
    fn dotted_stems_keep_every_segment() {
        assert_eq!(
            pdf_file_name(Path::new("notes.v1.md")).unwrap(),
            OsString::from("notes.v1.pdf")
        );
        assert_eq!(
            pdf_file_name(Path::new("2024.01.15.md")).unwrap(),
            OsString::from("2024.01.15.pdf")
        );
    }

    #[test]
    fn a_path_is_shown_with_forward_slashes_and_no_leading_dot() {
        let path = Path::new(".").join("tests").join("test.md");

        assert_eq!(display(&path), "tests/test.md");
    }

    #[test]
    fn normalizing_folds_dots_away() {
        assert_eq!(
            normalize(Path::new("a/./b/../c/note.md")),
            PathBuf::from("a/c/note.md")
        );
        assert_eq!(normalize(Path::new("../note.md")), PathBuf::from("note.md"));
    }

    #[test]
    fn the_walker_finds_the_fixture_and_skips_the_output() {
        let mut found = Vec::new();
        walk(Path::new("."), SEARCH_DEPTH, &mut |path| {
            if is_markdown(&path) {
                found.push(display(&path));
            }
        });

        assert!(found.iter().any(|label| label == "tests/test.md"));
        assert!(!found.iter().any(|label| label.starts_with("target/")));
        assert!(!found.iter().any(|label| label.starts_with("PDF/")));
    }
}
