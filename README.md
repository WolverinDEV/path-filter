# path-filter

`path-filter` matches normalized paths against a small include/exclude rule format.
Glob patterns exclude paths by default. Prefix a pattern with `!` to include paths again. 

`.ignorelist` file:
```text
/uploads/*
!/uploads/public/
!/uploads/private/error.log
```

```rust,ignore
use path_filter::PathMatcher;

fn main() -> Result<(), path_filter::Error> {
    let matcher = PathMatcher::from_file(".ignorelist")?;
    assert!(matcher.matches("/uploads/private.txt"));
    assert!(!matcher.matches("/uploads/public/file.txt"));

    Ok(())
}
```

Paths should use `/` separators. Directory paths should include a trailing `/`.
The full pathfilter format is documented in [`syntax.md`](syntax.md).
