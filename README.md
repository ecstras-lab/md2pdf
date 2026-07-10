<p align="center">
  <img
    width="640"
    alt="md2pdf"
    src="media/name.png" />
</p>

<p align="center">
  <br>
  <img
    alt="One note, rendered in the light theme and the dark theme"
    src="media/showcase.png" />
  <br><br>
</p>

## Overview

md2pdf turns an Obsidian flavoured Markdown note into a themed PDF. There is no browser anywhere in it. The note becomes Typst markup, and Typst typesets the PDF inside the same process.

It began as a port of a Node script that drove Puppeteer over a page of handwritten CSS. Everything that stylesheet did is here, matched against the old output to within about two points over a document six thousand points tall. What the browser was doing implicitly, such as reaching for a system font when Montserrat had no ✦ to give, is now done on purpose.

The result is one binary, with the fonts inside it, that renders the same document on any machine.

## Features

* **Obsidian syntax:** all 27 callout kinds, wikilink embeds, `==highlights==`, `%%comments%%`, `#tags`, YAML frontmatter as a properties table, footnotes with backlinks, and task lists.
* **Syntax highlighting:** the theme's highlight.js colours are translated into a TextMate scheme at run time, so code is coloured by the same syntect that Typst ships with.
* **Math:** LaTeX is converted to Typst math, inline and display.
* **Two themes:** light and dark, both carrying the original palette down to the hex value.
* **No network, no browser:** Montserrat, JetBrains Mono, DejaVu Sans and New Computer Modern Math are compiled into the binary.
* **Honest about gaps:** an embed the converter cannot draw, such as a video or an image that is not there, leaves a marked box in the PDF saying why.

## Usage

```bash
md2pdf note.md                       # writes PDF/note.pdf, light theme
md2pdf note.md -t dark               # dark theme
md2pdf notes/post.md -o ~/post.pdf   # choose the output path
md2pdf note -q                       # add the .md, and say nothing
```

Every run reports the theme, the source, the output, and any embed it could not draw. A missing `.md` extension is added for you. Without `--output` the PDF mirrors the source tree beneath `PDF/`, so `notes/2024/post.md` lands at `PDF/notes/2024/post.pdf`.

```
  -t, --theme <light|dark>   colour theme, light by default
  -o, --output <PATH>        write the PDF here
  -q, --quiet                report nothing but errors
```

## Building

You need Rust and Cargo. Nothing else.

```bash
git clone https://github.com/ecstra/md2pdf.git
cd md2pdf
cargo build --release
```

The fixture note under `tests/` exercises every feature at once, alongside the image, the video and the note it embeds.

```bash
cargo run -- tests/test.md
```

## How It Works

Four stages, one module each.

1. `markdown/frontmatter.rs` splits the YAML block off the top and sorts each value into one of the five shapes the properties table draws.
2. `markdown/` rewrites Obsidian embeds, parses the rest with `pulldown-cmark`, and walks the event stream to emit Typst markup. Every run of text is emitted as a Typst string literal, so no character in a note can be mistaken for syntax.
3. `document/` renders the theme as Typst bindings, glues them in front of `assets/theme.typ`, and appends the body.
4. `document/compile.rs` hands the whole source to Typst with the embedded fonts and the in memory files, then exports the PDF.

`assets/theme.typ` is the stylesheet, written in Typst rather than CSS. The body never styles anything itself. It only calls the helpers the stylesheet defines.

## Fidelity

The port is measured, not eyeballed. Against the original browser output, every horizontal landmark lands within 0.6pt, and the worst vertical drift across the shared sections is under three points. Two tests are worth knowing about.

* `the_stylesheet_compiles_every_element_it_styles` runs a fixture through a real Typst compile, in both themes, so a syntax error in `assets/theme.typ` fails in CI rather than at your shell.
* `syntect_parses_the_generated_theme` feeds the generated colour scheme to the same library Typst uses.

Where the port could not match the browser, `docs/decisions.md` says so and says why. Equations are set in New Computer Modern Math, because KaTeX drew formulas from ordinary glyphs and Typst needs a font with a MATH table. Box shadows are gone. Raw HTML is dropped, since a PDF has no HTML engine to render it with.

## Licences

The vendored fonts keep their own licences, which sit beside them in `assets/fonts/`.

* Montserrat and JetBrains Mono: SIL Open Font License.
* DejaVu Sans: the DejaVu licence, a permissive Bitstream Vera derivative.
* New Computer Modern Math: the GUST Font License.
