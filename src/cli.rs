//! What the user typed, and where it points.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anstyle::{AnsiColor, Style};
use anyhow::{Context, Result};
use clap::builder::Styles;
use clap::{ColorChoice, Parser, ValueEnum};

use crate::files;
use crate::report::Failure;
use crate::theme::Theme;

/// The help and the run report share one palette, so `--help` and the lines a
/// run prints afterwards look like they came from the same program.
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Cyan.on_default().bold())
    .usage(AnsiColor::Cyan.on_default().bold())
    .literal(Style::new().bold())
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::Red.on_default().bold())
    .valid(AnsiColor::Cyan.on_default().bold())
    .invalid(AnsiColor::Yellow.on_default().bold());

#[derive(Clone, Copy, PartialEq, Debug, ValueEnum)]
pub(crate) enum ThemeName {
    // A doc comment here would be printed under `--help` as the meaning of the
    // word `light`, which is a thing nobody needs told. `white` is accepted
    // because the flag this replaced was named `--white`.
    #[value(alias = "white")]
    Light,
    Dark,
}

impl ThemeName {
    pub(crate) fn build(self) -> Theme {
        match self {
            Self::Light => Theme::light(),
            Self::Dark => Theme::dark(),
        }
    }

    pub(crate) fn label(self) -> &'static str {
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
    styles = STYLES,
    after_help = "\
Examples:
  md2pdf note.md                       writes PDF/note.pdf
  md2pdf note.md -t dark               render dark
  md2pdf notes/post.md -o ~/post.pdf   choose the output path
  md2pdf note -q                       add the .md, and say nothing
  md2pdf -i                            browse and preview, then export

Every run reports the theme, the source, the output, and any embed it could
not draw. Those embeds are marked in the PDF too, so `--quiet` hides nothing
that is not already in the file.

Colour is on whenever a terminal is reading. NO_COLOR, CLICOLOR_FORCE and
--color all have a say in that."
)]
pub(crate) struct Cli {
    /// The Markdown file to convert. A missing `.md` extension is added.
    #[arg(value_name = "FILE", required_unless_present = "interactive")]
    pub(crate) file: Option<PathBuf>,

    /// Browse notes and preview the page before writing it.
    #[arg(short, long)]
    pub(crate) interactive: bool,

    /// Colour theme. A file is light by default, the interface dark.
    #[arg(short, long, value_enum)]
    pub(crate) theme: Option<ThemeName>,

    /// Write the PDF here instead of PDF/<source directory>/<name>.pdf.
    #[arg(short, long, value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,

    /// Report nothing but errors.
    #[arg(short, long)]
    pub(crate) quiet: bool,

    /// When to colour what is printed.
    #[arg(long, value_name = "WHEN", value_enum, default_value_t = ColorChoice::Auto)]
    pub(crate) color: ColorChoice,
}

impl Cli {
    /// The theme to render with, filling in the per-mode default when the flag
    /// was left off. A page on paper reads best light. A page on a dark
    /// terminal reads best dark, and matches the interface around it.
    pub(crate) fn chosen_theme(&self) -> ThemeName {
        self.theme.unwrap_or(match self.interactive {
            true => ThemeName::Dark,
            false => ThemeName::Light,
        })
    }
}

/// Reads the command line, having first settled how the output is to be
/// coloured.
pub(crate) fn parse() -> Cli {
    if let Some(choice) = color_from(std::env::args().skip(1)) {
        use_color(choice);
    }

    let cli = Cli::parse();
    use_color(cli.color);

    cli
}

/// clap prints `--help` and its own errors from inside the parser, drawing the
/// choice from the same place `anstream` does. So `--color` has to be settled
/// before the parser that owns it has ever run. This pass is lenient, and a
/// word it does not know is left for clap to reject with a proper message.
fn color_from(mut args: impl Iterator<Item = String>) -> Option<ColorChoice> {
    while let Some(arg) = args.next() {
        let word = if arg == "--color" {
            args.next()?
        } else if let Some(word) = arg.strip_prefix("--color=") {
            word.to_owned()
        } else {
            continue;
        };

        return ColorChoice::from_str(&word, true).ok();
    }

    None
}

/// Hands the choice to every stream the program prints on.
fn use_color(choice: ColorChoice) {
    match choice {
        ColorChoice::Auto => anstream::ColorChoice::Auto,
        ColorChoice::Always => anstream::ColorChoice::Always,
        ColorChoice::Never => anstream::ColorChoice::Never,
    }
    .write_global();
}

/// The path the user named is not there. Look for it somewhere else before
/// giving up, because a note that moved into a folder is the common case.
pub(crate) fn missing_source(source_path: &Path) -> Failure {
    let failure = Failure::new(format!("no such file `{}`", source_path.display()));

    let Some(name) = source_path.file_name() else {
        return failure;
    };

    let matches = find_by_name(Path::new("."), name);

    if matches.is_empty() {
        return failure.hint("check the path, or run `md2pdf --help`");
    }

    matches.iter().fold(failure, |failure, candidate| {
        failure.hint(format!("did you mean `{}`?", files::display(candidate)))
    })
}

/// Every file called `name` in reach of the shared walker, so the hint offers
/// exactly what the interactive picker would list.
fn find_by_name(
    root: &Path,
    name: &OsStr,
) -> Vec<PathBuf> {
    let mut found = Vec::new();

    files::walk(root, 4, &mut |path| {
        if path.file_name() == Some(name) {
            found.push(path);
        }
    });

    found
}

pub(crate) fn with_markdown_extension(path: PathBuf) -> PathBuf {
    match path.extension() {
        Some(extension) if extension.eq_ignore_ascii_case("md") => path,
        _ => {
            let mut name = path.into_os_string();
            name.push(".md");
            PathBuf::from(name)
        }
    }
}

/// Mirrors the source tree beneath `PDF/`, as the browser build did, so
/// `notes/2024/post.md` becomes `PDF/notes/2024/post.pdf`. The path is
/// normalised first, keeping a `..` in the source from steering the output
/// back out of the folder. A source from outside the working tree keeps its
/// parent folder's name, so notes from two vaults cannot land on each other.
pub(crate) fn default_output_path(source_path: &Path) -> Result<PathBuf> {
    let working_dir = std::env::current_dir()?;
    let absolute = files::normalize(&working_dir.join(source_path));

    let directory = absolute.parent().unwrap_or(&working_dir).to_path_buf();
    let relative = match directory.strip_prefix(&working_dir) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => directory.file_name().map(PathBuf::from).unwrap_or_default(),
    };

    let file = files::pdf_file_name(&absolute).context("the source file has no name")?;

    Ok(working_dir
        .join(files::DEFAULT_OUTPUT_DIR)
        .join(relative)
        .join(file))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// `notes.v1.md` and `notes.v2.md` are different notes. Swapping the
    /// extension in would collide them both onto `notes.pdf`.
    #[test]
    fn dotted_stems_survive_into_the_output_name() {
        let working_dir = std::env::current_dir().unwrap();

        let output = default_output_path(Path::new("notes.v1.md")).unwrap();
        assert_eq!(output, working_dir.join("PDF").join("notes.v1.pdf"));
    }

    /// A `..` in the source must not steer the output back out of `PDF/`.
    #[test]
    fn parent_components_cannot_escape_the_output_folder() {
        let working_dir = std::env::current_dir().unwrap();

        let output = default_output_path(Path::new("../md2pdf/note.md")).unwrap();
        assert_eq!(output, working_dir.join("PDF").join("note.pdf"));
    }

    /// A source outside the working tree keeps its parent folder's name, so
    /// same-stem notes from two elsewhere places cannot land on each other.
    #[test]
    fn outside_sources_keep_their_parent_folder_name() {
        let working_dir = std::env::current_dir().unwrap();

        let output = default_output_path(Path::new("C:/vault/daily/note.md")).unwrap();
        assert_eq!(
            output,
            working_dir.join("PDF").join("daily").join("note.pdf")
        );
    }

    /// The note that moved into `tests/` is the case this exists for.
    #[test]
    fn a_missing_note_is_looked_for_nearby() {
        let matches = find_by_name(Path::new("."), OsStr::new("test.md"));

        assert!(
            matches.iter().any(|path| path.ends_with("tests/test.md")),
            "did not find the fixture note, got {matches:?}",
        );
    }

    #[test]
    fn the_search_stays_out_of_build_output() {
        let matches = find_by_name(Path::new("."), OsStr::new("test.md"));

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
    fn the_theme_can_be_named() {
        assert_eq!(
            parse(&["a.md", "--theme", "dark"]).unwrap().theme,
            Some(ThemeName::Dark)
        );
        assert_eq!(
            parse(&["a.md", "-t", "light"]).unwrap().theme,
            Some(ThemeName::Light)
        );
        assert_eq!(parse(&["a.md"]).unwrap().theme, None);
        assert!(parse(&["a.md", "--theme", "purple"]).is_err());
    }

    /// A file on paper defaults to light. The interface, sitting on a dark
    /// terminal, defaults to dark. A named theme wins over either.
    #[test]
    fn the_theme_default_follows_the_mode() {
        assert_eq!(parse(&["a.md"]).unwrap().chosen_theme(), ThemeName::Light);
        assert_eq!(parse(&["-i"]).unwrap().chosen_theme(), ThemeName::Dark);
        assert_eq!(
            parse(&["-i", "-t", "light"]).unwrap().chosen_theme(),
            ThemeName::Light
        );
    }

    /// The `--white` flag is gone, but the word it used is still what reaches
    /// for the light theme first.
    #[test]
    fn white_is_accepted_as_a_name_for_the_light_theme() {
        assert_eq!(
            parse(&["a.md", "--theme", "white"]).unwrap().theme,
            Some(ThemeName::Light)
        );
    }

    #[test]
    fn the_replaced_theme_flags_are_gone() {
        assert!(parse(&["a.md", "--white"]).is_err());
        assert!(parse(&["a.md", "--dark"]).is_err());
    }

    #[test]
    fn a_run_reports_itself_unless_silenced() {
        assert!(!parse(&["a.md"]).unwrap().quiet);
        assert!(parse(&["a.md", "-q"]).unwrap().quiet);
        assert!(parse(&["a.md", "--quiet"]).unwrap().quiet);
        assert!(
            parse(&["a.md", "--verbose"]).is_err(),
            "reporting is now the default"
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
        assert_eq!(cli.theme, Some(ThemeName::Dark));
        assert_eq!(cli.output.unwrap(), PathBuf::from("test.pdf"));
    }

    #[test]
    fn colour_is_automatic_until_it_is_named() {
        assert_eq!(parse(&["a.md"]).unwrap().color, ColorChoice::Auto);
        assert_eq!(
            parse(&["a.md", "--color", "never"]).unwrap().color,
            ColorChoice::Never
        );
        assert_eq!(
            parse(&["a.md", "--color=always"]).unwrap().color,
            ColorChoice::Always
        );
        assert!(parse(&["a.md", "--color", "beige"]).is_err());
    }

    fn scan(args: &[&str]) -> Option<ColorChoice> {
        color_from(args.iter().map(|arg| (*arg).to_owned()))
    }

    /// The pre-scan runs before clap and has to read the flag both ways round,
    /// wherever on the line it lands.
    #[test]
    fn the_colour_flag_is_found_ahead_of_the_parser() {
        assert_eq!(
            scan(&["a.md", "--color", "never"]),
            Some(ColorChoice::Never)
        );
        assert_eq!(scan(&["--color=always", "a.md"]), Some(ColorChoice::Always));
        assert_eq!(scan(&["a.md", "--color", "AUTO"]), Some(ColorChoice::Auto));
    }

    /// Nothing to say is said by saying nothing, and clap gets to be the one
    /// that complains about a word it does not know.
    #[test]
    fn a_colour_the_pre_scan_cannot_read_is_left_for_clap() {
        assert_eq!(scan(&["a.md"]), None);
        assert_eq!(scan(&["a.md", "--color"]), None);
        assert_eq!(scan(&["a.md", "--color", "beige"]), None);
        assert_eq!(scan(&["--colorless", "a.md"]), None);
    }
}
