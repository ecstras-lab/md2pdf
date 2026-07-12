//! Splits the YAML frontmatter off a note and classifies it for the
//! properties block that renders beneath the title.

use std::sync::LazyLock;

use regex::Regex;

/// The closing fence is anchored to a whole line, so a `----` divider inside
/// the block cannot end it early and leak the rest of the YAML into the body.
static FENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---[ \t]*(?:\r?\n|\z)").unwrap());

/// Anchored at both ends, so a value that merely opens with a date, such as
/// `2026-07-12 Weekly review`, stays ordinary text instead of losing its tail.
static DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{1,2}):(\d{2})(?::\d{2})?)?\s*$").unwrap()
});

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// A frontmatter value, already resolved to the shape the block renders.
#[derive(Debug, PartialEq)]
pub enum PropertyValue {
    Tags(Vec<String>),
    Link(String),
    /// Pre-formatted for display, e.g. `5-Apr-2024 3:30 PM`.
    Date(String),
    Bool(bool),
    Text(String),
}

#[derive(Debug, PartialEq)]
pub struct Property {
    pub label: String,
    pub value: PropertyValue,
}

pub struct Frontmatter {
    pub properties: Vec<Property>,
    pub body: String,
    pub warnings: Vec<String>,
}

/// Strips a leading `---` fenced YAML block, returning it alongside the
/// remaining markdown. A block that fails to parse is still stripped, so its
/// keys never leak into the rendered document.
pub fn split(source: &str) -> Frontmatter {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);

    let Some(fence) = FENCE.captures(source) else {
        return Frontmatter {
            properties: Vec::new(),
            body: source.to_owned(),
            warnings: Vec::new(),
        };
    };

    let body = source[fence.get(0).unwrap().end()..].to_owned();
    let yaml = fence.get(1).unwrap().as_str();

    let parsed = serde_yaml::from_str::<serde_yaml::Value>(yaml);

    let Ok(serde_yaml::Value::Mapping(mapping)) = parsed else {
        return Frontmatter {
            properties: Vec::new(),
            body,
            warnings: vec!["frontmatter is not a YAML mapping; ignoring it".to_owned()],
        };
    };

    let mut properties = Vec::new();
    let mut warnings = Vec::new();

    for (key, value) in &mapping {
        let Some(key) = key.as_str() else {
            warnings.push(format!(
                "frontmatter key {key:?} is not a string; skipping it"
            ));
            continue;
        };

        match classify(value) {
            Some(value) => properties.push(Property {
                label: super::capitalize(key),
                value,
            }),
            None => warnings.push(format!("frontmatter key `{key}` has an unsupported value")),
        }
    }

    Frontmatter {
        properties,
        body,
        warnings,
    }
}

fn classify(value: &serde_yaml::Value) -> Option<PropertyValue> {
    if let serde_yaml::Value::Sequence(items) = value {
        let tags = items.iter().filter_map(scalar).collect();
        return Some(PropertyValue::Tags(tags));
    }

    if let serde_yaml::Value::Bool(flag) = value {
        return Some(PropertyValue::Bool(*flag));
    }

    if value.is_null() {
        return Some(PropertyValue::Text(String::new()));
    }

    let text = scalar(value)?;

    if text.starts_with("http://") || text.starts_with("https://") {
        return Some(PropertyValue::Link(text));
    }

    if let Some(formatted) = format_date(&text) {
        return Some(PropertyValue::Date(formatted));
    }

    // A quoted `"true"` reaches us as a string, but reads as a flag.
    match text.as_str() {
        "true" => Some(PropertyValue::Bool(true)),
        "false" => Some(PropertyValue::Bool(false)),
        _ => Some(PropertyValue::Text(text)),
    }
}

fn scalar(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(text) => Some(text.clone()),
        serde_yaml::Value::Number(number) => Some(number.to_string()),
        serde_yaml::Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// Formats `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM` as `5-Apr-2024 3:30 PM`.
/// Returns `None` when the text is not a date, including one whose fields are
/// out of range. An impossible value falls back to plain text rather than
/// being reshaped into an invented moment.
fn format_date(text: &str) -> Option<String> {
    let captures = DATE.captures(text)?;
    let group = |index: usize| captures.get(index).map(|m| m.as_str());

    let year = group(1)?;
    let month = group(2)?.parse::<usize>().ok()?;
    let day = group(3)?
        .parse::<u32>()
        .ok()
        .filter(|day| (1..=31).contains(day))?;

    let month = MONTHS.get(month.checked_sub(1)?)?;
    let date = format!("{day}-{month}-{year}");

    let (Some(hours), Some(minutes)) = (group(4), group(5)) else {
        return Some(date);
    };

    let hours = hours.parse::<u32>().ok().filter(|hours| *hours < 24)?;
    minutes
        .parse::<u32>()
        .ok()
        .filter(|minutes| *minutes < 60)?;

    let meridiem = if hours >= 12 { "PM" } else { "AM" };
    let hours = match hours % 12 {
        0 => 12,
        other => other,
    };

    Some(format!("{date} {hours}:{minutes} {meridiem}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_frontmatter_leaves_the_body_untouched() {
        let parsed = split("# Title\n\nBody\n");

        assert!(parsed.properties.is_empty());
        assert_eq!(parsed.body, "# Title\n\nBody\n");
    }

    #[test]
    fn frontmatter_is_stripped_and_key_order_is_preserved() {
        let parsed = split("---\nzebra: 1\nalpha: 2\nmiddle: 3\n---\n# Title\n");

        assert_eq!(parsed.body, "# Title\n");
        let labels: Vec<&str> = parsed.properties.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels, ["Zebra", "Alpha", "Middle"]);
    }

    /// A `----` line inside the block is YAML, not the closing fence. Ending
    /// there would leak the rest of the frontmatter into the document.
    #[test]
    fn a_dashed_line_inside_the_block_does_not_close_the_fence() {
        let parsed = split("---\ntitle: x\n----\nkey: y\n---\nBody\n");

        assert_eq!(parsed.body, "Body\n");
        assert!(!parsed.body.contains("key"));
    }

    /// The block may end at the last line of the note, with no newline after
    /// the closing fence.
    #[test]
    fn a_fence_at_the_end_of_the_note_still_closes() {
        let parsed = split("---\ntitle: x\n---");

        assert_eq!(parsed.properties.len(), 1);
        assert_eq!(parsed.body, "");
    }

    /// A value that merely opens with a date keeps its full text. Truncating
    /// `2026-07-12 Weekly review` to the date would silently discard content.
    #[test]
    fn text_that_opens_with_a_date_is_not_a_date() {
        let parsed = split("---\ntopic: 2026-07-12 Weekly review\n---\n");

        assert_eq!(
            parsed.properties[0].value,
            PropertyValue::Text("2026-07-12 Weekly review".into())
        );
    }

    /// An impossible moment falls back to plain text rather than being
    /// reshaped into an invented one.
    #[test]
    fn impossible_dates_and_times_stay_plain_text() {
        for value in ["2024-01-99", "2024-04-05T25:30", "2024-04-05T12:70"] {
            let parsed = split(&format!("---\nwhen: {value}\n---\n"));

            assert_eq!(
                parsed.properties[0].value,
                PropertyValue::Text(value.into()),
                "{value} should not classify as a date",
            );
        }
    }

    #[test]
    fn values_are_classified_by_shape() {
        let parsed = split(concat!(
            "---\n",
            "title: Obsidian Markdown Test File\n",
            "tags:\n  - test\n  - markdown\n",
            "created: 2026-03-26\n",
            "stamped: 2024-04-05T15:30:00\n",
            "home: https://example.com\n",
            "Tickbox: true\n",
            "quoted: \"false\"\n",
            "blank:\n",
            "---\n",
        ));

        let value = |label: &str| {
            &parsed
                .properties
                .iter()
                .find(|p| p.label == label)
                .unwrap()
                .value
        };

        assert_eq!(
            value("Title"),
            &PropertyValue::Text("Obsidian Markdown Test File".into())
        );
        assert_eq!(
            value("Tags"),
            &PropertyValue::Tags(vec!["test".into(), "markdown".into()])
        );
        assert_eq!(value("Created"), &PropertyValue::Date("26-Mar-2026".into()));
        assert_eq!(
            value("Stamped"),
            &PropertyValue::Date("5-Apr-2024 3:30 PM".into())
        );
        assert_eq!(
            value("Home"),
            &PropertyValue::Link("https://example.com".into())
        );
        assert_eq!(value("Tickbox"), &PropertyValue::Bool(true));
        assert_eq!(value("Quoted"), &PropertyValue::Bool(false));
        assert_eq!(value("Blank"), &PropertyValue::Text(String::new()));
    }

    #[test]
    fn midnight_and_noon_use_twelve_hour_clock() {
        assert_eq!(
            format_date("2024-04-05T00:07").unwrap(),
            "5-Apr-2024 12:07 AM"
        );
        assert_eq!(
            format_date("2024-04-05T12:00").unwrap(),
            "5-Apr-2024 12:00 PM"
        );
        assert_eq!(
            format_date("2024-04-05T13:05").unwrap(),
            "5-Apr-2024 1:05 PM"
        );
        assert_eq!(
            format_date("2024-04-05 09:30").unwrap(),
            "5-Apr-2024 9:30 AM"
        );
    }

    #[test]
    fn non_dates_are_rejected() {
        assert!(format_date("not a date").is_none());
        assert!(format_date("2024-13-01").is_none(), "month 13 has no name");
        assert!(format_date("2024-00-01").is_none(), "month 0 has no name");
    }

    #[test]
    fn a_byte_order_mark_does_not_shift_the_body() {
        let parsed = split("\u{feff}---\ntitle: T\n---\n# Heading\n");

        assert_eq!(parsed.body, "# Heading\n");
        assert_eq!(parsed.properties.len(), 1);
    }

    #[test]
    fn unparseable_frontmatter_is_still_stripped() {
        let parsed = split("---\n\tthis: [is not: valid\n---\n# Title\n");

        assert_eq!(parsed.body, "# Title\n");
        assert!(parsed.properties.is_empty());
        assert!(
            !parsed.warnings.is_empty(),
            "a parse failure should be reported"
        );
    }
}
