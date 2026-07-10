//! Renders the frontmatter as the properties block beneath the title.

use super::frontmatter::{Property, PropertyValue};
use super::literal::literal;

pub(super) fn render_properties(properties: &[Property]) -> String {
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
