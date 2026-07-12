//! Walks the parsed markdown once and emits the Typst body.
//!
//! Events are collected into a vector rather than streamed, because a callout
//! marker arrives split across several text events and has to be looked ahead.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Tag};
use regex::Regex;

use super::Rendered;
use super::frontmatter::Property;
use super::images::Images;
use super::inline::{code_block, display_math, heading_level, inline_math, write_inline};
use super::literal::{literal, push_literal};
use super::properties::render_properties;

/// `[!note] Optional title`, the Obsidian callout marker. The optional `-`
/// or `+` after the bracket folds the callout in Obsidian, and a PDF has
/// nothing to fold, so the sign is matched only to keep it out of the title.
static CALLOUT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[!([A-Za-z-]+)\][-+]?\s*(.*)$").unwrap());

pub(super) struct Renderer<'a> {
    events: Vec<Event<'a>>,
    cursor: usize,
    images: Images,
    warnings: Vec<String>,
    defined_footnotes: HashSet<String>,
    /// Name to number, in first reference order. The section derives its
    /// ordering from the numbers, so no second structure has to stay in step.
    footnote_numbers: HashMap<String, usize>,
    footnote_bodies: HashMap<String, String>,
    properties: String,
    properties_emitted: bool,
}

impl<'a> Renderer<'a> {
    pub(super) fn new(
        events: Vec<Event<'a>>,
        base_dir: &Path,
        properties: &[Property],
    ) -> Self {
        // A reference only ever names a footnote the parser has already seen
        // defined. Knowing them up front keeps a dangling Typst label, which
        // would fail the compile, from ever being written.
        let defined_footnotes = events
            .iter()
            .filter_map(|event| match event {
                Event::Start(Tag::FootnoteDefinition(name)) => Some(name.to_string()),
                _ => None,
            })
            .collect();

        Self {
            events,
            cursor: 0,
            images: Images::new(base_dir),
            warnings: Vec::new(),
            defined_footnotes,
            footnote_numbers: HashMap::new(),
            footnote_bodies: HashMap::new(),
            properties: render_properties(properties),
            properties_emitted: false,
        }
    }

    pub(super) fn run(mut self) -> Rendered {
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
    /// Inline containers nest, so only the `End` that closes the outer one
    /// stops the capture. Breaking on the first `End` of any kind would cut
    /// an alt like `an *italic* caption` short at the emphasis.
    fn capture_text(&mut self) -> String {
        let mut text = String::new();
        let mut depth = 0usize;

        while self.cursor < self.events.len() {
            let event = self.take();

            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                Event::Text(value) | Event::Code(value) => text.push_str(&value),
                _ => {}
            }
        }

        text
    }

    /// Consumes the event under the cursor, moving it out rather than cloning
    /// it, since every event is visited exactly once.
    fn take(&mut self) -> Event<'a> {
        let event = std::mem::replace(&mut self.events[self.cursor], Event::Rule);
        self.cursor += 1;
        event
    }

    fn node(
        &mut self,
        out: &mut String,
    ) {
        let event = self.take();

        match event {
            Event::Start(tag) => self.start(tag, out),
            Event::End(_) => {}
            Event::Text(text) => write_inline(out, &text),
            Event::Code(code) => out.push_str(&format!("#inline-code({})", literal(&code))),
            Event::InlineMath(tex) => out.push_str(&inline_math(&tex)),
            Event::DisplayMath(tex) => out.push_str(&display_math(&tex)),
            // The browser build rendered raw HTML. Typst has no HTML engine.
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
                // Items are consumed by `list`, so a stray one renders bare.
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
        // of the blockquote, meaning its lists, code blocks and nested quotes.
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
            super::capitalize(&kind)
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
                        // A loose item wraps its content in a paragraph and
                        // the marker sits inside it. It is read here and left
                        // in place, where the walk's no-op arm drops it.
                        Some(Event::Start(Tag::Paragraph)) => {
                            match self.events.get(self.cursor + 1) {
                                Some(Event::TaskListMarker(state)) => Some(*state),
                                _ => None,
                            }
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

        out.push_str(&format!("#fn-ref({number}, {first})"));
    }

    fn footnotes_section(&mut self) -> String {
        let mut referenced: Vec<(&String, &usize)> = self.footnote_numbers.iter().collect();
        referenced.sort_by_key(|(_, number)| **number);

        let mut entries: Vec<String> = referenced
            .into_iter()
            .filter_map(|(name, number)| {
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
