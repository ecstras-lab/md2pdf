# Decisions

Why the port is shaped the way it is. Read this for reasoning, not for what the system does. `docs/architecture.md` is the truth about that.

## Text is emitted as Typst string literals, not markup

The obvious approach is to escape the characters Typst treats as special. The trouble is that the set is large and context dependent, covering `#`, `$`, `*`, `_`, backtick, brackets, angle brackets, `@`, `~`, quotes, and the runs `--` and `...` that Typst silently rewrites. Getting one wrong turns a note into a compile error or, worse, into silently wrong output.

Wrapping each run of text in a Typst string literal sidesteps the whole class. Typst inserts a string verbatim. The escaping problem collapses to two characters, the backslash and the double quote, which is a problem Rust already solves. As a bonus the output is more faithful than markup would have been, because Typst never curls a quote or joins two hyphens into a dash.

## The text frame is made to match a CSS line box

This is the single most load bearing thing in `assets/theme.typ`, and it was wrong in the first draft.

Typst measures a run of text from its cap height down to its baseline. For Montserrat that frame is 0.7em tall. CSS measures the whole line box, which is `line-height`, and it splits the leftover half leading evenly above and below the font's own ascender and descender. The two disagree by about half an em.

Every margin in the stylesheet is a CSS pixel value, so as long as the frame is short, each margin lands against the wrong edge. The first version came out roughly twenty percent tighter than the browser build, everywhere at once. It also made the callout icons hang low, because a 12pt icon top aligned against an 8.4pt text frame sits well below the middle of the words next to it.

The fix is `line-box`, which sets `top-edge` and `bottom-edge` to the font's extents plus half the leading, so the frame is exactly the line height. Leading then drops to zero, because the frame already carries it. After that every pixel value in the stylesheet means what it meant in CSS, and the rendered document tracks the original to within about two points over its whole length.

## Borders are pulled inside the box

CSS draws a border inside the element and grows the box by its width. Typst centres a stroke on the box edge and grows nothing. Left alone, this shifts a coloured rule half its width into the margin and leaves every bordered block a point or two short.

Two corrections handle it. A block with a full border takes an inset equal to the border width, so its content starts just inside the border and the box grows the way CSS grows it. A block with only a left rule, meaning the callouts, the blockquotes and the h3 through h6 headings, takes a negative left `outset` of half the rule, which pulls the drawn edge back to where `border-left` had it.

The horizontal rule is a filled block rather than a stroked line, because `hr` is a one pixel tall box with a background, not a border.

## An inline box is not an inline element

CSS gives an inline element a painted box that is its own content area plus its padding and border, and it lets that box overflow the line without disturbing it. Typst has no such thing. The nearest tool is `box`, which is an atom that the line grows to contain, and which takes its height from whatever text sits inside it. Three of the small pills in this document ran into that.

- The tag pill inherited the document's 1.3 line box rather than its own 1.25, which made it tall enough that two stacked tags collided. It now sets its own line box, and it appends a zero width space, because a line holding nothing but boxes shrinks to the boxes and the gap between stacked tags disappears.
- The footnote reference badge rides above the line on `top: -0.5em`. Nothing reproduces that for free. `move` turns out to be a block, so the badge landed on a line of its own. Anything reached through `place` has its fill dropped. Raising the box's baseline is the one form Typst paints, and it costs about three points of line height wherever a footnote is referenced. That was the right trade, because the raised box carries the link annotation with it, and a badge you cannot click is worse than a line that is three points tall.
- The property tag pill painted its padding with an `outset`, which draws without reserving room. The pills came out taller than the rows holding them. Padding and border belong in the `inset`.

## Markup line breaks are spaces, flex gaps are not

Several rows in the properties block and the footnote list are flex containers in CSS, where whitespace between children is discarded. Writing the same thing across several lines of Typst markup inserts a space at every line break, which pushed the icons and the notes a hair over three points to the right. Those helpers are built in code mode instead, where the pieces join with nothing between them.

## An embed that cannot be drawn says so in the document

A missing image, a video, a note transclusion. The converter used to skip these and mention them on the terminal, which left a hole in the PDF that read as if nothing had ever been there. Each one now leaves a marked box naming the reason, where the embed belonged. The reader of the file learns as much as the person who ran the command.

The command reports itself on every run, naming the theme, the source, the output and anything it skipped. `--quiet` turns that off. It hides nothing, because every skipped embed is marked in the PDF as well.

## The preview is the page, not a picture of the page

The obvious way to show a note in the terminal is to render some approximation of it, in text. The interesting way turned out to be cheaper. Typst already lays the document out, and `typst-render` turns a laid out page into pixels using the same crate version that `typst-pdf` exports from. So the preview is the document, drawn at whatever resolution the pane happens to be.

That has a consequence worth stating. The interactive front end keeps the laid out document that its preview was drawn from, and exports that when the reader presses enter. The file on disk is the pages they were looking at, rather than a fresh compile that might disagree with them.

Two things follow from a page that is one image thousands of pixels tall. It is rendered once per note, per theme and per pane width, because those are the three things that change its size. Scrolling only cuts a different slice out of it, which costs nothing but the encoding.

The page is rendered at twice the pane resolution and then shrunk to fit, rather than straight at the resolution of the terminal. A terminal cell is a coarse thing to draw small body text into, and rendering straight at that size comes out soft. Downsampling a larger render is what gives the text a clean edge. A ceiling on the rendered height keeps a long note from asking for an image too tall to hold, at the cost of some sharpness once a note is very long.

The interface takes its own colours from the document palette. Toggling the theme retints the borders, the labels and the background along with the page, so the choice shows what it does instead of naming it.

## Nothing slow runs on the main thread

The first cut of the interface did two slow things where the keyboard could feel them. It encoded the visible slice of the page inside the draw, and it wrote the PDF inside the keypress that asked for it. Both froze the interface, the first on every scroll and the second every time a file was saved. The spinner, being on the same thread, froze with them.

So the three slow things each moved onto a thread. Typesetting and rendering a note, encoding a slice of it for the terminal, and writing a PDF. The loop hands work out and draws whatever has come back, and it takes a whole burst of held-key events before it draws, so a long scroll cannot fall a frame behind the keyboard.

The encoder is the interesting one, because scrolling asks it for a new slice faster than a terminal can be painted. It keeps only the latest request and drops the rest, and every drawing carries the number of the view it belongs to, so one for a scroll position already left behind is thrown away rather than shown. The reader sees the page catch up to where they scrolled, never a queue of stale frames draining out.

A theme is switched the same careful way. The chrome retints at once, because that is free, but the page it was showing is now the wrong colour, so the interface shows it loading until the newly rendered page arrives. The chrome and the page never disagree on screen.

## The terminal is a library's problem

The first version of `report.rs` asked `IsTerminal` whether anyone was watching, looked for `NO_COLOR`, and decided per stream whether to emit escape codes. It missed `CLICOLOR`, `CLICOLOR_FORCE` and `TERM=dumb`. Worse, on Windows a console will not act on an escape sequence until something turns escape sequence handling on, so a run under `conhost` printed the codes instead of obeying them.

`anstream` answers all of that, and it was already in the tree underneath clap, so depending on it directly costs nothing to build. Every line the report prints is now painted without asking, and the stream strips the paint back off on the way to a pipe or a file.

clap draws its colour choice from the same global, which is what makes `--color never` reach an error the parser raised before any of this code ran. The flag has to be found in the raw arguments and applied first, because the parser that owns it is also the thing that prints. Once clap has run and validated the word, the choice is applied again from the parsed value, which is the one that survives.

## Raw HTML is dropped

The browser build rendered `<div style="...">` and `<details>` natively. Typst has no HTML engine, so there is nothing to render them with. The alternatives were to strip the tags and keep the inner text, or to print the HTML source verbatim.

Dropping was chosen. Note the consequence, which is not obvious. An HTML block arrives from the parser as one opaque event, so the whole thing goes, text included. Inline tags such as `<b>` are separate events from the words they wrap, so only the tags go and the words survive. That is why `<details><summary>Click</summary>` loses the word Click, while the body paragraph beneath it stays.

## One continuous page

The browser build measured the rendered height and emitted a single page that tall, with no page breaks. Typst reproduces this exactly with `height: auto`, so it was kept. Real pagination was the alternative and remains a one line change if it is ever wanted.

This choice also solves footnotes for free. Typst puts footnotes at the bottom of their page and offers no document end mode. With one page, the bottom of the page is the end of the document.

## Fonts are vendored into the binary

The stylesheet asked for Montserrat and JetBrains Mono from the Google Fonts CDN. Typst needs real font files. Searching the system was the cheap option, but neither family was installed on the development machine, and a silent fallback to some other face would have quietly broken the theme on any machine that was missing them.

Two fonts were added that the original never named.

- DejaVu Sans, because Montserrat carries none of the ornaments the theme uses. The browser was quietly falling back to a system font for ✦, ⚙, ↩, ◦ and ▪, and there is no system font to fall back to here.
- New Computer Modern Math, because Typst lays equations out from a font's OpenType MATH table and none of the text faces carry one. Without it every equation fails with a bare message about no font being found. The browser had no such constraint, since KaTeX drew formulas out of ordinary glyphs.

The cost is about 4.2MB of assets. The gain is a binary that renders identically anywhere.

## Callouts are parsed by hand, not by the GFM extension

`pulldown-cmark` can parse GitHub alerts, which look identical to Obsidian callouts. It knows five kinds. Obsidian has 27, and the parser backtracks on any name it does not recognise, leaving the marker as literal text. Half of the callouts would have worked and the other half would have printed `[!bug]` into the document.

So the extension stays off and the marker is matched by hand. This turned out to need lookahead, because the parser splits `[!note]` into three separate text events, which is why the event stream is collected into a vector rather than streamed.

## Image destinations are wrapped in angle brackets

`![[Pasted image 20260326170345.png]]` expands to a Markdown image whose path contains spaces. CommonMark rejects a bare destination with a space in it, so the result is not an image at all. It is a paragraph of literal text.

The original had the same bug and worked around it downstream, with a pass over the generated HTML that searched paragraphs for anything that looked like unparsed image syntax. Here the destination is wrapped in angle brackets instead, which CommonMark defines for exactly this case. The workaround is then unnecessary and was not ported.

## Language tags are resolved against Typst's own syntax set

The original downgraded `python-repl` to `python`, because highlight.js knows the second and not the first. Typst renders an unknown language as plain text without complaining, so there is no way to detect the miss after the fact.

`two-face`, the crate that supplies Typst's grammars, is already in the dependency tree. Depending on it directly costs nothing to build and lets the same question be asked before the source is emitted. A tag that resolves is then guaranteed to highlight.

## Dead CSS variables were not ported

The stylesheet declared `--popover`, `--destructive`, `--input`, `--ring`, `--secondary-foreground` and `--accent-foreground` and never used any of them. They are gone. The `Theme` struct holds only colours that something reads.

## Known divergences

Worth knowing before comparing a new PDF against an old one.

- Equations are set in New Computer Modern Math. The old stylesheet forced KaTeX into Montserrat, which cannot be reproduced without a sans font that has a MATH table.
- Box shadows are gone. Typst has no equivalent, and faking one is not worth the complexity. Code blocks, tables and the properties block carry their borders but no shadow.
- Table columns are equal width. CSS sized them by content while filling the line. Typst has no direct equivalent.
- An undefined footnote reference stays as literal text. The old build emitted a link to an anchor that was never written.
- The ✦ above the title and the ⚙ in the properties header are DejaVu glyphs, where the browser reached for whatever the system offered. They are drawn a little differently. The star is set at 14.6pt rather than the stylesheet's 1rem so that its ink comes out the same size.
- The very first element on the page loses its top margin. Typst drops a block's leading spacing at the top of a page, so the title rule sits about six points higher than the browser put it. Everything below it keeps its spacing.
- A line carrying a footnote reference is about three points taller than the rest, because the raised badge is a box and Typst grows the line to hold it. See the note above on inline boxes.
- Footnote links may do nothing in some PDF viewers. The destinations are correct, and point at the right place on the page. The document is a single page thousands of points tall, and not every viewer will scroll within one page to reach a destination. Adobe Acrobat does not, Microsoft Edge does, and the browser build behaved the same way. Pagination would settle it.
- The notes divider is tinted with the primary colour in both themes. The old stylesheet turned it white on dark, which read as a mistake next to every other accent in the document.
