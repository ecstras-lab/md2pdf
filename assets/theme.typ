// The document's style rules.
//
// Lengths are the stylesheet's pixels converted at 0.75pt per px, so 1rem
// (16px) is 12pt. `palette`, `marker-colors`, `heading-rules`, `callouts` and
// `syntax-theme` are bound by the generated preamble that precedes this file.

// Montserrat carries no ornaments, so DejaVu Sans backs it for ✦ ※ ⚙ ↩ ◦ ▪.
#let sans = ("Montserrat", "DejaVu Sans")
#let mono = ("JetBrains Mono", "DejaVu Sans")
#let ornament = "DejaVu Sans"

// Vertical metrics of the two text faces, read from their `hhea` tables.
#let sans-ascender = 0.968em
#let sans-descender = 0.251em
#let mono-ascender = 1.02em
#let mono-descender = 0.3em

/// Reproduces a CSS line box.
///
/// Typst measures a text frame from cap height down to the baseline, which for
/// Montserrat is 0.7em. CSS measures the whole line box, splitting the leftover
/// half leading evenly above and below the font's own extents. Every margin in
/// this stylesheet is a CSS pixel value, so the frame has to agree with CSS or
/// each one lands against the wrong edge.
#let line-box(
  ascender,
  descender,
  height,
) = {
  let half = (height - ascender - descender) / 2
  (top-edge: ascender + half, bottom-edge: -(descender + half))
}

#let sans-line(height) = line-box(
  sans-ascender,
  sans-descender,
  height,
)

#let mono-line(height) = line-box(
  mono-ascender,
  mono-descender,
  height,
)

// ── Page and base typography ─────────────────────────────────

// 800pt wide is the 800px viewport the browser build measured itself against.
// `height: auto` yields the single uninterrupted page it used to emit.
#set page(
  width: 600pt,
  height: auto,
  margin: (x: 30pt, y: 21pt),
  fill: palette.background,
)

#set text(
  font: sans,
  size: 12pt,
  fill: palette.foreground,
  weight: 400,
  tracking: -0.12pt,
  lang: "en",
  ..sans-line(1.3em),
)

// The line box already carries the leading, so lines simply stack.
#set par(
  leading: 0pt,
  spacing: 12pt,
  justify: false,
)

// `marked` never curled quotes, and neither did the browser build.
#set smartquote(enabled: false)

#set raw(
  theme: syntax-theme,
  tab-size: 2,
)
#show raw: set text(font: mono)
#show raw.where(block: true): set text(
  size: 10.2pt,
  fill: palette.code,
  ..mono-line(1.4em),
)

// ── Headings ─────────────────────────────────────────────────

// `.centered` outranks `h1`, so its top margin is 0.5rem. CSS then collapses
// that against the preceding element's bottom margin, which is at least the
// 1rem of a paragraph. Typst has no collapse and ignores paragraph spacing
// before a block, so `above` carries the collapsed value directly.
//
// The insets run half a stroke wide because Typst centres a stroke on the
// block edge rather than growing the box, as a CSS border does.
#show heading.where(level: 1): it => block(
  width: 100%,
  above: 12pt,
  below: 30pt,
  inset: (y: 6.75pt),
  stroke: (top: 1.5pt + palette.border, bottom: 1.5pt + palette.border),
)[
  #set align(center)
  #set text(
    size: 30pt,
    weight: 700,
    fill: palette.heading,
    tracking: -0.45pt,
    ..sans-line(1.2em),
  )
  #it.body
  // The box masks the rule behind it, so the ornament sits in a gap. The
  // offset centres the glyph's ink on the rule, since DejaVu draws ✦ well
  // above the baseline. It is set larger than the stylesheet's 1rem because
  // DejaVu draws the star small within its em, and 14.6pt restores the ink
  // to roughly the 9pt the browser's fallback face produced.
  #place(
    bottom + center,
    dy: 15.26pt,
  )[
    #box(
      fill: palette.background,
      inset: (x: 9pt),
    )[
      #text(
        size: 14.6pt,
        fill: palette.primary,
        font: ornament,
      )[✦]
    ]
  ]
]

#show heading.where(level: 2): it => block(
  width: 100%,
  above: 18pt,
  below: 9pt,
  inset: (bottom: 4.8pt),
  stroke: (bottom: 0.75pt + palette.border),
)[
  #text(
    size: 21pt,
    weight: 600,
    fill: palette.heading,
    tracking: -0.525pt,
    ..sans-line(1.2em),
  )[#it.body]
]

/// A heading carried by a colored left rule that bleeds into a fading wash.
///
/// Typst centres a stroke on the box edge, while a CSS border sits inside it.
/// Pulling the drawn box in by half the rule's width puts the rule back where
/// `border-left` had it, and the inset then clears both rule and padding.
#let rule-heading(
  body,
  size,
  color,
  rule,
  thickness,
  radius,
) = block(
  width: 100%,
  above: 18pt,
  below: 9pt,
  outset: (left: -thickness / 2),
  inset: (left: 12pt + thickness),
  radius: radius,
  fill: gradient.linear(
    (rule.transparentize(90%), 0%),
    (rule.transparentize(100%), 60%),
    (rule.transparentize(100%), 100%),
    dir: ltr,
  ),
  stroke: (left: thickness + rule),
)[
  #text(
    size: size,
    weight: 600,
    fill: color,
    ..sans-line(1.2em),
  )[#body]
]

#show heading.where(level: 3): it => rule-heading(
  it.body,
  18pt,
  palette.heading-h3,
  heading-rules.at(0),
  3pt,
  3pt,
)
#show heading.where(level: 4): it => rule-heading(
  it.body,
  15pt,
  palette.heading-h4,
  heading-rules.at(1),
  2.25pt,
  2.25pt,
)
#show heading.where(level: 5): it => rule-heading(
  it.body,
  12pt,
  palette.heading-h5,
  heading-rules.at(2),
  1.5pt,
  1.5pt,
)
#show heading.where(level: 6): it => rule-heading(
  it.body,
  10.5pt,
  palette.muted-foreground,
  heading-rules.at(3),
  1.5pt,
  1.5pt,
)

// ── Lists ────────────────────────────────────────────────────

// `ul` and `ol` carry `padding-left: 1.5rem`, so each level advances the body
// by exactly 18pt. A fixed width marker box keeps that true whatever the
// marker happens to be, and right aligns it against the body the way a
// browser positions an outside list marker.
#let marker-width = 12pt
#let marker-gap = 6pt

#let marker(
  index,
  glyph,
) = box(
  width: marker-width,
  align(
    right,
    text(
      fill: marker-colors.at(index),
      weight: 600,
      font: ornament,
    )[#glyph],
  ),
)

#set list(
  indent: 0pt,
  body-indent: marker-gap,
  spacing: 3pt,
  marker: (marker(
    0,
    "•",
  ), marker(
    1,
    "◦",
  ), marker(
    2,
    "▪",
  ), marker(
    3,
    "•",
  )),
)

#let enum-numbering(..slots) = {
  let numbers = slots.pos()
  let index = calc.rem(
    numbers.len() - 1,
    4,
  )
  let patterns = ("1.", "a.", "i.", "1.")

  box(
    width: marker-width,
    align(right, text(
      fill: marker-colors.at(index),
      weight: 600,
    )[
      #numbering(
        patterns.at(index),
        numbers.last(),
      )
    ]),
  )
}

#set enum(
  numbering: enum-numbering,
  full: true,
  indent: 0pt,
  body-indent: marker-gap,
  spacing: 3pt,
)

// ── Inline elements ──────────────────────────────────────────

// `hr { height: 1px; background-color: ... }`, a filled box rather than a
// stroke, so it occupies a pixel of layout the way the original did.
#let doc-rule() = block(
  width: 100%,
  height: 0.75pt,
  above: 18pt,
  below: 18pt,
  fill: palette.rule,
)

#let doc-link(
  url,
  body,
) = link(
  url,
  text(fill: palette.primary)[#body],
)

// `.tag` is an inline flex pill. Its height is its own `line-height: 1.25`
// plus padding, not the surrounding line box, and it hangs from the baseline
// of the text inside. Left at the document's 1.3 line box it grew tall enough
// for two stacked tags to collide.
#let doc-tag(name) = {
  box(
    fill: palette.primary,
    radius: 100pt,
    inset: (x: 6pt, y: 1.5pt),
    text(
      size: 9pt,
      weight: 500,
      fill: palette.primary-foreground,
      ..sans-line(1.25em),
      "#" + name,
    ),
  )

  // Typst sizes a line to the items in it, so a line holding nothing but tags
  // would shrink to the pill and leave stacked tags touching. A zero width
  // space is a text run, and carries the paragraph's own line box back in.
  text("\u{200B}")
}

#let doc-highlight(body) = highlight(
  fill: palette.highlight,
  radius: 2.4pt,
  extent: 1.2pt,
)[#body]

#let doc-comment(body) = text(
  fill: palette.muted-foreground.transparentize(40%),
  style: "italic",
)[#body]

#let inline-code(body) = box(
  fill: palette.muted,
  radius: 3pt,
  inset: (x: 0.4em),
  outset: (y: 0.25em),
)[#text(
  size: 0.875em,
  fill: palette.code,
)[#raw(body)]]

#let math-block(body) = block(
  width: 100%,
  above: 3.6pt,
  below: 3.6pt,
)[
  #align(center)[#text(size: 1.1em)[#body]]
]

// ── Code blocks ──────────────────────────────────────────────

// The inset is the border's own width: a CSS border sits inside the box, so
// the content, and the absolutely positioned language label, begin just past it.
#let code-block(
  lang,
  body,
) = block(
  width: 100%,
  above: 15pt,
  below: 15pt,
  fill: palette.accent,
  radius: 6pt,
  stroke: 0.75pt + palette.border-style,
  clip: true,
  inset: 0.75pt,
)[
  #block(
    width: 100%,
    above: 0pt,
    below: 0pt,
    inset: (x: 12pt, y: 9pt),
  )[#body]
  #if lang != none {
    place(top + right)[
      #box(
        fill: palette.code-lang-background,
        inset: (x: 7.2pt, y: 2.4pt),
        radius: (bottom-left: 4.5pt),
        stroke: (
          left: 0.75pt + palette.border-style,
          bottom: 0.75pt + palette.border-style,
        ),
      )[
        #text(
          font: mono,
          size: 8.4pt,
          weight: 500,
          fill: palette.code-lang-foreground,
        )[#lang]
      ]
    ]
  }
]

// ── Blockquotes and callouts ─────────────────────────────────

#let note-quote(body) = block(
  width: 100%,
  above: 15pt,
  below: 15pt,
  fill: palette.secondary,
  stroke: (left: 3pt + palette.primary),
  radius: 3pt,
  outset: (left: -1.5pt),
  inset: (left: 18pt, right: 15pt, y: 9pt),
)[
  #set text(fill: palette.muted-foreground)
  #body
]

#let callout(
  kind,
  title,
  body,
) = {
  let spec = callouts.at(
    kind,
    default: callouts.at("info"),
  )
  let bare = body == none

  block(
    width: 100%,
    above: 15pt,
    below: 15pt,
    fill: spec.background,
    stroke: (left: 3pt + spec.color),
    radius: 3pt,
    clip: true,
    outset: (left: -1.5pt),
    inset: 0pt,
  )[
    #block(
      width: 100%,
      above: 0pt,
      below: 0pt,
      inset: (left: 15pt, right: 12pt, top: 9pt, bottom: if bare { 9pt } else { 6pt }),
    )[
      #grid(
        columns: (auto, 1fr),
        column-gutter: 6pt,
        align: (top, top),
        // `.callout-icon { margin-top: 0.1rem }`, plus the point or so by which
        // the browser settles a flex icon below where the box model puts it.
        // Matched against the reference render, not derived.
        move(
          dy: 2.33pt,
          image(
            spec.icon,
            width: 12pt,
          ),
        ),
        text(
          fill: spec.color,
          weight: 600,
        )[#title],
      )
    ]
    #if not bare {
      block(
        width: 100%,
        above: 0pt,
        below: 0pt,
        inset: (left: 15pt, right: 12pt, bottom: 9pt),
      )[#body]
    }
  ]
}

// ── Task lists ───────────────────────────────────────────────

#let checkbox(checked) = box(
  width: 13.2pt,
  height: 13.2pt,
  radius: 3pt,
  baseline: 2.4pt,
  fill: if checked { palette.primary } else { palette.secondary },
  stroke: 1.125pt + (if checked { palette.primary } else { palette.muted-foreground }),
)[
  #if checked {
    align(
      center + horizon,
      image(
        "/icons/check-on-primary.svg",
        width: 9pt,
      ),
    )
  }
]

#let task-list(
  tight,
  items,
) = block(
  width: 100%,
  above: 12pt,
  below: 12pt,
)[
  #for (index, entry) in items.enumerate() {
    let (checked, body) = entry
    let gap = if index == 0 { 0pt } else if tight { 3pt } else { 6pt }

    block(
      width: 100%,
      above: gap,
      below: 0pt,
    )[
      #checkbox(checked)
      #h(6pt)
      #if checked {
        text(
          fill: palette.muted-foreground,
          strike(body),
        )
      } else {
        body
      }
    ]
  }
]

// ── Tables ───────────────────────────────────────────────────

#let doc-table(
  aligns,
  rows,
) = {
  let columns = aligns.len()
  let total = rows.len()

  let cells = rows
    .enumerate()
    .map(((y, row)) => row.map(cell => {
      if y == 0 { text(
        weight: 500,
        fill: palette.card-foreground,
        cell,
      ) } else { cell }
    }))
    .flatten()

  block(
    width: 100%,
    above: 18pt,
    below: 18pt,
    radius: 6pt,
    stroke: 0.75pt + palette.border-style,
    clip: true,
    inset: 0.75pt,
  )[
    // `font-size: 0.875rem; line-height: 1.25rem`
    #set text(
      size: 10.5pt,
      ..sans-line(1.4286em),
    )
    #table(
      columns: (1fr,) * columns,
      align: aligns,
      inset: (x: 9pt, y: 7.2pt),
      stroke: (x, y) => (
        right: if x < columns - 1 { 0.75pt + palette.border },
        bottom: if y < total - 1 { 0.75pt + palette.border },
      ),
      fill: (x, y) => {
        if y == 0 { palette.secondary } else if calc.even(y) { palette.accent }
      },
      ..cells,
    )
  ]
}

// ── Images ───────────────────────────────────────────────────

/// `max-width: 100%; height: auto` — never upscale past the image's own size.
#let sized-image(path) = layout(available => {
  let natural = measure(image(path))
  let width = calc.min(
    natural.width,
    available.width,
  )

  box(
    radius: 6pt,
    clip: true,
    image(
      path,
      width: width,
    ),
  )
})

#let doc-image(path) = block(
  width: 100%,
  above: 15pt,
  below: 15pt,
)[
  #align(
    center,
    sized-image(path),
  )
]

/// Stands in for an embed that could not be drawn, so the reason lands in the
/// document rather than only on the terminal.
#let doc-missing(reason) = block(
  width: 100%,
  above: 15pt,
  below: 15pt,
  fill: palette.secondary,
  stroke: (thickness: 0.75pt, paint: palette.border, dash: "dashed"),
  radius: 6pt,
  inset: (x: 12pt, y: 9pt),
)[
  #grid(
    columns: (auto, 1fr),
    column-gutter: 7.5pt,
    align: (top, top),
    move(
      dy: 2.33pt,
      image(
        "/icons/missing.svg",
        width: 12pt,
      ),
    ),
    text(
      size: 10.5pt,
      fill: palette.muted-foreground,
      style: "italic",
      reason,
    ),
  )
]

#let doc-figure(
  path,
  caption,
) = block(
  width: 100%,
  above: 18pt,
  below: 18pt,
)[
  #align(center)[
    #sized-image(path)
    #v(
      6pt,
      weak: true,
    )
    #text(
      size: 10.5pt,
      fill: palette.muted-foreground,
      style: "italic",
    )[#caption]
  ]
]

// ── Footnotes ────────────────────────────────────────────────

/// `.footnote-ref`, a badge riding above the line.
///
/// It is drawn as highlighted text rather than a box. A box is an atom that
/// Typst grows the line to contain, so raising one either stretches the line
/// or, with `move`, drops it onto a line of its own. Raising a text baseline
/// is what `super` does, and it leaves the line alone. The edges stand in for
/// the vertical padding that `highlight` has no parameter for.
/// `.footnote-ref`, a badge riding above the line on `top: -0.5em` inside a
/// `sup`. Raising the box's baseline is the only form Typst will paint: it
/// drops the fill of anything reached through `place`, and `move` is a block.
#let fn-pill(body) = box(
  fill: palette.secondary,
  radius: 3pt,
  inset: (x: 3pt, y: 1.2pt),
  baseline: -8.8pt,
  text(
    size: 9pt,
    weight: 600,
    fill: palette.footnote-accent,
    body,
  ),
)

#let fn-ref(
  number,
  anchor,
) = {
  let target = link(
    label("fn-" + str(number)),
    fn-pill(str(number)),
  )

  if anchor {
    [#target#label("fnref-" + str(number))]
  } else {
    target
  }
}

/// `.footnote-backref`, the arrow that jumps back to the reference.
#let fn-backref(number) = link(
  label("fnref-" + str(number)),
  box(
    fill: palette.secondary,
    radius: 3pt,
    inset: (x: 2.4pt),
  )[
    #text(
      size: 9pt,
      fill: palette.footnote-accent,
      font: ornament,
    )[↩]
  ],
)

#let footnotes-section(entries) = block(
  width: 100%,
  above: 24pt,
)[
  #block(
    width: 100%,
    above: 0pt,
    below: 0pt,
    // `.footnotes { padding-top: 1rem }` plus the `ol`'s own `margin: 1rem 0`.
    inset: (top: 24pt),
    stroke: (top: 0.75pt + palette.rule),
  )[
    #place(
      top + center,
      dy: -31pt,
    )[
      #box(
        fill: palette.background,
        // Padding plus the border, which CSS counts inside the box.
        inset: (x: 10.35pt, y: 3.15pt),
        radius: 3pt,
        stroke: 1.5pt + palette.rule,
      )[#{
        // `content: '\203B notes'` ends the escape on the space that follows
        // it, so the mark butts against the word. Only DejaVu carries ※, and
        // only Montserrat has a semibold face for the word.
        set text(
          fill: palette.primary,
          weight: 600,
        )
        box(
          baseline: 1.1pt,
          text(
            size: 10.5pt,
            font: ornament,
            top-edge: "bounds",
            bottom-edge: "bounds",
          )[※],
        )
        h(0.42pt)
        text(
          size: 8.4pt,
          font: sans,
          tracking: 0.42pt,
        )[NOTES]
      }]
    ]
    // The notes are an `ol`, so `ol > li` targets each item directly and its
    // `--foreground` beats the muted colour the section would have passed down.
    // `ol > li::marker` then tints the number like any other top level list.
    #set text(
      size: 10.5pt,
      fill: palette.foreground,
    )
    #for entry in entries {
      block(
        width: 100%,
        above: 6pt,
        below: 0pt,
      )[
        #grid(
          // `.footnotes ol { padding-left: 1.25rem }`, split between the
          // hanging number and the gap before the note.
          columns: (9pt, 1fr),
          column-gutter: 6pt,
          align: (right + top, left + top),
          text(
            fill: marker-colors.at(0),
            weight: 600,
          )[#str(entry.number).],
          // Written flat: a line break here would open the note with a space,
          // and a label can only attach to the content it follows in markup.
          [#entry.body#label("fn-" + str(entry.number))#if entry.backref [#h(3.9pt)#fn-backref(entry.number)]],
        )
      ]
    }
  ]
]

// ── Properties block ─────────────────────────────────────────

// `.prop-tag` is an inline block: its padding and its border both sit inside
// the box. An `outset` would paint them without reserving the room, leaving
// the pills taller than the row that holds them.
#let prop-tags(items) = {
  let pill(item) = box(
    fill: palette.muted,
    radius: 100pt,
    inset: (x: 7.35pt, y: 1.95pt),
    stroke: 0.75pt + palette.border,
    text(
      size: 9pt,
      weight: 500,
      fill: palette.foreground,
      item,
    ),
  )

  items.map(pill).join(h(3.6pt))
}

#let prop-link(url) = link(
  url,
  text(
    size: 9.6pt,
    fill: palette.primary,
  )[#url],
)

// Built in code mode: `.prop-date` is a flex row, which drops the whitespace
// between its children, whereas a line break in markup would become a space.
#let prop-date(value) = box({
  box(
    baseline: 1.6pt,
    image(
      "/icons/calendar.svg",
      width: 10.5pt,
    ),
  )
  h(4.2pt)
  text(
    size: 10.2pt,
    fill: palette.foreground,
    value,
  )
})

#let prop-bool(flag) = box(
  width: 13.2pt,
  height: 13.2pt,
  radius: 3pt,
  baseline: 2.4pt,
  fill: if flag { palette.primary } else { palette.secondary },
  stroke: if flag { none } else { 0.75pt + palette.border },
)[
  #if flag {
    align(
      center + horizon,
      image(
        "/icons/check-on-primary.svg",
        width: 9pt,
      ),
    )
  } else {
    align(
      center + horizon,
      image(
        "/icons/cross-muted.svg",
        width: 7.5pt,
      ),
    )
  }
]

#let prop-text(value) = text(
  size: 10.2pt,
  fill: palette.foreground,
)[
  #if value == "" { "—" } else { value }
]

#let properties-block(rows) = block(
  width: 100%,
  above: 0pt,
  below: 21pt,
  radius: 6pt,
  stroke: 0.75pt + palette.border,
  fill: palette.card,
  clip: true,
  inset: 0.75pt,
)[
  #block(
    width: 100%,
    above: 0pt,
    below: 0pt,
    fill: palette.secondary,
    inset: (x: 12pt, y: 6.6pt),
    stroke: (bottom: 0.75pt + palette.border),
  )[
    // `.properties-header` is a flex row, so only its `gap` separates the
    // icon from the title. Code mode keeps a stray line break from adding one.
    #{
      box(
        baseline: 1.2pt,
        text(
          size: 10.8pt,
          fill: palette.muted-foreground.transparentize(30%),
          font: ornament,
        )[⚙],
      )
      h(6pt)
      text(
        size: 9.6pt,
        weight: 600,
        fill: palette.muted-foreground,
        tracking: 0.48pt,
      )[PROPERTIES]
    }
  ]
  #block(
    width: 100%,
    above: 0pt,
    below: 0pt,
    inset: (y: 3pt),
  )[
    #for row in rows {
      block(
        width: 100%,
        above: 0pt,
        below: 0pt,
        inset: (x: 12pt, y: 4.2pt),
      )[
        #grid(
          columns: (75pt, 1fr),
          column-gutter: 12pt,
          align: (left + top, left + top),
          text(
            size: 9.6pt,
            weight: 500,
            fill: palette.muted-foreground,
          )[#row.key],
          row.value,
        )
      ]
    }
  ]
]
