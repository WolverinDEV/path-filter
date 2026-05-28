use std::{
    fs,
    path::Path,
    str::FromStr,
};

use crate::{
    error::Error,
    parser::{
        self,
        PatternFile,
        PatternFileEntry,
    },
};

#[derive(Debug)]
struct Rule {
    order: usize,
    priority: u32,
    negated: bool,
    recursive: bool,
    pattern_text: String,
    pattern: glob::Pattern,
}

/// Matches paths against a pathfilter rule set.
///
/// A `PathFilter` is built from pathfilter text, either by parsing a string
/// with [`FromStr`] or by loading a file with [`PathFilter::from_file`].
///
/// Patterns exclude paths by default. A pattern prefixed with `!` includes
/// matching paths again. Inline directives, such as `@priority:100`, may appear
/// before a pattern on the same line to change how that rule is evaluated.
///
/// Rules are evaluated by priority. The highest-priority matching rule wins,
/// and later rules win when multiple matching rules have the same priority.
#[derive(Debug)]
pub struct PathFilter {
    rules: Vec<Rule>,
}

impl PathFilter {
    /// Loads a pathfilter file and builds a matcher from its contents.
    ///
    /// This is equivalent to reading the file as UTF-8 text and parsing it with
    /// [`PathFilter`]'s [`FromStr`] implementation.
    pub fn from_file<P: AsRef<Path>>(file: P) -> Result<Self, Error> {
        let contents = fs::read_to_string(file)?;
        contents.parse()
    }
}

impl FromStr for PathFilter {
    type Err = Error;

    /// Parses pathfilter text into a matcher.
    ///
    /// Blank lines and comments are ignored. Directive-only lines are rejected;
    /// supported directives must appear inline before the pattern they affect.
    /// The only supported directive is currently `@priority:<number>`.
    ///
    /// Negated patterns must be absolute and must not contain `**`, because the
    /// matcher expands them into parent-directory include rules.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let PatternFile { entries } = parser::parse_input(s)?;

        let mut rules = vec![];
        for entry in entries {
            match entry {
                PatternFileEntry::Blank { .. } => {}
                PatternFileEntry::Comment { .. } => {}
                PatternFileEntry::Pattern {
                    directives,
                    pattern: pat,
                } => {
                    let mut priority = pat.pattern.len() as u32;
                    for dir in directives {
                        match dir.key.as_str() {
                            "priority" => {
                                priority = dir.value.parse::<u32>().map_err(|cause| {
                                    Error::DirectiveValueInvalid {
                                        key: dir.key.to_string(),
                                        cause: cause.into(),
                                    }
                                })?;
                            }
                            _ => {
                                return Err(Error::DirectiveUnknown {
                                    key: dir.key.to_string(),
                                });
                            }
                        }
                    }

                    if !pat.negated {
                        push_rule(&mut rules, priority, false, false, &pat.pattern)?;
                        continue;
                    }

                    if !pat.pattern.starts_with("/") {
                        /*
                         * We can not build a whitelist of parent directories for none
                         * absolute directories.
                         */
                        return Err(Error::PatternNegatedMustBeAbsolute);
                    }

                    let trimmed_pattern = pat.pattern.trim_start_matches('/').trim_end_matches('/');
                    let parts = if trimmed_pattern.is_empty() {
                        Vec::new()
                    } else {
                        trimmed_pattern.split('/').collect::<Vec<_>>()
                    };

                    for part in &parts {
                        if *part == "**" {
                            /*
                             * Arbitrary subdirectories would cause the rule to match every subdir in the tree.
                             */
                            return Err(Error::PatternNegatedNoArbitrarySubdirectories);
                        }
                    }

                    push_rule(&mut rules, priority, true, false, "/")?;

                    let mut current_pattern = String::from("/");
                    for part in parts.iter().take(parts.len().saturating_sub(1)) {
                        current_pattern.push_str(part);
                        current_pattern.push('/');
                        push_rule(&mut rules, priority, true, false, &current_pattern)?;
                    }

                    push_rule(
                        &mut rules,
                        priority,
                        true,
                        pat.pattern.ends_with('/'),
                        &pat.pattern,
                    )?;
                }
                PatternFileEntry::Directive { .. } => {
                    return Err(Error::DirectiveGlobalUnsupported);
                }
            }
        }

        rules.sort_by(|a, b| b.priority.cmp(&a.priority).then(b.order.cmp(&a.order)));

        Ok(Self { rules })
    }
}

fn push_rule(
    rules: &mut Vec<Rule>,
    priority: u32,
    negated: bool,
    recursive: bool,
    pattern: &str,
) -> Result<(), Error> {
    rules.push(Rule {
        order: rules.len(),
        priority,
        negated,
        recursive,
        pattern_text: pattern.to_string(),
        pattern: glob::Pattern::new(pattern).map_err(|source| Error::Pattern {
            pattern: pattern.to_string(),
            source,
        })?,
    });

    Ok(())
}

impl PathFilter {
    /// Returns whether `path` is excluded by this matcher.
    ///
    /// A return value of `true` means the path is excluded. A return value of
    /// `false` means the path is included or may contain included descendants.
    ///
    /// Directory paths are represented with a trailing `/`. For those paths,
    /// this method may return `false` even when the directory entry without the
    /// trailing slash is excluded, because a later rule can include something
    /// below that directory.
    ///
    /// For example, with these rules:
    ///
    /// ```text
    /// /uploads/*
    /// !/uploads/my-dir/important
    /// ```
    ///
    /// | Path | Excluded |
    /// | --- | --- |
    /// | `/uploads/blub` | `true` |
    /// | `/uploads/my-dir` | `true` |
    /// | `/uploads/my-dir/` | `false` |
    /// | `/uploads/my-dir/xxx` | `true` |
    /// | `/uploads/my-dir/important` | `false` |
    /// | `/uploads/my-dir/important/` | `false` |
    ///
    /// `path` is matched as provided; callers should normalize separators and
    /// use a trailing `/` for directory paths.
    pub fn matches(&self, path: &str) -> bool {
        for rule in &self.rules {
            if !rule.matches(path) {
                continue;
            }

            return !rule.negated;
        }

        false
    }
}

impl Rule {
    fn matches(&self, path: &str) -> bool {
        if self.pattern.matches(path) {
            return true;
        }

        self.negated
            && (path
                .strip_suffix('/')
                .is_some_and(|path| self.pattern.matches(path))
                || (self.recursive
                    && self.pattern_text.ends_with('/')
                    && path.starts_with(&self.pattern_text)))
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::filter::PathFilter;

    fn matcher(input: &str) -> PathFilter {
        PathFilter::from_str(input).unwrap()
    }

    fn assert_matches(input: &str, cases: &[(&str, bool)]) {
        let matcher = matcher(input);

        for (path, expected) in cases {
            assert_eq!(
                matcher.matches(path),
                *expected,
                "unexpected match result for {path:?} with rules:\n{input}"
            );
        }
    }

    #[test]
    fn unmatched_paths_are_included() {
        assert_matches("*.log", &[("/app.txt", false), ("/logs/debug.txt", false)]);
    }

    #[test]
    fn excludes_log_files() {
        assert_matches(
            "*.log",
            &[
                ("/app.log", true),
                ("/logs/debug.log", true),
                ("/app.txt", false),
            ],
        );
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        assert_matches(
            "# Exclude build outputs\n\n/build/*\n",
            &[("/build/app.o", true), ("/src/main.rs", false)],
        );
    }

    #[test]
    fn include_rule_overrides_less_specific_exclude_rule() {
        assert_matches(
            "/cache/*\n!/cache/keep/",
            &[
                ("/cache/file", true),
                ("/cache/keep", true),
                ("/cache/keep/", false),
                ("/cache/keep/config.toml", false),
            ],
        );
    }

    #[test]
    fn global_directives_are_rejected() {
        let error = PathFilter::from_str("@priority:200\n!/logs/important/").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("global directives are currently not supported")
        );
    }

    #[test]
    fn inline_priority_directive_applies_to_its_pattern_line() {
        assert_matches(
            "/logs/*\n@priority:200 !/logs/important/",
            &[
                ("/logs/debug.log", true),
                ("/logs/important/", false),
                ("/logs/important/app.log", false),
            ],
        );
    }

    #[test]
    fn higher_explicit_priority_wins_over_longer_derived_priority() {
        assert_matches(
            "@priority:200 /logs/*\n@priority:100 !/logs/important/",
            &[
                ("/logs/debug.log", true),
                ("/logs/important/", true),
                ("/logs/important/app.log", true),
            ],
        );
    }

    #[test]
    fn later_rule_wins_when_matching_rules_have_the_same_priority() {
        assert_matches(
            "@priority:100 /logs/*\n@priority:100 !/logs/important/",
            &[
                ("/logs/important/", false),
                ("/logs/important/app.log", false),
            ],
        );
    }

    #[test]
    fn directory_paths_may_be_included_to_allow_included_descendants() {
        assert_matches(
            "/uploads/*\n!/uploads/my-dir/important",
            &[
                ("/", false),
                ("/uploads/", false),
                ("/uploads/blub", true),
                ("/uploads/my-dir", true),
                ("/uploads/my-dir/", false),
                ("/uploads/my-dir/xxx", true),
                ("/uploads/my-dir/important", false),
            ],
        );
    }

    #[test]
    fn directory_paths_may_be_included_to_allow_included_descendants_dir() {
        assert_matches(
            "/uploads/*\n!/uploads/my-dir/important/",
            &[
                ("/", false),
                ("/uploads/", false),
                ("/uploads/blub", true),
                ("/uploads/my-dir", true),
                ("/uploads/my-dir/", false),
                ("/uploads/my-dir/xxx", true),
                ("/uploads/my-dir/important", true),
                ("/uploads/my-dir/important/", false),
                ("/uploads/my-dir/important/xxx", false),
            ],
        );
    }

    #[test]
    fn negated_patterns_with_wildcard() {
        assert_matches(
            "/uploads/*\n!/uploads/*/important",
            &[
                ("/", false),
                ("/uploads/", false),
                ("/uploads/blub", true),
                ("/uploads/my-dir", true),
                ("/uploads/my-dir/", false),
                ("/uploads/my-dir/xxx", true),
                ("/uploads/my-dir/important", false),
                ("/uploads/xxx", true),
                ("/uploads/xxx/", false),
                ("/uploads/xxx/xxx", true),
                ("/uploads/xxx/important", false),
            ],
        );
    }

    #[test]
    fn unknown_directive_is_rejected() {
        let error = PathFilter::from_str("@unknown:1 /pattern").unwrap_err();

        assert!(error.to_string().contains("unknown directive 'unknown'"));
    }

    #[test]
    fn invalid_priority_value_is_rejected() {
        let error = PathFilter::from_str("@priority:high /pattern").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("invalid directive value for 'priority'")
        );
    }

    #[test]
    fn negated_patterns_must_be_absolute() {
        let error = PathFilter::from_str("*\n!important/").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("negated patterns must be absolute")
        );
    }
}
