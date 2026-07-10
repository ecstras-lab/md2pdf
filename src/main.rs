//! Converts an Obsidian-flavoured Markdown file into a themed PDF.

mod color;
mod compile;
mod document;
mod frontmatter;
mod icons;
mod markdown;
mod report;
mod theme;
mod tmtheme;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

use report::Failure;
use theme::Theme;

#[derive(Clone, Copy, PartialEq, Debug, ValueEnum)]
enum ThemeName {
    /// `white` is accepted because the flag it replaced was named `--white`.
    #[value(alias = "white")]
    Light,
    Dark,
}

impl ThemeName {
    fn build(self) -> Theme {
        match self {
            Self::Light => Theme::light(),
            Self::Dark => Theme::dark(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "md2pdf",
    version,
    about = "Convert an Obsidian-flavoured Markdown file to a themed PDF",
    after_help = "\
Examples:
  md2pdf note.md                       writes PDF/note.pdf
  md2pdf note.md -t dark               render dark
  md2pdf notes/post.md -o ~/post.pdf   choose the output path
  md2pdf note -v                       add the .md, and say what happened

Nothing is printed unless something needs saying. Embeds that cannot be drawn,
such as a video or a missing image, are marked in the PDF where they belong."
)]
struct Cli {
    /// The Markdown file to convert. A missing `.md` extension is added.
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Colour theme to render with.
    #[arg(short, long, value_enum, default_value_t = ThemeName::Light)]
    theme: ThemeName,

    /// Write the PDF here instead of PDF/<source directory>/<name>.pdf.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Say which theme was used, where the PDF went, and what was skipped.
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let Err(failure) = run() else {
        return;
    };

    failure.print();
    std::process::exit(1);
}

fn run() -> Result<(), Failure> {
    let cli = Cli::parse();
    let source_path = with_markdown_extension(cli.file.clone());

    if !source_path.is_file() {
        return Err(missing_source(&source_path));
    }

    let output_path = match cli.output {
        Some(path) => path,
        None => default_output_path(&source_path)?,
    };

    let warnings = convert(&source_path, &output_path, &cli.theme.build())?;

    if cli.verbose {
        println!("theme:  {}", cli.theme.label());
        println!("source: {}", source_path.display());

        for warning in &warnings {
            println!("skipped: {warning}");
        }

        println!("output: {}", output_path.display());
    } else if !warnings.is_empty() {
        let plural = if warnings.len() == 1 { "" } else { "s" };
        let verb = if warnings.len() == 1 { "is" } else { "are" };

        report::note(&format!(
            "{} embed{plural} could not be drawn, and {verb} marked in the PDF. \
             Run with --verbose to list them.",
            warnings.len(),
        ));
    }

    Ok(())
}

/// The path the user named is not there. Look for it somewhere else before
/// giving up, because a note that moved into a folder is the common case.
fn missing_source(source_path: &Path) -> Failure {
    let failure = Failure::new(format!("no such file `{}`", source_path.display()));

    let Some(name) = source_path.file_name() else {
        return failure;
    };

    let matches = find_by_name(Path::new("."), name, 3);

    if matches.is_empty() {
        return failure.hint("check the path, or run `md2pdf --help`");
    }

    matches.iter().fold(failure, |failure, candidate| {
        // The walk starts at `.`, which nobody wants to read back.
        let shown = candidate.strip_prefix(".").unwrap_or(candidate);
        failure.hint(format!("did you mean `{}`?", shown.display()))
    })
}

/// Every file called `name` within `depth` directories of `root`, skipping the
/// places nothing worth converting lives.
fn find_by_name(
    root: &Path,
    name: &OsStr,
    depth: usize,
) -> Vec<PathBuf> {
    const SKIPPED: [&str; 4] = ["target", "PDF", ".git", "node_modules"];

    if depth == 0 {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut found = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() {
            if path.file_name() == Some(name) {
                found.push(path);
            }
            continue;
        }

        let skip = path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|folder| SKIPPED.contains(&folder));

        if !skip {
            found.extend(find_by_name(&path, name, depth - 1));
        }
    }

    found
}

/// Converts the file and returns everything that had to be skipped.
fn convert(
    source_path: &Path,
    output_path: &Path,
    theme: &Theme,
) -> Result<Vec<String>> {
    let markdown = std::fs::read_to_string(source_path)
        .with_context(|| format!("could not read {}", source_path.display()))?;

    let base_dir = source_path.parent().unwrap_or(Path::new("."));

    let parsed = frontmatter::split(&markdown);
    let rendered = markdown::render(&parsed.body, base_dir, &parsed.properties);

    let mut warnings = parsed.warnings;
    warnings.extend(rendered.warnings);

    let mut files = document::assets(theme);
    files.extend(rendered.files);

    let source = document::source(theme, &rendered.body);

    let pdf = compile::to_pdf(&source, &files)?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }

    std::fs::write(output_path, pdf)
        .with_context(|| format!("could not write {}", output_path.display()))?;

    Ok(warnings)
}

fn with_markdown_extension(path: PathBuf) -> PathBuf {
    match path.extension() {
        Some(extension) if extension.eq_ignore_ascii_case("md") => path,
        _ => {
            let mut name = path.into_os_string();
            name.push(".md");
            PathBuf::from(name)
        }
    }
}

/// Mirrors the source tree beneath `PDF/`, as the browser build did:
/// `notes/2024/post.md` becomes `PDF/notes/2024/post.pdf`.
fn default_output_path(source_path: &Path) -> Result<PathBuf> {
    let working_dir = std::env::current_dir()?;
    let absolute = working_dir.join(source_path);

    let directory = absolute.parent().unwrap_or(&working_dir).to_path_buf();
    let relative = directory
        .strip_prefix(&working_dir)
        .unwrap_or(Path::new(""));

    let stem = absolute
        .file_stem()
        .context("the source file has no name")?
        .to_owned();

    Ok(working_dir
        .join("PDF")
        .join(relative)
        .join(stem)
        .with_extension("pdf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises every helper `assets/theme.typ` defines. A Typst syntax
    /// error or a renamed binding fails here rather than at the user's shell.
    const EVERY_ELEMENT: &str = r#"---
title: Fixture
tags:
  - one
  - two
created: 2024-04-05T15:30:00
home: https://example.com
flag: true
blank:
---

# Title

## Second

### Third
#### Fourth
##### Fifth
###### Sixth

Plain text with *emph*, **strong**, ~~struck~~, ==highlight==, %%comment%%,
a #tag/nested, `inline code`, a [link](https://example.com) and a footnote[^a].

- bullet
  - nested
    - deeper
      - deepest

1. first
   1. inner
      1. innermost

- [ ] open
- [x] done

> plain quote

> [!warning] Careful
> body text

> [!quote] Title only

```python
def greet():
    print("hi")
```

```
no language
```

| A | B |
| - | :-: |
| 1 | 2 |

---

Inline $E = mc^2$ and a block:

$$
\int_0^1 x^2 dx
$$

[^a]: the note
"#;

    #[test]
    fn the_stylesheet_compiles_every_element_it_styles() {
        let parsed = frontmatter::split(EVERY_ELEMENT);
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);

        let rendered = markdown::render(&parsed.body, Path::new("."), &parsed.properties);

        assert!(rendered.warnings.is_empty(), "{:?}", rendered.warnings);

        for theme in [Theme::light(), Theme::dark()] {
            let mut files = document::assets(&theme);
            files.extend(rendered.files.clone());

            let source = document::source(&theme, &rendered.body);

            let pdf = compile::to_pdf(&source, &files);

            assert!(pdf.is_ok(), "{:?}", pdf.err());
        }
    }

    #[test]
    fn a_missing_extension_is_added() {
        assert_eq!(
            with_markdown_extension("note".into()),
            PathBuf::from("note.md")
        );
        assert_eq!(
            with_markdown_extension("note.md".into()),
            PathBuf::from("note.md")
        );
        assert_eq!(
            with_markdown_extension("note.MD".into()),
            PathBuf::from("note.MD")
        );
        assert_eq!(
            with_markdown_extension("a.b/note".into()),
            PathBuf::from("a.b/note.md")
        );
    }

    #[test]
    fn output_mirrors_the_source_tree_under_pdf() {
        let working_dir = std::env::current_dir().unwrap();

        let flat = default_output_path(Path::new("note.md")).unwrap();
        assert_eq!(flat, working_dir.join("PDF").join("note.pdf"));

        let nested = default_output_path(Path::new("notes/2024/post.md")).unwrap();
        assert_eq!(nested, working_dir.join("PDF/notes/2024/post.pdf"));
    }

    /// The note that moved into `tests/` is the case this exists for.
    #[test]
    fn a_missing_note_is_looked_for_nearby() {
        let matches = find_by_name(Path::new("."), OsStr::new("test.md"), 3);

        assert!(
            matches.iter().any(|path| path.ends_with("tests/test.md")),
            "did not find the fixture note, got {matches:?}",
        );
    }

    #[test]
    fn the_search_stays_out_of_build_output() {
        let matches = find_by_name(Path::new("."), OsStr::new("test.md"), 3);

        for path in &matches {
            let text = path.to_string_lossy();
            assert!(!text.contains("target"), "walked into build output: {text}");
        }
    }

    #[test]
    fn a_missing_note_with_no_twin_still_offers_a_way_forward() {
        let failure = missing_source(Path::new("no-such-note-anywhere.md"));

        assert!(failure.hints().iter().any(|hint| hint.contains("--help")));
    }

    #[test]
    fn the_command_definition_is_well_formed() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("md2pdf").chain(args.iter().copied()))
    }

    #[test]
    fn the_theme_defaults_to_light_and_can_be_named() {
        assert_eq!(parse(&["a.md"]).unwrap().theme, ThemeName::Light);
        assert_eq!(
            parse(&["a.md", "--theme", "dark"]).unwrap().theme,
            ThemeName::Dark
        );
        assert_eq!(
            parse(&["a.md", "-t", "light"]).unwrap().theme,
            ThemeName::Light
        );
        assert!(parse(&["a.md", "--theme", "purple"]).is_err());
    }

    /// The `--white` flag is gone, but the word it used is still what reaches
    /// for the light theme first.
    #[test]
    fn white_is_accepted_as_a_name_for_the_light_theme() {
        assert_eq!(
            parse(&["a.md", "--theme", "white"]).unwrap().theme,
            ThemeName::Light
        );
    }

    #[test]
    fn the_replaced_theme_flags_are_gone() {
        assert!(parse(&["a.md", "--white"]).is_err());
        assert!(parse(&["a.md", "--dark"]).is_err());
    }

    #[test]
    fn output_is_silent_unless_asked() {
        assert!(!parse(&["a.md"]).unwrap().verbose);
        assert!(parse(&["a.md", "-v"]).unwrap().verbose);
        assert!(parse(&["a.md", "--verbose"]).unwrap().verbose);
        assert!(
            parse(&["a.md", "--quiet"]).is_err(),
            "quiet is now the default"
        );
    }

    #[test]
    fn an_explicit_output_path_overrides_the_default() {
        let cli = parse(&["notes/a.md", "-o", "out/x.pdf"]).unwrap();
        assert_eq!(cli.output.unwrap(), PathBuf::from("out/x.pdf"));

        assert!(parse(&["notes/a.md"]).unwrap().output.is_none());
    }

    #[test]
    fn flags_may_follow_the_file() {
        let cli = parse(&["test.md", "--theme", "dark", "--output", "test.pdf"]).unwrap();
        assert_eq!(cli.theme, ThemeName::Dark);
        assert_eq!(cli.output.unwrap(), PathBuf::from("test.pdf"));
    }
}
