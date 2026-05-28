# path-filter - Your gitignore-like Path Filter &emsp; [![Latest Version]][crates.io] [![License: GPL v3]](./LICENSE) [![GitHub build status]][actions]

[license: gpl v3]: https://img.shields.io/badge/License-GPLv3-blue.svg
[latest version]: https://img.shields.io/crates/v/path-filter.svg
[crates.io]: https://crates.io/crates/path-filter
[github build status]: https://github.com/WolverinDEV/path-filter/actions/workflows/ci.yml/badge.svg?branch=master
[actions]: https://github.com/WolverinDEV/path-filter/actions/workflows/ci.yml

`path-filter` matches normalized paths against a small include/exclude rule format.
Glob patterns exclude paths by default. Prefix a pattern with `!` to include paths again. 

Paths should use `/` separators. Directory paths should include a trailing `/`.
The full pathfilter format is documented in [`syntax.md`](syntax.md).

## Getting Started

To use `path-filter`, add it as a dependency in your `Cargo.toml`:

```toml
[dependencies]
path-filter = "0.1"
```

## Basic Usage

Create a filter file with the paths you want to exclude and any exceptions you
want to include again:

`.ignorelist`

```text
/uploads/*
!/uploads/public/
!/uploads/private/error.log
```

Then load the file and ask whether a path is excluded. `matches` returns `true`
for excluded paths and `false` for included paths.

```rust,ignore
use path_filter::PathFilter;

fn main() -> Result<(), path_filter::Error> {
    let matcher = PathFilter::from_file(".ignorelist")?;
    assert!(matcher.matches("/uploads/private.txt"));
    assert!(!matcher.matches("/uploads/public/file.txt"));

    Ok(())
}
```
