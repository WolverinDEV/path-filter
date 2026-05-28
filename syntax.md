# Pathfilter Format Specification
A pathfilter file specifies intentionally included and excluded paths with the following characteristics:
- Every non-blank, non-comment line contains a pattern.
- Patterns are matched using glob semantics.
- Blank lines match no files and are ignored.
- A line starting with `#` serves as a comment.
- A directive may appear before a pattern on the same line.
- Trailing spaces are ignored unless quoted or escaped.

By default, patterns specify exclusion rules.
A pattern may be negated by prefixing it with `!`.
Negated patterns define inclusion rules.

An optional explicit priority may be specified using a `@priority:<number>` directive before a pattern on the same line.

The resulting syntax is:

```text
file           = line*
line           = blank | comment | directive-line | pattern-line
comment        = "#" text
directive-line = directive+
pattern-line   = (directive whitespace+)* pattern
directive      = "@" key ":" value
value          = bare-value | quoted-value
pattern        = [ "!" ] glob
```

Example:

```text
# Exclude everything in uploads
/uploads/*

# But include this directory again
!/uploads/rubbish/stuff/KEEP_ME/

# Force logs to stay excluded even if another rule includes them
@priority:100 *.log

# Explicitly include this one log file
@priority:200 !/uploads/error.log
```

## Directives
Directives are metadata and never match paths directly.
A directive has the form `@<key>:<value>`.
A value containing spaces must be wrapped in double quotes (`"`).
Backslash escapes the next character inside quoted values.

Directives that affect matching must appear before the pattern on the same line, separated from the pattern by whitespace.
A line containing only directives is currently unsupported by the matcher and is rejected.
Such lines are part of the parsed syntax, but they are not valid matcher input.

Only the `@priority:` directive is currently supported.
It applies an explicit priority to the pattern on the same line.
Its value must be a number.
Unknown directives are invalid.

## Pattern Format
A pattern is interpreted as a glob expression.
The following special characters are supported:

* `*` matches any sequence of characters except `/`
* `**` matches any sequence of characters including `/`
* `?` matches any single character except `/`
* `[abc]` matches any character inside the set
* `[a-z]` matches any character inside the range

A leading slash `/` matches relative to the filter root.
A trailing slash `/` causes the pattern to only match directories.

Examples:
- `*.log` matches `/app.log` and `/logs/debug.log`
- `build/` matches `/build/` and `/tmp/build/` but not `/build.txt`
- `/foo/bar` matches `/foo/bar` but not `/tmp/foo/bar`
- `**/cache/` matches `cache/`, `tmp/cache/`, and `build/tmp/cache/`

## Negation
An optional prefix `!` negates the pattern.
Negated patterns specify inclusion rules.
Negated patterns must start with `/`.
Negated patterns must not contain `**`.

Example:
```text
/cache/*
!/cache/keep/
```

Excludes everything inside `cache/` except `cache/keep/`.

Negated patterns may themselves be overridden by patterns with higher priority.
When a negated pattern includes a trailing slash, the included directory and its descendants are included.
When a negated pattern does not include a trailing slash, only the exact path is included, while parent directories are included only so matching can reach that path.

## Priority
Each rule has a priority.
If no explicit priority is specified, the priority is derived from the pattern length.
Longer patterns have higher priority.

Examples:
```text
/tmp/
/tmp/cache/
/tmp/cache/keep/
```

The last rule has the highest derived priority.
An explicit priority may be specified using the `@priority:<number>` directive.
The directive applies to the pattern on the same line.

Example:
```text
@priority:100 /logs/*
@priority:200 !/logs/important/
```

The include rule has a higher priority (200) than the exclude rule (100).
Explicit priorities always override derived priorities.

## Rule Evaluation
For a given path:
1. All matching patterns are collected.
2. The pattern with the highest priority is selected.
3. If multiple matching patterns have the same priority, the last matching pattern is used.
4. The selected pattern determines whether the path is included or excluded.

If no pattern matches, the path is included.

## Examples
Exclude log files:
```text
*.log
```

Exclude a directory:
```text
/build/*
```

Exclude everything except a specific directory:
```text
/*
!/important/
```

This also includes descendants of `/important/`.

Exclude a directory but include a nested path:
```text
/cache/*
!/cache/keep/
```

Force a rule to always win:
```text
@priority:100 /logs/*
@priority:1000 !/logs/important/
```

## Notes
Patterns are evaluated using normalized path separators (`/`) regardless of the operating system.
Directory exclusions recursively affect contained files and directories unless overridden by a rule with higher priority.
Rule ordering only matters when multiple matching rules resolve to the same priority.
