//! Converts markdown into the Typst markup consumed by `assets/theme.typ`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};
use regex::Regex;
use two_face::re_exports::syntect::parsing::SyntaxSet;

use crate::frontmatter::{Property, PropertyValue};

/// The same syntax set Typst highlights with, so a tag that resolves here
/// is guaranteed to highlight in the compiled document.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

/// `![["note"]]`, the quoted wikilink embed. Must run before [`WIKILINK`],
/// whose broader pattern would otherwise swallow the quotes.
static WIKILINK_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"!\[\["([^"]+)"\]\]"#).unwrap());

/// `![[note]]`, Obsidian's wikilink embed.
static WIKILINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[\[([^\]]+)\]\]").unwrap());

/// `!["alt"]("percent%20encoded")`, the quoted embed form.
static QUOTED_EMBED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"!\["([^"]+)"\]\("([^"]+)"\)"#).unwrap());

/// Fenced blocks, then inline spans. Order matters: the longest fence wins.
static CODE_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```.*?```|~~~.*?~~~|``.*?``|`[^`\n]*`").unwrap());

/// A hashtag, anchored to a word boundary the way the stylesheet's rule was.
static HASHTAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|\s)#([A-Za-z0-9_/-]+)").unwrap());

/// `[!note] Optional title`, the Obsidian callout marker.
static CALLOUT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[!([A-Za-z-]+)\]\s*(.*)$").unwrap());

/// Raster and vector formats Typst can decode.
const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "svg", "webp"];

/// Recognised only so an embed can be turned down by name rather than by
/// the fact that a PDF has nowhere to put it.
const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "mov", "webm", "mkv", "avi", "m4v"];
const AUDIO_EXTENSIONS: [&str; 5] = ["mp3", "wav", "ogg", "m4a", "flac"];

/// A sentinel that cannot occur in markdown, used to park code while
/// wikilinks are rewritten around it.
const CODE_SENTINEL: char = '\u{e000}';

pub struct Rendered {
    pub body: String,
    /// Virtual path to bytes, registered with the Typst engine.
    pub files: Vec<(String, Vec<u8>)>,
    pub warnings: Vec<String>,
}

pub fn render(
    markdown: &str,
    base_dir: &Path,
    properties: &[Property],
) -> Rendered {
    let prepared = preprocess(markdown);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_MATH);

    let events: Vec<Event> = Parser::new_ext(&prepared, options).collect();

    let defined_footnotes = events
        .iter()
        .filter_map(|event| match event {
            Event::Start(Tag::FootnoteDefinition(name)) => Some(name.to_string()),
            _ => None,
        })
        .collect();

    Renderer {
        events,
        cursor: 0,
        images: Images::new(base_dir),
        warnings: Vec::new(),
        defined_footnotes,
        footnote_numbers: HashMap::new(),
        footnote_order: Vec::new(),
        footnote_bodies: HashMap::new(),
        properties: render_properties(properties),
        properties_emitted: false,
    }
    .run()
}

// ─────────────────────────────────────────────────────────────
// Pre-processing
// ─────────────────────────────────────────────────────────────

/// Rewrites Obsidian's embed syntaxes into plain markdown images.
///
/// pulldown-cmark shatters `![[name]]` into loose bracket tokens, so this has
/// to happen on the source text. Code is parked first: a fence containing
/// `![[..]]` must survive untouched.
fn preprocess(markdown: &str) -> String {
    let mut parked = Vec::new();

    let guarded = CODE_SPAN.replace_all(markdown, |captures: &regex::Captures| {
        parked.push(captures[0].to_owned());
        format!("{CODE_SENTINEL}{}{CODE_SENTINEL}", parked.len() - 1)
    });

    // The quoted form runs first; the general pattern would swallow its quotes.
    let rewritten = QUOTED_EMBED.replace_all(&guarded, |captures: &regex::Captures| {
        let source = urlencoding::decode(&captures[2]).unwrap_or_else(|_| captures[2].into());
        format!("![{}]({})", &captures[1], destination(&source))
    });

    let rewritten = WIKILINK_QUOTED.replace_all(&rewritten, |captures: &regex::Captures| {
        format!("![{}]({})", &captures[1], destination(&captures[1]))
    });

    let rewritten = WIKILINK.replace_all(&rewritten, |captures: &regex::Captures| {
        let target = &captures[1];
        let name = target.rsplit(['/', '\\']).next().unwrap_or(target);
        format!("![{name}]({})", destination(target))
    });

    let mut restored = rewritten.into_owned();
    for (index, code) in parked.iter().enumerate() {
        restored = restored.replace(&format!("{CODE_SENTINEL}{index}{CODE_SENTINEL}"), code);
    }

    restored
}

/// Wraps an image path as a CommonMark angle-bracket destination.
/// Obsidian attachments routinely contain spaces, which a bare destination
/// cannot hold: `![a](my file.png)` is not an image at all, just text.
fn destination(target: &str) -> String {
    let escaped = target
        .replace('\\', r"\\")
        .replace('<', r"\<")
        .replace('>', r"\>");

    format!("<{escaped}>")
}

// ─────────────────────────────────────────────────────────────
// Typst literals
// ─────────────────────────────────────────────────────────────

/// Wraps text in a Typst string literal. Typst inserts the contents verbatim,
/// so nothing inside can be mistaken for markup.
fn literal(text: &str) -> String {
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
fn push_literal(
    out: &mut String,
    text: &str,
) {
    if !text.is_empty() {
        out.push_str(&format!("#({})", literal(text)));
    }
}

// ─────────────────────────────────────────────────────────────
// Images
// ─────────────────────────────────────────────────────────────

struct Images {
    base_dir: PathBuf,
    working_dir: PathBuf,
    files: Vec<(String, Vec<u8>)>,
    resolved: HashMap<String, String>,
}

impl Images {
    fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            files: Vec::new(),
            resolved: HashMap::new(),
        }
    }

    /// Loads a local image and returns the virtual path Typst should read.
    fn resolve(
        &mut self,
        source: &str,
    ) -> Result<String, String> {
        if let Some(existing) = self.resolved.get(source) {
            return Ok(existing.clone());
        }

        if source.starts_with("http://") || source.starts_with("https://") {
            return Err(format!("remote image not embedded: {source}"));
        }

        if source.starts_with("data:") {
            return Err(format!("inline data URI not embedded: {source}"));
        }

        // Judge the format from the name, so a video or a bare `![[Note]]`
        // is turned down for what it is rather than for being missing.
        let extension = Path::new(source)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();

        if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            return Err(match extension.as_str() {
                "" => format!("note embeds are not supported: {source}"),
                extension if VIDEO_EXTENSIONS.contains(&extension) => {
                    format!("video embeds are not supported: {source}")
                }
                extension if AUDIO_EXTENSIONS.contains(&extension) => {
                    format!("audio embeds are not supported: {source}")
                }
                extension => format!(".{extension} is not an embeddable image: {source}"),
            });
        }

        let found = self
            .candidates(source)
            .into_iter()
            .find(|path| path.is_file())
            .ok_or_else(|| format!("image not found: {source}"))?;

        let bytes =
            std::fs::read(&found).map_err(|error| format!("{}: {error}", found.display()))?;

        let virtual_path = format!("/images/{}.{extension}", self.files.len());
        self.files.push((virtual_path.clone(), bytes));
        self.resolved
            .insert(source.to_owned(), virtual_path.clone());

        Ok(virtual_path)
    }

    fn candidates(
        &self,
        source: &str,
    ) -> Vec<PathBuf> {
        let normalized = source.replace('\\', "/");
        let decoded = urlencoding::decode(&normalized)
            .map(|value| value.into_owned())
            .unwrap_or_else(|_| normalized.clone());

        let mut names = vec![source.to_owned(), normalized, decoded];
        names.dedup();

        names
            .iter()
            .flat_map(|name| [self.base_dir.join(name), self.working_dir.join(name)])
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────
// Properties block
// ─────────────────────────────────────────────────────────────

fn render_properties(properties: &[Property]) -> String {
    if properties.is_empty() {
        return String::new();
    }

    let rows: String = properties
        .iter()
        .map(|property| {
            let value = match &property.value {
                PropertyValue::Tags(tags) => {
                    let items: String = tags
                        .iter()
                        .map(|tag| format!("{}, ", literal(tag)))
                        .collect();
                    format!("prop-tags(({items}))")
                }
                PropertyValue::Link(url) => format!("prop-link({})", literal(url)),
                PropertyValue::Date(text) => format!("prop-date({})", literal(text)),
                PropertyValue::Bool(flag) => format!("prop-bool({flag})"),
                PropertyValue::Text(text) => format!("prop-text({})", literal(text)),
            };

            format!("  (key: {}, value: {value}),\n", literal(&property.label))
        })
        .collect();

    format!("#properties-block((\n{rows}))\n\n")
}

// ─────────────────────────────────────────────────────────────
// Renderer
// ─────────────────────────────────────────────────────────────

struct Renderer<'a> {
    events: Vec<Event<'a>>,
    cursor: usize,
    images: Images,
    warnings: Vec<String>,
    defined_footnotes: HashSet<String>,
    footnote_numbers: HashMap<String, usize>,
    footnote_order: Vec<String>,
    footnote_bodies: HashMap<String, String>,
    properties: String,
    properties_emitted: bool,
}

impl Renderer<'_> {
    fn run(mut self) -> Rendered {
        let mut body = String::new();

        while self.cursor < self.events.len() {
            self.node(&mut body);
        }

        // A note without a level-one heading gets its properties up top.
        if !self.properties.is_empty() && !self.properties_emitted {
            body.insert_str(0, &self.properties);
        }

        body.push_str(&self.footnotes_section());

        Rendered {
            body,
            files: self.images.files,
            warnings: self.warnings,
        }
    }

    fn peek(&self) -> Option<&Event<'_>> {
        self.events.get(self.cursor)
    }

    /// Renders events until the `End` that closes the current container.
    fn children(
        &mut self,
        out: &mut String,
    ) {
        while let Some(event) = self.peek() {
            if matches!(event, Event::End(_)) {
                self.cursor += 1;
                return;
            }

            self.node(out);
        }
    }

    fn capture(&mut self) -> String {
        let mut buffer = String::new();
        self.children(&mut buffer);
        buffer
    }

    /// Collects the plain text of a container, discarding any inline markup.
    fn capture_text(&mut self) -> String {
        let mut text = String::new();

        while self.cursor < self.events.len() {
            let event = self.events[self.cursor].clone();
            self.cursor += 1;

            match event {
                Event::End(_) => break,
                Event::Text(value) | Event::Code(value) => text.push_str(&value),
                _ => {}
            }
        }

        text
    }

    fn node(
        &mut self,
        out: &mut String,
    ) {
        let event = self.events[self.cursor].clone();
        self.cursor += 1;

        match event {
            Event::Start(tag) => self.start(tag, out),
            Event::End(_) => {}
            Event::Text(text) => write_inline(out, &text),
            Event::Code(code) => out.push_str(&format!("#inline-code({})", literal(&code))),
            Event::InlineMath(tex) => out.push_str(&inline_math(&tex)),
            Event::DisplayMath(tex) => out.push_str(&display_math(&tex)),
            // The browser build rendered raw HTML; Typst has no HTML engine.
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::FootnoteReference(name) => self.footnote_reference(&name, out),
            // `marked` ran with `breaks: true`, so a single newline broke the line.
            Event::SoftBreak | Event::HardBreak => out.push_str("#linebreak()"),
            Event::Rule => out.push_str("#doc-rule()\n\n"),
            Event::TaskListMarker(_) => {}
        }
    }

    fn start(
        &mut self,
        tag: Tag<'_>,
        out: &mut String,
    ) {
        match tag {
            Tag::Paragraph => {
                let inner = self.capture();
                if !inner.trim().is_empty() {
                    out.push_str(&inner);
                    out.push_str("\n\n");
                }
            }

            Tag::Heading { level, .. } => {
                let inner = self.capture();
                let level = heading_level(level);
                out.push_str(&format!("#heading(level: {level})[{inner}]\n\n"));

                if level == 1 && !self.properties_emitted && !self.properties.is_empty() {
                    out.push_str(&self.properties);
                    self.properties_emitted = true;
                }
            }

            Tag::BlockQuote(_) => self.block_quote(out),

            Tag::CodeBlock(kind) => {
                let tag = match &kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_owned()
                    }
                    CodeBlockKind::Indented => String::new(),
                };

                let code = self.capture_text();
                out.push_str(&code_block(&tag, code.trim_end_matches('\n')));
            }

            Tag::List(start) => self.list(start, out),
            Tag::Item => {
                // Items are consumed by `list`; a stray one renders bare.
                let inner = self.capture();
                out.push_str(&inner);
            }

            Tag::Table(alignments) => self.table(&alignments, out),
            Tag::TableHead | Tag::TableRow | Tag::TableCell => {
                let inner = self.capture();
                out.push_str(&inner);
            }

            Tag::FootnoteDefinition(name) => {
                // Trailing paragraph break trimmed so the backlink stays on
                // the note's last line, as `.footnotes li p { display: inline }` did.
                let body = self.capture();
                self.footnote_bodies
                    .insert(name.to_string(), body.trim_end().to_owned());
            }

            Tag::Emphasis => {
                let inner = self.capture();
                out.push_str(&format!("#emph[{inner}]"));
            }
            Tag::Strong => {
                let inner = self.capture();
                out.push_str(&format!("#strong[{inner}]"));
            }
            Tag::Strikethrough => {
                let inner = self.capture();
                out.push_str(&format!("#strike[{inner}]"));
            }

            Tag::Link { dest_url, .. } => {
                let inner = self.capture();
                out.push_str(&format!("#doc-link({})[{inner}]", literal(&dest_url)));
            }

            Tag::Image { dest_url, .. } => {
                let alt = self.capture_text();
                self.image(&dest_url, &alt, out);
            }

            Tag::HtmlBlock => {
                self.children(&mut String::new());
            }

            other => {
                self.warnings
                    .push(format!("unsupported markdown element: {other:?}"));
                self.children(out);
            }
        }
    }

    fn image(
        &mut self,
        source: &str,
        alt: &str,
        out: &mut String,
    ) {
        match self.images.resolve(source) {
            Ok(path) => {
                // A caption only earns its place when it says more than the filename.
                if alt.is_empty() || alt == source {
                    out.push_str(&format!("#doc-image({})\n\n", literal(&path)));
                } else {
                    out.push_str(&format!(
                        "#doc-figure({}, {})\n\n",
                        literal(&path),
                        literal(alt),
                    ));
                }
            }
            // The reader deserves to see the hole, not just the terminal.
            Err(reason) => {
                out.push_str(&format!("#doc-missing({})\n\n", literal(&reason)));
                self.warnings.push(reason);
            }
        }
    }

    fn block_quote(
        &mut self,
        out: &mut String,
    ) {
        let Some((kind, title)) = self.take_callout_marker() else {
            let inner = self.capture();
            out.push_str(&format!("#note-quote[{inner}]\n\n"));
            return;
        };

        // The marker was consumed from inside the first paragraph, so the
        // first capture closes that paragraph and the second drains the rest
        // of the blockquote: its lists, code blocks and nested quotes.
        let mut inner = self.capture();
        if !inner.trim().is_empty() {
            inner.push_str("\n\n");
        }
        inner.push_str(&self.capture());

        let (kind, title) = (literal(&kind), literal(&title));

        if inner.trim().is_empty() {
            out.push_str(&format!("#callout({kind}, {title}, none)\n\n"));
        } else {
            out.push_str(&format!("#callout({kind}, {title})[{inner}]\n\n"));
        }
    }

    /// Detects a leading `[!kind] title`. pulldown-cmark splits the marker
    /// across several text events, so the run has to be reassembled first.
    fn take_callout_marker(&mut self) -> Option<(String, String)> {
        if !matches!(self.peek(), Some(Event::Start(Tag::Paragraph))) {
            return None;
        }

        let mut index = self.cursor + 1;
        let mut run = String::new();

        while let Some(Event::Text(text)) = self.events.get(index) {
            run.push_str(text);
            index += 1;
        }

        let captures = CALLOUT.captures(&run)?;
        let kind = captures[1].to_ascii_lowercase();
        let title = captures[2].trim();

        let title = if title.is_empty() {
            let mut characters = kind.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().chain(characters).collect(),
                None => String::new(),
            }
        } else {
            title.to_owned()
        };

        self.cursor = index;

        // Drop the break that separated the marker from the body.
        if matches!(self.peek(), Some(Event::SoftBreak | Event::HardBreak)) {
            self.cursor += 1;
        }

        Some((kind, title))
    }

    fn list(
        &mut self,
        start: Option<u64>,
        out: &mut String,
    ) {
        let mut items = Vec::new();
        let mut loose = false;

        while let Some(event) = self.peek() {
            match event {
                Event::End(_) => {
                    self.cursor += 1;
                    break;
                }
                Event::Start(Tag::Item) => {
                    self.cursor += 1;

                    let checked = match self.peek() {
                        Some(Event::TaskListMarker(state)) => {
                            let state = *state;
                            self.cursor += 1;
                            Some(state)
                        }
                        _ => None,
                    };

                    loose |= matches!(self.peek(), Some(Event::Start(Tag::Paragraph)));
                    items.push((checked, self.capture()));
                }
                _ => {
                    self.cursor += 1;
                }
            }
        }

        let tight = !loose;

        if !items.is_empty() && items.iter().all(|(checked, _)| checked.is_some()) {
            let entries: String = items
                .iter()
                .map(|(checked, body)| format!("({}, [{body}]), ", checked.unwrap()))
                .collect();

            out.push_str(&format!("#task-list({tight}, ({entries}))\n\n"));
            return;
        }

        let entries: String = items
            .iter()
            .map(|(checked, body)| match checked {
                Some(state) => format!("[#checkbox({state}) {body}], "),
                None => format!("[{body}], "),
            })
            .collect();

        match start {
            Some(first) => out.push_str(&format!(
                "#enum(tight: {tight}, start: {first}, {entries})\n\n"
            )),
            None => out.push_str(&format!("#list(tight: {tight}, {entries})\n\n")),
        }
    }

    fn table(
        &mut self,
        alignments: &[Alignment],
        out: &mut String,
    ) {
        let mut rows: Vec<Vec<String>> = Vec::new();

        while let Some(event) = self.peek() {
            match event {
                Event::End(_) => {
                    self.cursor += 1;
                    break;
                }
                Event::Start(Tag::TableHead | Tag::TableRow) => {
                    self.cursor += 1;
                    rows.push(self.table_row());
                }
                _ => {
                    self.cursor += 1;
                }
            }
        }

        let aligns: String = alignments
            .iter()
            .map(|alignment| match alignment {
                Alignment::Center => "center, ",
                Alignment::Right => "right, ",
                Alignment::Left | Alignment::None => "left, ",
            })
            .collect();

        let cells: String = rows
            .iter()
            .map(|row| {
                let row: String = row.iter().map(|cell| format!("[{cell}], ")).collect();
                format!("  ({row}),\n")
            })
            .collect();

        out.push_str(&format!("#doc-table(({aligns}), (\n{cells}))\n\n"));
    }

    fn table_row(&mut self) -> Vec<String> {
        let mut cells = Vec::new();

        while let Some(event) = self.peek() {
            match event {
                Event::End(_) => {
                    self.cursor += 1;
                    break;
                }
                Event::Start(Tag::TableCell) => {
                    self.cursor += 1;
                    cells.push(self.capture());
                }
                _ => {
                    self.cursor += 1;
                }
            }
        }

        cells
    }

    fn footnote_reference(
        &mut self,
        name: &str,
        out: &mut String,
    ) {
        // pulldown-cmark only raises a reference once it has seen the matching
        // definition, so this guard just keeps a dangling Typst label from
        // ever reaching the compiler.
        if !self.defined_footnotes.contains(name) {
            self.warnings
                .push(format!("footnote `{name}` has no definition"));
            push_literal(out, &format!("[^{name}]"));
            return;
        }

        let next = self.footnote_numbers.len() + 1;
        let number = *self.footnote_numbers.entry(name.to_owned()).or_insert(next);

        // Only the first reference anchors the label the backlink jumps to.
        let first = number == next;
        if first {
            self.footnote_order.push(name.to_owned());
        }

        out.push_str(&format!("#fn-ref({number}, {first})"));
    }

    fn footnotes_section(&mut self) -> String {
        let mut entries: Vec<String> = self
            .footnote_order
            .iter()
            .filter_map(|name| {
                let number = self.footnote_numbers.get(name)?;
                let body = self.footnote_bodies.get(name)?;
                Some(format!(
                    "  (number: {number}, backref: true, body: [{body}]),\n"
                ))
            })
            .collect();

        // Definitions nobody pointed at still belong in the list, without a backlink.
        let mut orphans: Vec<&String> = self
            .footnote_bodies
            .keys()
            .filter(|name| !self.footnote_numbers.contains_key(*name))
            .collect();
        orphans.sort();

        let mut number = self.footnote_numbers.len();
        for name in orphans {
            number += 1;
            let body = &self.footnote_bodies[name];
            entries.push(format!(
                "  (number: {number}, backref: false, body: [{body}]),\n"
            ));
        }

        if entries.is_empty() {
            return String::new();
        }

        format!("#footnotes-section((\n{}))\n", entries.concat())
    }
}

// ─────────────────────────────────────────────────────────────
// Inline text
// ─────────────────────────────────────────────────────────────

/// Expands Obsidian's `==highlight==` and `%%comment%%` spans, then hashtags.
/// Code never reaches here: it arrives as its own event.
fn write_inline(
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

// ─────────────────────────────────────────────────────────────
// Code and math
// ─────────────────────────────────────────────────────────────

/// Resolves a fence's language the way highlight.js did: exact match first,
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

fn code_block(
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

fn inline_math(tex: &str) -> String {
    match tex2typst_rs::tex2typst(tex.trim()) {
        Ok(typst) => format!("${typst}$"),
        Err(_) => format!("#({})", literal(tex.trim())),
    }
}

fn display_math(tex: &str) -> String {
    match tex2typst_rs::tex2typst(tex.trim()) {
        Ok(typst) => format!("#math-block($ {typst} $)"),
        Err(_) => format!("#math-block[#({})]", literal(tex.trim())),
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
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

    fn body(markdown: &str) -> String {
        render(markdown, Path::new("."), &[]).body
    }

    #[test]
    fn literals_escape_quotes_and_backslashes() {
        assert_eq!(literal(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(literal("line\nnext"), r#""line\nnext""#);
    }

    #[test]
    fn wikilinks_become_markdown_images() {
        assert_eq!(preprocess("![[a/b/c.png]]"), "![c.png](<a/b/c.png>)");
        assert_eq!(preprocess(r#"![["file.png"]]"#), "![file.png](<file.png>)");
        assert_eq!(preprocess(r#"!["alt"]("a%20b.png")"#), "![alt](<a b.png>)");
    }

    /// Obsidian attachments carry spaces. A bare destination would make
    /// `![a](my file.png)` parse as text, never as an image.
    #[test]
    fn attachment_names_with_spaces_still_parse_as_images() {
        assert_eq!(
            preprocess("![[Pasted image 20260326170345.png]]"),
            "![Pasted image 20260326170345.png](<Pasted image 20260326170345.png>)",
        );

        let rendered = render("![[Pasted image 1.png]]\n", Path::new("."), &[]);
        assert!(
            rendered
                .warnings
                .iter()
                .any(|w| w.contains("image not found")),
            "a spaced attachment must reach the image resolver, got {:?}",
            rendered.warnings,
        );
    }

    #[test]
    fn wikilinks_inside_code_are_left_alone() {
        let source = "```\n![[keep.png]]\n```\n";
        assert_eq!(preprocess(source), source);
        assert_eq!(preprocess("`![[keep.png]]`"), "`![[keep.png]]`");
    }

    /// The load-bearing safety property: nothing from the source text can land
    /// in the output as live Typst syntax. Strip every `#("..")` literal and
    /// no metacharacter may remain.
    #[test]
    fn typst_metacharacters_stay_inside_string_literals() {
        // Escaped in markdown, so these reach the renderer as ordinary text.
        // `C#` is deliberate: a hash that starts no tag must stay literal.
        let out = body(r"a \$dollar\$ and \[bracket\] and \\ slash and C\# sharp");

        let literals = Regex::new(r#"#\("(?:[^"\\]|\\.)*"\)"#).unwrap();
        let outside = literals.replace_all(&out, "");

        for metacharacter in ['$', '[', ']', '\\', '#'] {
            assert!(
                !outside.contains(metacharacter),
                "a bare `{metacharacter}` escaped into Typst markup: {outside:?}",
            );
        }

        assert!(out.contains("dollar") && out.contains("bracket") && out.contains("slash"));
        assert!(!out.contains("#math-block"));
    }

    #[test]
    fn currency_is_not_treated_as_math() {
        let out = body("Should not be latex: $10 to $20\n");
        assert!(
            out.contains(r#"#("$")"#),
            "the dollar sign should be literal text"
        );
        assert!(!out.contains("math-block"));
    }

    #[test]
    fn code_fences_keep_obsidian_syntax_verbatim() {
        let out = body("```\n==no==\n%%no%%\n$no$\n```\n");
        assert!(!out.contains("doc-highlight"));
        assert!(!out.contains("doc-comment"));
        assert!(out.contains(r#"raw(block: true, "==no==\n%%no%%\n$no$")"#));
    }

    #[test]
    fn highlights_and_comments_become_helpers() {
        assert!(body("==hot==\n").contains(r#"#doc-highlight[#("hot")]"#));
        assert!(body("%% quiet %%\n").contains(r#"#doc-comment[#(" quiet ")]"#));
    }

    #[test]
    fn an_unpaired_delimiter_stays_literal() {
        let out = body("a == b\n");
        assert!(!out.contains("doc-highlight"));
        assert!(out.contains("=="));
    }

    #[test]
    fn hashtags_become_pills_but_headings_do_not() {
        assert!(body("#tag/miaow\n").contains(r#"#doc-tag("tag/miaow")"#));
        assert!(!body("C# is a language\n").contains("doc-tag"));
        assert!(!body("\\# Not a heading\n").contains("doc-tag"));
    }

    #[test]
    fn callout_markers_are_parsed_across_split_text_events() {
        assert!(body("> [!note]\n> body\n").contains(r#"#callout("note", "Note")"#));
        assert!(body("> plain\n").contains("#note-quote["));
    }

    #[test]
    fn a_callout_with_only_a_title_carries_no_body() {
        assert!(body("> [!QUOTE] A title\n").contains(r#"#callout("quote", "A title", none)"#));
    }

    /// The byte range of the callout's content block, found by matching
    /// brackets. Safe here because the fixture holds no bracket in its text.
    fn callout_content_span(out: &str) -> (usize, usize) {
        let open = out.find("#callout(").expect("no callout emitted");
        let start = open + out[open..].find('[').expect("no content block");

        let mut depth = 0usize;
        for (offset, character) in out[start..].char_indices() {
            match character {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return (start, start + offset);
                    }
                }
                _ => {}
            }
        }

        panic!("the callout content block is never closed");
    }

    /// Everything after the marker's paragraph belongs to the callout, not to
    /// the document. The marker is consumed mid-paragraph, so the blockquote
    /// has to be drained in two passes.
    #[test]
    fn a_callout_keeps_every_block_that_follows_its_marker() {
        let out = body(
            "> [!note]\n> **Important:**\n>\n> - Item 1\n>\n> ```js\n> log();\n> ```\n> > nested\n",
        );
        let (start, end) = callout_content_span(&out);

        for fragment in ["#strong[", "#list(", "#code-block(", "#note-quote["] {
            let at = out
                .find(fragment)
                .unwrap_or_else(|| panic!("{fragment} missing"));
            assert!(start < at && at < end, "{fragment} escaped the callout");
        }
    }

    #[test]
    fn language_tags_resolve_the_way_highlight_js_did() {
        assert_eq!(resolve_language("python").as_deref(), Some("python"));
        assert_eq!(resolve_language("python-repl").as_deref(), Some("python"));
        assert_eq!(resolve_language("").as_deref(), None);
        assert_eq!(resolve_language("not-a-language").as_deref(), None);
    }

    #[test]
    fn an_unknown_language_still_labels_the_block() {
        let out = body("```wat\ncode\n```\n");
        assert!(out.contains(r#"#code-block("wat", raw(block: true, "code"))"#));
    }

    #[test]
    fn task_lists_render_as_checkboxes() {
        let out = body("- [ ] todo\n- [x] done\n");
        assert!(out.contains("#task-list(true, ((false, ["));
        assert!(out.contains("(true, ["));
    }

    #[test]
    fn ordered_lists_keep_their_start_number() {
        assert!(body("3. three\n4. four\n").contains("#enum(tight: true, start: 3,"));
    }

    #[test]
    fn footnotes_are_numbered_by_reference_order() {
        let out = body("Text[^b] more[^a]\n\n[^a]: first\n[^b]: second\n");

        assert!(
            out.contains("#fn-ref(1, true)"),
            "the first reference is footnote 1"
        );
        assert!(out.contains("#fn-ref(2, true)"));
        assert!(out.contains("(number: 1, backref: true, body: [#(\"second\")"));
    }

    #[test]
    fn a_repeated_reference_anchors_only_once() {
        let out = body("a[^x] b[^x]\n\n[^x]: note\n");

        assert!(out.contains("#fn-ref(1, true)"));
        assert!(
            out.contains("#fn-ref(1, false)"),
            "the second reference must not re-label"
        );
    }

    /// An undefined reference must never emit `fn-ref`, which would link to a
    /// Typst label that was never placed and fail the compile.
    #[test]
    fn a_reference_without_a_definition_never_emits_a_link() {
        let rendered = render("dangling[^gone]\n", Path::new("."), &[]);

        assert!(!rendered.body.contains("#fn-ref"));
        assert!(!rendered.body.contains("#footnotes-section"));
        assert!(
            rendered.body.contains("^gone"),
            "the text should survive verbatim"
        );
    }

    #[test]
    fn an_unreferenced_definition_still_appears_without_a_backlink() {
        let out = body("text\n\n[^loose]: orphan\n");
        assert!(out.contains("(number: 1, backref: false,"));
    }

    /// An HTML block is one opaque event, so all of it goes. Inline tags are
    /// separate events from the words they wrap, so only the tags go.
    #[test]
    fn raw_html_is_dropped_but_inline_text_survives() {
        let out = body("<div style=\"color: red\">gone</div>\n\ntext <b>bold</b>\n");

        assert!(
            !out.contains("div"),
            "the html block should vanish entirely"
        );
        assert!(
            !out.contains("gone"),
            "text inside an html block goes with it"
        );
        assert!(!out.contains("<b>"));
        assert!(out.contains(r#"#("text ")"#));
        assert!(
            out.contains(r#"#("bold")"#),
            "words between inline tags are real text"
        );
    }

    #[test]
    fn a_missing_image_leaves_a_placeholder_rather_than_a_broken_reference() {
        let rendered = render("![[no-such-file.png]]\n", Path::new("."), &[]);

        assert!(
            rendered
                .body
                .contains(r#"#doc-missing("image not found: no-such-file.png")"#)
        );
        assert!(
            rendered
                .warnings
                .iter()
                .any(|w| w.contains("image not found"))
        );
        assert!(rendered.files.is_empty());
    }

    /// Every embed the converter turns down says so in the document, and says
    /// why. Silence would read as "there was nothing here".
    #[test]
    fn an_unrenderable_embed_names_its_reason_in_the_document() {
        let cases = [
            (
                "![[clip.mp4]]\n",
                "video embeds are not supported: clip.mp4",
            ),
            (
                "![[song.mp3]]\n",
                "audio embeds are not supported: song.mp3",
            ),
            (
                "![[Another Note]]\n",
                "note embeds are not supported: Another Note",
            ),
            (
                "![[sheet.xlsx]]\n",
                ".xlsx is not an embeddable image: sheet.xlsx",
            ),
            (
                "![a](https://x.test/a.png)\n",
                "remote image not embedded: https://x.test/a.png",
            ),
        ];

        for (source, reason) in cases {
            let rendered = render(source, Path::new("."), &[]);

            assert!(
                rendered
                    .body
                    .contains(&format!("#doc-missing({})", literal(reason))),
                "{source} produced {:?}",
                rendered.body,
            );
            assert_eq!(rendered.warnings, vec![reason.to_owned()]);
        }
    }

    #[test]
    fn math_is_converted_to_typst() {
        assert!(body("Inline: $E = mc^2$\n").contains("$E = m c^2$"));
        assert!(
            body("$$\n\\int_0^1 x^2 dx\n$$\n").contains("#math-block($ integral_0^1 x^2 d x $)"),
            "the LaTeX integral should become Typst's `integral`",
        );
    }

    #[test]
    fn unconvertible_math_falls_back_to_literal_text() {
        let out = body("$\\undefinedmacro{x}$\n");
        assert!(
            !out.contains("$\\"),
            "raw LaTeX must not reach Typst as math"
        );
    }

    #[test]
    fn properties_land_after_the_first_heading() {
        let properties = [Property {
            label: "Title".to_owned(),
            value: PropertyValue::Text("T".to_owned()),
        }];

        let out = render("# Head\n\nbody\n", Path::new("."), &properties).body;
        let heading = out.find("#heading(level: 1)").unwrap();
        let block = out.find("#properties-block").unwrap();
        let text = out.find(r#"#("body")"#).unwrap();

        assert!(heading < block && block < text);
    }

    #[test]
    fn properties_lead_a_note_that_has_no_heading() {
        let properties = [Property {
            label: "Title".to_owned(),
            value: PropertyValue::Text("T".to_owned()),
        }];

        let out = render("body\n", Path::new("."), &properties).body;
        assert!(out.starts_with("#properties-block"));
    }
}
