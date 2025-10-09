# Changelog

## Version 0.5.0

- feat!: Improve array support (#106)
- feat: Add initial debugging capabilities
- feat: Support PostgreSQL row-level locking syntax (FOR UPDATE, FOR SHARE) (#108)
- fix: More inline formatting edge cases (#109)
- fix: Type specifiers after numeric literals (#108)

## Version 0.4.0

- feat!: More formatting options (#78)
- feat!: More formatting options (#74)
- feat: Consistently space blocks after arguments
- feat: Correctly inline opening parentheses (#100)
- feat: Support more conditionally top level tokens (#99)
- feat: Complex insert support (#90)
- feat: Add fmt for SQLite blob literal
- feat: Uniform the behaviour of block and top-level span options
- fix: Place a whitespace between the array type specifier and a reserved word
- fix: Correctly format array type specifier (#91)
- fix: Format inline `CASE <expression> WHEN` correctly (#86)
- fix: Fix formatting WITH as a single line
- fix: Keep the previous tokens per-block
- fix: Improve the inline/columnar combination (#80)
- fix: Consider WITH a top level reserved word
- fix: Fix the span computation and usage
- chore: Upgrade to Winnow 0.7.0 (#79)

## Version 0.3.5

- Support pg_trgm operators
- Remove usage of a deprecated `winnow` function

## Version 0.3.4

- Migrate from `nom` to `winnow`, provides about 30% performance improvement

## Version 0.3.3

- Reduce binary size by removing regex dependency (#68)

## Version 0.3.2

- support ClickHouse/DuckDB join variants
- handle double colons better

## Version 0.3.1

- Allow latest regex version (#55)
  - [slightly breaking] Increases minimum Rust version to 1.65
- Fixes for operator parsing (#57)
- Performance improvements (#58)

## Version 0.3.0

- [breaking] fix: Ignore keywords for uppercase=True (#53)
- fix: uppercase=false does not lowercase the query (#50)
- fix: Possible to provide an escape hatch for expressions (#51)

## Version 0.2.6

- fix: ON UPDATE with two many blank formatted incorrectly (#46)
- fix: `EXCEPT` not handled well
- fix: REFERENCES xyz ON UPDATE .. causes formatter to treat the remaining as an UPDATE statement
- fix: Escaped strings formatted incorrectly
- fix: RETURNING is not placed on a new line
- fix: fix the issue of misaligned comments after formatting (#40)

## Version 0.2.4

- Remove `itertools` dependency [#34](https://github.com/shssoichiro/sqlformat-rs/pull/34)

## Version 0.2.3

- Allow alphanumeric characters in SQLite style parameters [#32](https://github.com/shssoichiro/sqlformat-rs/pull/32)
- Format "begin" and "declare" for PLPgSql [#30](https://github.com/shssoichiro/sqlformat-rs/pull/30)
- Allow scientific notation with or without "+"/"-" [#31](https://github.com/shssoichiro/sqlformat-rs/pull/31)
- Treat "$$" as a reserved token that sits on its own line [#29](https://github.com/shssoichiro/sqlformat-rs/pull/29)
- Bump itertools to version 0.12 [#28](https://github.com/shssoichiro/sqlformat-rs/pull/28)

## Version 0.2.2

- Fix a performance issue where the tokenizer would run in O^2
  time [#24](https://github.com/shssoichiro/sqlformat-rs/pull/24)

## Version 0.2.1

- Fix extra spaces inside of scientific notation [#16](https://github.com/shssoichiro/sqlformat-rs/pull/16)
- Remove unnecessary space in BETWEEN clause [#17](https://github.com/shssoichiro/sqlformat-rs/pull/17)
- Denote the minimum Rust version in Cargo.toml

## Version 0.2.0

- Fix extra spaces in string escaping [#13](https://github.com/shssoichiro/sqlformat-rs/pull/13)
- Fix panic on overflowing integer [#14](https://github.com/shssoichiro/sqlformat-rs/pull/14)
- Bump Rust edition to 2021
  - This is technically a breaking change as it bumps the minimum Rust version to 1.56

## Version 0.1.8

- Remove regex dependency
- Remove unused maplit dependency

## Version 0.1.7

- Bump nom to 7.0, which reportedly also fixes some build issues

## Version 0.1.6

- Fix compatibility with Rust 1.44 which was broken in 0.1.5

## Version 0.1.5

- Fix a possible panic on multibyte unicode strings

## Version 0.1.4

- Attempt again to fix the issue some users experience where this crate would fail to compile

## Version 0.1.3

- Fix an issue some users experienced where this crate would fail to compile

## Version 0.1.2

- Rewrite the parser in nom, providing significant performance improvements across the board
- Other significant performance improvement on pathological queries

## Version 0.1.1

- Significant performance improvements

## Version 0.1.0

- Initial release
