# Architecture

md2pdf turns an Obsidian flavoured Markdown note into a themed PDF. There is no browser anywhere in the process. Markdown becomes Typst markup, and Typst compiles that markup to a PDF inside the same process.

## Usage

```
md2pdf [OPTIONS] <FILE>

  -i, --interactive              pick a note and a folder, then export
  -t, --theme <light|dark>       colour theme
  -o, --output <PATH>            write the PDF here
  -q, --quiet                    report nothing but errors
      --color <auto|always|never>
```

A missing `.md` extension is added for you. Without `--output` the PDF mirrors the source tree beneath `PDF/`, so `notes/2024/post.md` is written to `PDF/notes/2024/post.pdf`. `--interactive` is the only way to leave the file out, because there it is picked from a list. It also flips the default theme, since a file on paper reads best light and a page on a dark terminal reads best dark.

The interface does not mirror the source tree. It saves into one folder, which starts at `PDF` or the folder of any `--output` given, and names each PDF after its note.

Every run reports the theme, the source, the output, how large the PDF came out and how long it took, along with any embed the converter could not draw. An embed it cannot draw, such as a video, a note transclusion or an image that is not there, also leaves a marked box in the PDF that names the reason. So `--quiet` hides nothing that is not already in the file.

Colour is negotiated by `anstream`. It reads `NO_COLOR`, `CLICOLOR` and `CLICOLOR_FORCE`, asks whether a terminal is reading the stream it is about to write on, and on Windows either turns on escape sequence handling or falls back to the console API. `--color` overrides all of it. `cli.rs` finds that flag in the raw arguments and settles the choice before clap runs, because clap prints its own help and its own errors through the same global, from inside the parser the flag belongs to.

`tests/` holds a fixture note that exercises every feature, alongside the image, the video and the note it embeds. Convert it with `md2pdf tests/test.md` to see the whole theme at once.

## The pipeline

Each stage lives in one module and hands a value to the next.

1. `markdown/frontmatter.rs` splits the YAML block off the top of the note. Every value is sorted into one of the five shapes that the properties table knows how to draw, namely tags, link, date, boolean and text. A block that will not parse is still stripped, so its keys never leak into the document.
2. `markdown/` rewrites Obsidian embeds in `preprocess.rs`, parses the rest with `pulldown-cmark`, and walks the event stream in `renderer.rs` to emit Typst markup. `images.rs` collects the bytes of every local image it resolves.
3. `document/mod.rs` renders the `Theme` as Typst bindings, glues them in front of `assets/theme.typ`, and appends the body. It also builds the two kinds of file that the Typst source reads by path, the syntax theme and the icons.
4. `document/compile.rs` hands the whole source to Typst along with the embedded fonts and those in memory files, and returns the PDF bytes.

`convert.rs` runs the first three stages and the fourth. Both the command and the interface go through it, so a file is never built two different ways.

## The interactive front end

`tui/` picks a note, chooses where the PDF lands, and writes it. It is built on ratatui, and it is a plain loop rather than an async runtime.

- `tui/app.rs` holds the state and is the only place a keypress changes anything.
- `tui/ui.rs` draws. It reads the state and writes none of it.
- `tui/notes.rs` finds the notes and narrows them by a query.

The one slow thing, writing the PDF, runs on a worker thread, so the interface stays live while it works and reports itself when it lands. Nothing else blocks. The note list is a flat view of every note beneath the working directory, nearest first, narrowed by a loose subsequence search where the letters need only appear in order. The save folder is edited in place, and the PDF takes its name from the note.

## Where the styling lives

`assets/theme.typ` is the stylesheet, written in Typst rather than CSS. It reads four bindings that `document/mod.rs` emits ahead of it.

- `palette`: every colour that differs between the light and dark themes.
- `marker-colors`: the four accents that list markers cycle through by depth.
- `heading-rules`: the rule colour for headings three through six.
- `callouts`: a map from each of the 27 Obsidian callout names to its colour and icon.

Lengths in the stylesheet are the original pixel values converted at 0.75pt per pixel, so one rem, which was 16px, is 12pt. Those values only line up because `line-box` makes a run of text occupy the same frame a CSS line box would. Change that helper and every margin in the file drifts. See `docs/decisions.md` for why.

The body that `markdown/renderer.rs` emits never styles anything itself. It only calls the helpers the stylesheet defines, such as `callout`, `code-block`, `doc-table` and `properties-block`. That split is what keeps the theme in one readable file.

## Text is never markup

Every run of text from the source document is emitted as a Typst string literal, spelled `#("like this")`, rather than as Typst markup. Typst inserts the contents of a string verbatim, so no character in a note can be mistaken for syntax. A stray dollar sign cannot open a formula and a stray bracket cannot open a content block.

The test `typst_metacharacters_stay_inside_string_literals` guards this. It strips every literal from the rendered body and asserts that no metacharacter survives outside one.

## Fonts

Four families ship inside the binary, under `assets/fonts/`.

- Montserrat, in six faces, for body text.
- JetBrains Mono, in three faces, for code.
- DejaVu Sans, which carries the ornaments Montserrat lacks, meaning ✦ for the title rule, ⚙ for the properties header, ↩ for a footnote backlink, and the ◦ and ▪ list markers. The mark on the notes divider is drawn as an SVG instead, because no bundled face has a reference mark that reads at 6pt.
- New Computer Modern Math, which Typst needs for its OpenType MATH table. Without it, any equation fails to compile.

Licences sit beside the font files. All four are freely redistributable.

## Syntax highlighting

Typst highlights code with syntect, which wants a TextMate colour scheme rather than CSS classes. `theme/tmtheme.rs` writes one from the theme's colours at run time, mapping each `hljs-*` class to the TextMate scope that the Sublime grammars actually emit. `markdown/inline.rs` resolves a fence's language against the same syntax set Typst uses, so a tag that resolves is guaranteed to highlight.

## Testing

`cargo test` covers the parsing and rendering logic directly. Two tests are worth knowing about.

- `the_stylesheet_compiles_every_element_it_styles` runs a fixture that touches every helper through a real Typst compile, in both themes. A syntax error or a renamed binding in `assets/theme.typ` fails here.
- `syntect_parses_the_generated_theme` feeds the generated colour scheme to the same library Typst uses.

`cargo fmt` is safe to run. `rustfmt.toml` keeps function parameters one per line. It cannot hold call arguments to the same shape, so rustfmt reflows those and the rest of that rule lives in review.
