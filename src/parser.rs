use itertools::Itertools;
use pest::{
    Parser,
    iterators::Pair,
};
use pest_derive::Parser;

use crate::error::Error;

#[derive(Parser)]
#[grammar = "gramma.pest"]
struct PatternFileParser;

#[derive(Debug, PartialEq, Eq)]
pub struct PatternFile {
    pub entries: Vec<PatternFileEntry>,
}

impl TryFrom<Pair<'_, Rule>> for PatternFile {
    type Error = Error;

    fn try_from(value: Pair<'_, Rule>) -> Result<Self, Self::Error> {
        assert_eq!(value.as_rule(), Rule::file);
        let mut inner = value.into_inner();

        let entries = inner
            .by_ref()
            .take_while(|entry| entry.as_rule() != Rule::EOI)
            .map(PatternFileEntry::try_from)
            .collect::<Result<_, _>>()?;

        assert_eq!(inner.next(), None);
        Ok(Self { entries })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PatternFileEntry {
    Blank {},
    Comment {},
    Directive {
        directives: Vec<Directive>,
    },
    Pattern {
        directives: Vec<Directive>,
        pattern: GlobPattern,
    },
}

impl TryFrom<Pair<'_, Rule>> for PatternFileEntry {
    type Error = Error;

    fn try_from(value: Pair<'_, Rule>) -> Result<Self, Self::Error> {
        let rule = value.as_rule();
        let mut inner = value.into_inner();

        let result = match rule {
            Rule::blank_line => Self::Blank {},
            Rule::comment_line => {
                while let Some(_) = inner.next() { /* drain everything */ }

                Self::Comment {}
            }
            Rule::directive_line => Self::Directive {
                directives: inner
                    .by_ref()
                    .map(Directive::try_from)
                    .collect::<Result<_, _>>()?,
            },
            Rule::pattern_line => {
                let directives = inner
                    .take_while_ref(|entry| entry.as_rule() == Rule::directive)
                    .map(Directive::try_from)
                    .collect::<Result<Vec<_>, _>>()?;

                let pattern = GlobPattern::try_from(inner.next().expect("to contain a pattern"))?;
                Self::Pattern {
                    directives,
                    pattern,
                }
            }
            _ => unreachable!("unexpected rule {rule:?}"),
        };

        assert_eq!(inner.next(), None);
        Ok(result)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct GlobPattern {
    pub negated: bool,
    pub pattern: String,
}

impl TryFrom<Pair<'_, Rule>> for GlobPattern {
    type Error = Error;

    fn try_from(value: Pair<'_, Rule>) -> Result<Self, Self::Error> {
        assert_eq!(value.as_rule(), Rule::pattern);
        let mut inner = value.into_inner();

        let mut negated = false;

        while let Some(entry) = inner.next() {
            match entry.as_rule() {
                Rule::pattern_glob => {
                    assert_eq!(inner.next(), None);
                    return Ok(Self {
                        negated,
                        pattern: entry.as_str().to_string(),
                    });
                }
                Rule::pattern_negation => {
                    assert_eq!(negated, false);
                    negated = true;
                }
                _ => unreachable!("unexpected rule {:?}", entry.as_rule()),
            }
        }

        unreachable!()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Directive {
    pub key: String,
    pub value: String,
}

impl TryFrom<Pair<'_, Rule>> for Directive {
    type Error = Error;

    fn try_from(value: Pair<Rule>) -> Result<Self, Self::Error> {
        assert_eq!(value.as_rule(), Rule::directive);
        let mut inner = value.into_inner();

        let key = inner.next().expect("a directive to have a key");
        assert_eq!(key.as_rule(), Rule::directive_key);
        let key = key.as_str();

        let value = inner.next().expect("a directive to have a value");
        assert_eq!(value.as_rule(), Rule::directive_value);

        let value =
            StringLiteral::try_from(value.into_inner().next().expect("missing directive value"))?;

        assert_eq!(inner.next(), None);
        Ok(Self {
            key: key.to_string(),
            value: value.value,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct StringLiteral {
    pub value: String,
}

impl StringLiteral {
    fn unescape_quoted_value(value: &str) -> String {
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .expect("quoted_value to be wrapped in quotes");

        let mut result = String::with_capacity(value.len());
        let mut chars = value.chars();

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                result.push(
                    chars
                        .next()
                        .expect("quoted_value escape to have a character"),
                );
            } else {
                result.push(ch);
            }
        }

        result
    }
}

impl TryFrom<Pair<'_, Rule>> for StringLiteral {
    type Error = Error;

    fn try_from(value: Pair<Rule>) -> Result<Self, Self::Error> {
        let value = match value.as_rule() {
            Rule::bare_value => value.as_str().to_string(),
            Rule::quoted_value => Self::unescape_quoted_value(value.as_str()),
            _ => unreachable!("invalid value of {value}"),
        };

        Ok(Self { value })
    }
}

pub fn parse_input(value: &str) -> Result<PatternFile, Error> {
    let mut value = PatternFileParser::parse(Rule::file, value)?;
    PatternFile::try_from(value.next().expect("parse to contain at least one entry"))
}

#[cfg(test)]
mod test {

    use crate::parser::{
        Directive,
        GlobPattern,
        PatternFile,
        PatternFileEntry,
        StringLiteral,
    };

    fn directive(key: &str, value: &str) -> Directive {
        Directive {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    fn pattern(negated: bool, value: &str) -> GlobPattern {
        GlobPattern {
            negated,
            pattern: value.to_string(),
        }
    }

    #[test]
    fn pattern_file_parses_blank_comments_and_patterns() {
        let file =
            super::parse_input("# ignore everything\n*\n\n# but uploads\n!/uploads\n").unwrap();

        assert_eq!(
            file,
            PatternFile {
                entries: vec![
                    PatternFileEntry::Comment {},
                    PatternFileEntry::Pattern {
                        directives: vec![],
                        pattern: pattern(false, "*"),
                    },
                    PatternFileEntry::Blank {},
                    PatternFileEntry::Comment {},
                    PatternFileEntry::Pattern {
                        directives: vec![],
                        pattern: pattern(true, "/uploads"),
                    },
                ],
            }
        );
    }

    #[test]
    fn pattern_file_parses_directive_only_lines() {
        let file = super::parse_input("@priority:100\n@tag:\"two words\"\n").unwrap();

        assert_eq!(
            file,
            PatternFile {
                entries: vec![
                    PatternFileEntry::Directive {
                        directives: vec![directive("priority", "100")],
                    },
                    PatternFileEntry::Directive {
                        directives: vec![directive("tag", "two words")],
                    },
                ],
            }
        );
    }

    #[test]
    fn pattern_file_parses_pattern_lines_with_inline_directives() {
        let file = super::parse_input("@priority:200 @label:\"important logs\" !logs/important/\n")
            .unwrap();

        assert_eq!(
            file,
            PatternFile {
                entries: vec![PatternFileEntry::Pattern {
                    directives: vec![
                        directive("priority", "200"),
                        directive("label", "important logs"),
                    ],
                    pattern: pattern(true, "logs/important/"),
                }],
            }
        );
    }

    #[test]
    fn pattern_file_unescapes_quoted_directive_values() {
        let file = super::parse_input(r#"@xx:"y-\"y\\y" pattern"#).unwrap();

        assert_eq!(
            file,
            PatternFile {
                entries: vec![PatternFileEntry::Pattern {
                    directives: vec![directive("xx", r#"y-"y\y"#)],
                    pattern: pattern(false, "pattern"),
                }],
            }
        );
    }

    #[test]
    fn unescape_quoted_value_removes_quotes() {
        assert_eq!(StringLiteral::unescape_quoted_value(r#""hello""#), "hello");
    }

    #[test]
    fn unescape_quoted_value_handles_escaped_quote() {
        assert_eq!(StringLiteral::unescape_quoted_value(r#""y-\"y""#), "y-\"y");
    }

    #[test]
    fn unescape_quoted_value_handles_escaped_backslash() {
        assert_eq!(StringLiteral::unescape_quoted_value(r#""y\\y""#), r#"y\y"#);
    }
}
