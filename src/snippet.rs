//! Parse and fill variables in favorite text.

use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub struct Var {
    pub name: String,
    pub default: Option<String>,
}

/// List each variable in first-use order, keeping its first explicit default.
pub fn vars(text: &str) -> Vec<Var> {
    let mut defaults = defaults(text);
    placeholders(text)
        .filter_map(|placeholder| {
            let default = defaults.remove(placeholder.name)?;
            Some(Var {
                name: placeholder.name.to_owned(),
                default: default.map(str::to_owned),
            })
        })
        .collect()
}

/// Fill each occurrence from the supplied values, its name's default, or an empty string.
pub fn fill(text: &str, values: &HashMap<String, String>) -> String {
    let defaults = defaults(text);
    let mut filled = String::with_capacity(text.len());
    let mut end = 0;
    for placeholder in placeholders(text) {
        filled.push_str(&text[end..placeholder.range.start]);
        let value = values
            .get(placeholder.name)
            .map(String::as_str)
            .or(defaults[placeholder.name])
            .unwrap_or_default();
        filled.push_str(value);
        end = placeholder.range.end;
    }
    filled.push_str(&text[end..]);
    filled
}

/// Check whether the text contains at least one valid placeholder.
pub fn has_vars(text: &str) -> bool {
    placeholders(text).next().is_some()
}

pub(crate) struct Placeholder<'a> {
    pub range: Range<usize>,
    pub name: &'a str,
    default: Option<&'a str>,
}

pub(crate) fn placeholders(text: &str) -> impl Iterator<Item = Placeholder<'_>> {
    let mut end = 0;
    std::iter::from_fn(move || {
        while let Some(open) = text[end..].find("{{") {
            let start = end + open;
            let body_start = start + 2;
            let close = body_start + text[body_start..].find("}}")?;
            let body = &text[body_start..close];
            end = close + 2;

            let (name, default) = match body.split_once(':') {
                Some((name, default)) => (name, Some(default)),
                None => (body, None),
            };
            if !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
            {
                return Some(Placeholder {
                    range: start..end,
                    name,
                    default,
                });
            }
        }
        None
    })
}

fn defaults(text: &str) -> HashMap<&str, Option<&str>> {
    let mut defaults = HashMap::new();
    for placeholder in placeholders(text) {
        let default = defaults.entry(placeholder.name).or_insert(None);
        if default.is_none() {
            *default = placeholder.default;
        }
    }
    defaults
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_names_follow_first_use_and_keep_the_first_default() {
        let text =
            "{{B-2.a_0}} {{host:local}} {{B-2.a_0:one}} {{host:remote}} {{B-2.a_0:two}} {{bare}}";
        assert_eq!(
            vars(text),
            [
                Var {
                    name: "B-2.a_0".into(),
                    default: Some("one".into()),
                },
                Var {
                    name: "host".into(),
                    default: Some("local".into()),
                },
                Var {
                    name: "bare".into(),
                    default: None,
                },
            ]
        );
        assert_eq!(fill(text, &HashMap::new()), "one local one local one ");
        assert!(has_vars(text));
    }

    #[test]
    fn invalid_names_and_other_brace_syntax_stay_literal() {
        let literal = "${VAR} {x} {name: .x} {{}} {{ }} {{a b}} {{é}} {{a/b}} {{:value}}";
        assert!(vars(literal).is_empty());
        assert!(!has_vars(literal));
        assert_eq!(fill(literal, &HashMap::new()), literal);

        let valid = "{{valid:yes}}";
        let unclosed = "{{unclosed";
        let text = format!("{literal} {valid} {unclosed}");
        assert_eq!(
            vars(&text),
            [Var {
                name: "valid".into(),
                default: Some("yes".into()),
            }]
        );
        assert_eq!(
            fill(&text, &HashMap::new()),
            format!("{literal} yes {unclosed}")
        );
        assert!(!has_vars("{{unclosed"));
    }

    #[test]
    fn defaults_allow_empty_and_literal_text() {
        assert_eq!(
            vars("{{empty:}} {{url:https://example.test/a b}}"),
            [
                Var {
                    name: "empty".into(),
                    default: Some(String::new()),
                },
                Var {
                    name: "url".into(),
                    default: Some("https://example.test/a b".into()),
                },
            ]
        );
    }

    #[test]
    fn fills_a_command_using_a_default_and_a_supplied_value() {
        let text = "dig @{{resolver:10.0.0.53}} {{domain}} A +notcp";
        let values = HashMap::from([("domain".into(), "example.test".into())]);
        assert_eq!(fill(text, &values), "dig @10.0.0.53 example.test A +notcp");
    }

    #[test]
    fn remembered_values_override_defaults_and_are_inserted_verbatim() {
        let values = HashMap::from([
            ("resolver".into(), "1.1.1.1".into()),
            ("empty".into(), String::new()),
            ("literal".into(), "{{resolver}}".into()),
        ]);
        assert_eq!(
            fill(
                "{{resolver:10.0.0.53}}|{{empty:fallback}}|{{literal}}",
                &values
            ),
            "1.1.1.1||{{resolver}}"
        );
    }
}
