# sqlformat

[![Version](https://img.shields.io/crates/v/sqlformat.svg)](https://crates.io/crates/sqlformat)
[![Docs](https://docs.rs/sqlformat/badge.svg)](https://docs.rs/sqlformat)
[![Build Status](https://github.com/shssoichiro/sqlformat-rs/workflows/sqlformat/badge.svg)](https://github.com/shssoichiro/sqlformat-rs/actions?query=branch%3Amaster)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![Codecov](https://img.shields.io/codecov/c/github/shssoichiro/sqlformat-rs)](https://app.codecov.io/gh/shssoichiro/sqlformat-rs)

Format SQL strings into readable, consistently styled output. `sqlformat` is a pure-Rust library designed to pretty-print SQL from a variety of mainstream dialects, ideal for logging, debugging, tests, or developer tools.

This crate is a Rust port of [sql-formatter-plus](https://github.com/kufii/sql-formatter-plus). There is currently no binary; the crate is intended to be used as a library.

## Key features

- **Broad SQL support**: Common constructs from PostgreSQL, MySQL/MariaDB, SQLite, SQL Server, and Oracle (DDL, DML, CTEs, CASE, JOINs, window functions, operators, type casts, etc.).
- **Configurable style**: Indentation (spaces or tabs), upper/lower/preserve keyword case, control lines between statements.
- **Inline controls**: Keep short blocks or argument lists inline when they fit; split when they don’t.
- **Parameter interpolation**: Supports `?`, `?1`, `$1`, `$name`, `:name`, `@name`, and bracketed variants via `QueryParams`.
- **Comment-aware**: Respects line/block comments; supports in-query toggles to temporarily disable formatting.
- **Safe Rust**: `#![forbid(unsafe_code)]`.

## Quick start

```rust
use sqlformat::{format, FormatOptions, Indent, QueryParams};

fn main() {
    let sql = "SELECT id, name FROM users WHERE created_at > NOW();";
    let options = FormatOptions::default();
    let formatted = format(sql, &QueryParams::None, &options);
    println!("{}", formatted);
}
```

Output:

```text
SELECT
  id,
  name
FROM
  users
WHERE
  created_at > NOW();
```

## Installation

Add via Cargo:

```bash
cargo add sqlformat
```

Or manually in `Cargo.toml`:

```toml
[dependencies]
sqlformat = "*"
```

Minimum Supported Rust Version (MSRV): `1.84`.

## Usage examples

### Basic formatting

```rust
use sqlformat::{format, FormatOptions, QueryParams};

let sql = "SELECT count(*), col FROM t WHERE a = 1 AND b = 2;";
let out = format(sql, &QueryParams::None, &FormatOptions::default());
```

### Indentation

```rust
use sqlformat::{format, FormatOptions, Indent, QueryParams};

let options = FormatOptions { indent: Indent::Spaces(4), ..Default::default() };
let out = format("SELECT a, b FROM t;", &QueryParams::None, &options);

let options = FormatOptions { indent: Indent::Tabs, ..Default::default() };
let out = format("SELECT a, b FROM t;", &QueryParams::None, &options);
```

### Keyword case conversion

```rust
use sqlformat::{format, FormatOptions, QueryParams};

// Uppercase reserved keywords
let options = FormatOptions { uppercase: Some(true), ..Default::default() };
let out = format("select distinct * from foo where bar = 1", &QueryParams::None, &options);

// Lowercase reserved keywords
let options = FormatOptions { uppercase: Some(false), ..Default::default() };
let out = format("SELECT DISTINCT * FROM FOO WHERE BAR = 1", &QueryParams::None, &options);

// Preserve case with exceptions
let options = FormatOptions {
    uppercase: Some(true),
    ignore_case_convert: Some(vec!["from", "where"]),
    ..Default::default()
};
let out = format("select * from foo where bar = 1", &QueryParams::None, &options);
```

### Inline/compact formatting

Control how aggressively short blocks and argument lists are kept on one line.

```rust
use sqlformat::{format, FormatOptions, QueryParams};

let options = FormatOptions {
    inline: false,              // when true, forces single-line output
    max_inline_block: 50,       // characters allowed to keep a parenthesized block inline
    max_inline_arguments: Some(40),
    max_inline_top_level: Some(40),
    ..Default::default()
};
let out = format("SELECT a, b, c, d, e, f, g, h FROM t;", &QueryParams::None, &options);
```

### JOIN layout

Treat any JOIN as a top-level keyword (affects line breaks):

```rust
use sqlformat::{format, FormatOptions, QueryParams};

let options = FormatOptions { joins_as_top_level: true, ..Default::default() };
let out = format("SELECT * FROM a INNER JOIN b ON a.id = b.a_id", &QueryParams::None, &options);
```

### Parameter interpolation

`sqlformat` can substitute placeholders using `QueryParams`:

```rust
use sqlformat::{format, FormatOptions, QueryParams};

// Numbered / positional (e.g., ?, ?1, $1)
let sql = "SELECT ?1, ?, $2;";
let params = QueryParams::Indexed(vec!["first".to_string(), "second".to_string(), "third".to_string()]);
let out = format(sql, &params, &FormatOptions::default());

// Named (e.g., $name, :name, @name, :\"weird name\")
let sql = "SELECT $hash, :name, @`var name`;";
let params = QueryParams::Named(vec![
    ("hash".to_string(), "hash value".to_string()),
    ("name".to_string(), "Alice".to_string()),
    ("var name".to_string(), "Bob".to_string()),
]);
let out = format(sql, &params, &FormatOptions::default());
```

### Controlling blank lines between statements

```rust
use sqlformat::{format, FormatOptions, QueryParams};

let options = FormatOptions { lines_between_queries: 2, ..Default::default() };
let out = format("SELECT 1; SELECT 2;", &QueryParams::None, &options);
```

### Temporarily disabling the formatter

You can turn formatting off/on using SQL comments. This is helpful when you want to preserve a very specific layout.

```sql
-- fmt: off
SELECT    *   FROM   t    WHERE   a=1 AND b=2;  -- preserved as-is
-- fmt: on

/* fmt: off */ SELECT 1 +   2; /* fmt: on */
```

## Configuration reference

The formatter is configured through `FormatOptions`. See the full API on the docs site for list of options.

## API reference

- Crate docs: [`docs.rs/sqlformat`](https://docs.rs/sqlformat)
- Primary entry point: `format(query: &str, params: &QueryParams, options: &FormatOptions) -> String`

## Contributing

Contributions are welcome!

- Run tests: `cargo test`
- Run benchmarks (optional): `cargo bench`

Please open issues and pull requests with clear descriptions and examples. Bug reports that include an input SQL snippet, your `FormatOptions`, and the actual vs. expected output are especially helpful.

## License

Dual-licensed under either of:

- MIT License (`LICENSE-MIT`)
- Apache License, Version 2.0 (`LICENSE-APACHE`)

## Acknowledgements

Based on the excellent work in [`sql-formatter-plus`](https://github.com/kufii/sql-formatter-plus).
