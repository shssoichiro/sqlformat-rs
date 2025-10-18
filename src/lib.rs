//! This crate is a port of <https://github.com/kufii/sql-formatter-plus>
//! written in Rust. It is intended to be usable as a pure-Rust library
//! for formatting SQL queries.

#![type_length_limit = "99999999"]
#![forbid(unsafe_code)]
// Maintains semver compatibility for older Rust versions
#![allow(clippy::manual_strip)]
// This lint is overly pedantic and annoying
#![allow(clippy::needless_lifetimes)]

use std::borrow::Cow;

use bon::{bon, builder, Builder};

mod formatter;
mod indentation;
mod inline_block;
mod params;
mod tokenizer;

#[cfg(feature = "debug")]
mod debug;

/// The SQL dialect to use. This affects parsing of special characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// Generic SQL syntax, most dialect-specific constructs are disabled
    #[default]
    Generic,
    /// Enables array syntax (`[`, `]`) and operators
    PostgreSql,
    /// Enables `[bracketed identifiers]` and `@variables`
    SQLServer,
}

/// SQL FormatOptions
#[derive(Debug, Clone, Builder)]
pub struct FormatOptions<'a> {
    /// Controls the type and length of indentation to use
    ///
    #[builder(default, into)]
    indent: Indent,
    /// When set, changes reserved keywords to ALL CAPS
    uppercase: Option<bool>,
    /// Controls the number of line breaks after a query
    #[builder(default = 1)]
    lines_between_queries: u8,
    /// Ignore case conversion for specified strings in the array.
    ignore_case_convert: Option<Vec<&'a str>>,
    /// Keep the query in a single line
    #[builder(default)]
    inline: bool,
    /// Maximum length of an inline block
    #[builder(default = 50)]
    max_inline_block: usize,
    /// Maximum length of inline arguments
    ///
    /// If unset keep every argument in a separate line
    max_inline_arguments: Option<usize>,
    /// Inline the argument at the top level if they would fit a line of this length
    max_inline_top_level: Option<usize>,
    /// Consider any JOIN statement as a top level keyword instead of a reserved keyword
    #[builder(default)]
    joins_as_top_level: bool,
    /// Tell the SQL dialect to use
    #[builder(default)]
    dialect: Dialect,
    /// Replacements for the placeholders in the query
    #[builder(default, into)]
    params: QueryParams<'a>,
}

impl<'a> FormatOptions<'a> {
    /// Format the SQL query string
    pub fn format(&self, query: &str) -> String {
        let tokens = tokenizer::tokenize(query, self.params.is_named(), self);
        formatter::format(&tokens, &self.params, self)
    }
}

#[bon]
impl<'a> FormatOptions<'a> {
    /// Use the FormatOptions with different params
    #[builder(
        finish_fn(
            name = format,
            doc {
                /// Format the SQL query string
            }
        )
    )]
    pub fn with_params<'b>(
        &self,
        #[builder(start_fn, into)] params: QueryParams<'b>,
        #[builder(finish_fn)] query: &str,
    ) -> String {
        let tokens = tokenizer::tokenize(query, params.is_named(), self);
        formatter::format(&tokens, &params, self)
    }
}

use format_options_builder::State;

impl<'a, S: State> FormatOptionsBuilder<'a, S> {
    /// Build and format the SQL query string in a single step
    pub fn format(self, query: &str) -> String {
        self.build().format(query)
    }
}

impl<'a> Default for FormatOptions<'a> {
    fn default() -> Self {
        Self::builder().build()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Indent {
    Spaces(u8),
    Tabs,
}

impl From<u8> for Indent {
    fn from(value: u8) -> Self {
        Self::Spaces(value)
    }
}

impl Default for Indent {
    fn default() -> Self {
        Self::Spaces(2)
    }
}

#[derive(Debug, Clone, Default)]
pub enum QueryParams<'a> {
    Named(Cow<'a, [(String, String)]>),
    Indexed(Cow<'a, [String]>),
    #[default]
    None,
}

impl<'a> From<Vec<(String, String)>> for QueryParams<'a> {
    fn from(value: Vec<(String, String)>) -> Self {
        Self::Named(Cow::Owned(value))
    }
}

impl<'a> From<Vec<String>> for QueryParams<'a> {
    fn from(value: Vec<String>) -> Self {
        Self::Indexed(Cow::Owned(value))
    }
}

impl<'a> From<&'a Vec<(String, String)>> for QueryParams<'a> {
    fn from(value: &'a Vec<(String, String)>) -> Self {
        Self::Named(Cow::Borrowed(value.as_ref()))
    }
}

impl<'a> From<&'a Vec<String>> for QueryParams<'a> {
    fn from(value: &'a Vec<String>) -> Self {
        Self::Indexed(Cow::Borrowed(value.as_ref()))
    }
}

impl<'a> From<&'a [(String, String)]> for QueryParams<'a> {
    fn from(value: &'a [(String, String)]) -> Self {
        Self::Named(Cow::Borrowed(value))
    }
}

impl<'a> From<&'a [String]> for QueryParams<'a> {
    fn from(value: &'a [String]) -> Self {
        Self::Indexed(Cow::Borrowed(value))
    }
}

impl<'a> QueryParams<'a> {
    fn is_named(&self) -> bool {
        matches!(self, QueryParams::Named(_))
    }
}

#[derive(Default, Debug, Clone)]
pub(crate) struct SpanInfo {
    pub full_span: usize,
    pub blocks: usize,
    pub newline_before: bool,
    pub newline_after: bool,
    pub arguments: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_sqlite_blob_literal_fmt() {
        let options = FormatOptions::default();

        let input = "SELECT x'73716c69676874' AS BLOB_VAL;";
        let expected = indoc!(
            "
            SELECT
              x'73716c69676874' AS BLOB_VAL;"
        );
        assert_eq!(options.format(input), expected);

        let input = "SELECT X'73716c69676874' AS BLOB_VAL;";
        let expected = indoc!(
            "
            SELECT
              X'73716c69676874' AS BLOB_VAL;"
        );
        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_uses_given_indent_config_for_indentation() {
        let input = "SELECT count(*),Column1 FROM Table1;";
        let options = FormatOptions::builder().indent(4);
        let expected = indoc!(
            "
            SELECT
                count(*),
                Column1
            FROM
                Table1;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_simple_set_schema_queries() {
        let input = "SET SCHEMA schema1; SET CURRENT SCHEMA schema2;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SET SCHEMA
              schema1;
            SET CURRENT SCHEMA
              schema2;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_simple_select_query() {
        let input = "SELECT count(*),Column1 FROM Table1;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              count(*),
              Column1
            FROM
              Table1;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_complex_select() {
        let input =
            "SELECT DISTINCT name, ROUND(age/7) field1, 18 + 20 AS field2, 'some string' FROM foo;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT DISTINCT
              name,
              ROUND(age / 7) field1,
              18 + 20 AS field2,
              'some string'
            FROM
              foo;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_over_with_window() {
        let input =
            "SELECT id, val, at, SUM(val) OVER win AS cumulative FROM data WINDOW win AS (PARTITION BY id ORDER BY at);";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              id,
              val,
              at,
              SUM(val) OVER win AS cumulative
            FROM
              data
            WINDOW
              win AS (
                PARTITION BY
                  id
                ORDER BY
                  at
              );"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_distinct_from() {
        let input = "SELECT bar IS DISTINCT FROM 'baz', IS NOT DISTINCT FROM 'foo' FROM foo;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              bar IS DISTINCT FROM 'baz',
              IS NOT DISTINCT FROM 'foo'
            FROM
              foo;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn keep_select_arguments_inline() {
        let input = indoc! {
            "
            SELECT
              a,
              b,
              c,
              d,
              e,
              f,
              g,
              h
            FROM foo;"
        };
        let options = FormatOptions::builder().max_inline_arguments(50);
        let expected = indoc! {
            "
            SELECT
              a, b, c, d, e, f, g, h
            FROM
              foo;"
        };
        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn split_select_arguments_inline_top_level() {
        let input = indoc! {
            "
            SELECT
              a,
              b,
              c,
              d,
              e,
              f,
              g,
              h
            FROM foo;"
        };
        let options = FormatOptions::builder()
            .max_inline_arguments(50)
            .max_inline_top_level(50);
        let expected = indoc! {
            "
            SELECT a, b, c, d, e, f, g, h
            FROM foo;"
        };
        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn inline_arguments_when_possible() {
        let input = indoc! {
            "
            SELECT
              a,
              b,
              c,
              d,
              e,
              f,
              g,
              h
            FROM foo;"
        };
        let options = FormatOptions::builder()
            .max_inline_arguments(50)
            .max_inline_top_level(20);
        let expected = indoc! {
            "
            SELECT
              a, b, c, d, e, f, g, h
            FROM foo;"
        };
        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn inline_single_block_argument() {
        let input = "SELECT a, b, c FROM ( SELECT (e+f) AS a, (m+o) AS b FROM d) WHERE (a != b) OR (c IS NULL AND a == b)";
        let options = FormatOptions::builder()
            .max_inline_arguments(10)
            .max_inline_top_level(20);
        let expected = indoc! {
            "
            SELECT a, b, c
            FROM (
              SELECT
                (e + f) AS a,
                (m + o) AS b
              FROM d
            )
            WHERE
              (a != b)
              OR (
                c IS NULL
                AND a == b
              )"
        };
        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_with_complex_where() {
        let input = indoc!(
            "
            SELECT * FROM foo WHERE Column1 = 'testing'
            AND ( (Column2 = Column3 OR Column4 >= NOW()) );
      "
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              *
            FROM
              foo
            WHERE
              Column1 = 'testing'
              AND (
                (
                  Column2 = Column3
                  OR Column4 >= NOW()
                )
              );"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_with_complex_where_inline() {
        let input = indoc!(
            "
            SELECT * FROM foo WHERE Column1 = 'testing'
            AND ( (Column2 = Column3 OR Column4 >= NOW()) );
      "
        );
        let options = FormatOptions::builder().max_inline_arguments(100);
        let expected = indoc!(
            "
            SELECT
              *
            FROM
              foo
            WHERE
              Column1 = 'testing' AND ((Column2 = Column3 OR Column4 >= NOW()));"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_with_top_level_reserved_words() {
        let input = indoc!(
            "
            SELECT * FROM foo WHERE name = 'John' GROUP BY some_column
            HAVING column > 10 ORDER BY other_column LIMIT 5;
      "
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              *
            FROM
              foo
            WHERE
              name = 'John'
            GROUP BY
              some_column
            HAVING
              column > 10
            ORDER BY
              other_column
            LIMIT
              5;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_with_for_update_of() {
        let input: &'static str = "SELECT id FROM users WHERE disabled_at IS NULL FOR UPDATE OF users SKIP LOCKED LIMIT 1";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              id
            FROM
              users
            WHERE
              disabled_at IS NULL
            FOR UPDATE
              OF users SKIP LOCKED
            LIMIT
              1"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_limit_with_two_comma_separated_values_on_single_line() {
        let input = "LIMIT 5, 10;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            LIMIT
              5, 10;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_limit_of_single_value_followed_by_another_select_using_commas() {
        let input = "LIMIT 5; SELECT foo, bar;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            LIMIT
              5;
            SELECT
              foo,
              bar;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_type_specifiers() {
        let input = "SELECT id,  ARRAY [] :: UUID [] FROM UNNEST($1  ::  UUID   []) WHERE $1::UUID[] IS NOT NULL;";
        let options = FormatOptions::builder().dialect(Dialect::PostgreSql);
        let expected = indoc!(
            "
            SELECT
              id,
              ARRAY[]::UUID[]
            FROM
              UNNEST($1::UUID[])
            WHERE
              $1::UUID[] IS NOT NULL;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_arrays_as_function_arguments() {
        let input =
            "SELECT array_position(ARRAY['sun','mon','tue',  'wed',   'thu','fri',  'sat'], 'mon');";
        let options = FormatOptions::builder().dialect(Dialect::PostgreSql);
        let expected = indoc!(
            "
            SELECT
              array_position(
                ARRAY['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'],
                'mon'
              );"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_arrays_as_values() {
        let input = " INSERT INTO t VALUES('a', ARRAY[0, 1,2,3], ARRAY[['a','b'],    ['c' ,'d']]);";
        let options = FormatOptions::builder()
            .dialect(Dialect::PostgreSql)
            .max_inline_block(10)
            .max_inline_top_level(50);
        let expected = indoc!(
            "
            INSERT INTO t
            VALUES (
              'a',
              ARRAY[0, 1, 2, 3],
              ARRAY[
                ['a', 'b'],
                ['c', 'd']
              ]
            );"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_array_index_notation() {
        let input = "SELECT a [ 1 ] + b [ 2 ] [   5+1 ] > c [3] ;";
        let options = FormatOptions::builder().dialect(Dialect::PostgreSql);
        let expected = indoc!(
            "
            SELECT
              a[1] + b[2][5 + 1] > c[3];"
        );

        assert_eq!(options.format(input), expected);
    }
    #[test]
    fn it_formats_limit_of_single_value_and_offset() {
        let input = "LIMIT 5 OFFSET 8;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            LIMIT
              5 OFFSET 8;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_limit_in_lowercase() {
        let input = "limit 5, 10;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            limit
              5, 10;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_preserves_case_of_keywords() {
        let input = "select distinct * frOM foo left join bar WHERe a > 1 and b = 3";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            select distinct
              *
            frOM
              foo
              left join bar
            WHERe
              a > 1
              and b = 3"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_query_with_select_query_inside_it() {
        let input = "SELECT *, SUM(*) AS sum FROM (SELECT * FROM Posts LIMIT 30) WHERE a > b";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              *,
              SUM(*) AS sum
            FROM
              (
                SELECT
                  *
                FROM
                  Posts
                LIMIT
                  30
              )
            WHERE
              a > b"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_does_format_drop() {
        let input = indoc!(
            "
                DROP INDEX IF EXISTS idx_a;
                DROP INDEX IF EXISTS idx_b;
                "
        );

        let options = FormatOptions {
            ..Default::default()
        };

        let expected = indoc!(
            "
                DROP INDEX IF EXISTS
                  idx_a;
                DROP INDEX IF EXISTS
                  idx_b;"
        );

        assert_eq!(options.format(input), expected);

        let input = indoc!(
            r#"
                -- comment
                DROP TABLE IF EXISTS "public"."table_name";
                "#
        );

        let expected = indoc!(
            r#"
                -- comment
                DROP TABLE IF EXISTS
                  "public"."table_name";"#
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_query_with_inner_join() {
        let input = indoc!(
            "
            SELECT customer_id.from, COUNT(order_id) AS total FROM customers
            INNER JOIN orders ON customers.customer_id = orders.customer_id;"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              customer_id.from,
              COUNT(order_id) AS total
            FROM
              customers
              INNER JOIN orders ON customers.customer_id = orders.customer_id;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_query_with_non_standard_join() {
        let input = indoc!(
            "
            SELECT customer_id.from, COUNT(order_id) AS total FROM customers
            INNER ANY JOIN orders ON customers.customer_id = orders.customer_id
            LEFT
            SEMI JOIN foo ON foo.id = customers.id
            PASTE
            JOIN bar
            ;"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              customer_id.from,
              COUNT(order_id) AS total
            FROM
              customers
              INNER ANY JOIN orders ON customers.customer_id = orders.customer_id
              LEFT SEMI JOIN foo ON foo.id = customers.id
              PASTE JOIN bar;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_query_with_non_standard_join_as_toplevel() {
        let input = indoc!(
            "
            SELECT customer_id.from, COUNT(order_id) AS total FROM customers
            INNER ANY JOIN orders ON customers.customer_id = orders.customer_id
            LEFT
            SEMI JOIN foo ON foo.id = customers.id
            PASTE
            JOIN bar
            ;"
        );
        let options = FormatOptions::builder()
            .joins_as_top_level(true)
            .max_inline_top_level(40)
            .max_inline_arguments(40);
        let expected = indoc!(
            "
            SELECT
              customer_id.from,
              COUNT(order_id) AS total
            FROM customers
            INNER ANY JOIN
              orders ON customers.customer_id = orders.customer_id
            LEFT SEMI JOIN foo ON foo.id = customers.id
            PASTE JOIN bar;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_query_with_different_comments() {
        let input = indoc!(
            "
            SELECT
            /*
             * This is a block comment
             */
            * FROM
            -- This is another comment
            MyTable # One final comment
            WHERE 1 = 2;"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              /*
               * This is a block comment
               */
              *
            FROM
              -- This is another comment
              MyTable # One final comment
            WHERE
              1 = 2;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_maintains_block_comment_indentation() {
        let input = indoc!(
            "
            SELECT
              /*
               * This is a block comment
               */
              *
            FROM
              MyTable
            WHERE
              1 = 2;"
        );
        let options = FormatOptions::default();

        assert_eq!(options.format(input), input);
    }

    #[test]
    fn it_formats_simple_insert_query() {
        let input = "INSERT INTO Customers (ID, MoneyBalance, Address, City) VALUES (12,-123.4, 'Skagen 2111','Stv');";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            INSERT INTO
              Customers (ID, MoneyBalance, Address, City)
            VALUES
              (12, -123.4, 'Skagen 2111', 'Stv');"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_complex_insert_query() {
        let input = "
 INSERT INTO t(id, a, min, max) SELECT input.id, input.a, input.min, input.max FROM ( SELECT id, a, min, max FROM foo WHERE a IN ('a', 'b') ) AS input WHERE (SELECT true FROM condition) ON CONFLICT ON CONSTRAINT a_id_key DO UPDATE SET id = EXCLUDED.id, a = EXCLUDED.severity, min = EXCLUDED.min, max = EXCLUDED.max RETURNING *; ";
        let max_line = 50;
        let options = FormatOptions::builder()
            .max_inline_block(max_line)
            .max_inline_arguments(max_line)
            .max_inline_top_level(max_line);

        let expected = indoc!(
            "
            INSERT INTO t(id, a, min, max)
            SELECT input.id, input.a, input.min, input.max
            FROM (
              SELECT id, a, min, max
              FROM foo
              WHERE a IN ('a', 'b')
            ) AS input
            WHERE (SELECT true FROM condition)
            ON CONFLICT ON CONSTRAINT a_id_key DO UPDATE SET
              id = EXCLUDED.id,
              a = EXCLUDED.severity,
              min = EXCLUDED.min,
              max = EXCLUDED.max
            RETURNING *;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_keeps_short_parenthesized_list_with_nested_parenthesis_on_single_line() {
        let input = "SELECT (a + b * (c - NOW()));";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              (a + b * (c - NOW()));"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_breaks_long_parenthesized_lists_to_multiple_lines() {
        let input = indoc!(
            "
            INSERT INTO some_table (id_product, id_shop, id_currency, id_country, id_registration) (
            SELECT IF(dq.id_discounter_shopping = 2, dq.value, dq.value / 100),
            IF (dq.id_discounter_shopping = 2, 'amount', 'percentage') FROM foo);"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            INSERT INTO
              some_table (
                id_product,
                id_shop,
                id_currency,
                id_country,
                id_registration
              ) (
                SELECT
                  IF (
                    dq.id_discounter_shopping = 2,
                    dq.value,
                    dq.value / 100
                  ),
                  IF (
                    dq.id_discounter_shopping = 2,
                    'amount',
                    'percentage'
                  )
                FROM
                  foo
              );"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_keep_long_parenthesized_lists_to_multiple_lines() {
        let input = indoc!(
            "
            INSERT INTO some_table (id_product, id_shop, id_currency, id_country, id_registration) (
            SELECT IF (dq.id_discounter_shopping = 2, dq.value, dq.value / 100),
            IF (dq.id_discounter_shopping = 2, 'amount', 'percentage') FROM foo);"
        );
        let options = FormatOptions::builder().max_inline_block(100);
        let expected = indoc!(
            "
            INSERT INTO
              some_table (id_product, id_shop, id_currency, id_country, id_registration) (
                SELECT
                  IF (dq.id_discounter_shopping = 2, dq.value, dq.value / 100),
                  IF (dq.id_discounter_shopping = 2, 'amount', 'percentage')
                FROM
                  foo
              );"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_simple_update_query() {
        let input = "UPDATE Customers SET ContactName='Alfred Schmidt', City='Hamburg' WHERE CustomerName='Alfreds Futterkiste';";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            UPDATE
              Customers
            SET
              ContactName = 'Alfred Schmidt',
              City = 'Hamburg'
            WHERE
              CustomerName = 'Alfreds Futterkiste';"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_simple_update_query_inlining_set() {
        let input = "UPDATE Customers SET ContactName='Alfred Schmidt', City='Hamburg' WHERE CustomerName='Alfreds Futterkiste';";
        let options = FormatOptions::builder()
            .max_inline_top_level(20)
            .max_inline_arguments(10);
        let expected = indoc!(
            "
            UPDATE Customers SET
              ContactName = 'Alfred Schmidt',
              City = 'Hamburg'
            WHERE
              CustomerName = 'Alfreds Futterkiste';"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_simple_delete_query() {
        let input = "DELETE FROM Customers WHERE CustomerName='Alfred' AND Phone=5002132;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            DELETE FROM
              Customers
            WHERE
              CustomerName = 'Alfred'
              AND Phone = 5002132;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_full_delete_query() {
        let input =
            "DELETE FROM Customers USING Phonebook WHERE CustomerName='Alfred' AND Phone=5002132;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            DELETE FROM
              Customers
            USING
              Phonebook
            WHERE
              CustomerName = 'Alfred'
              AND Phone = 5002132;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_simple_drop_query() {
        let input = "DROP TABLE IF EXISTS admin_role;";
        let options = FormatOptions::default();
        let output = indoc!(
            "
            DROP TABLE IF EXISTS
              admin_role;"
        );
        assert_eq!(options.format(input), output);
    }

    #[test]
    fn it_formats_incomplete_query() {
        let input = "SELECT count(";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              count("
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_query_that_ends_with_open_comment() {
        let input = indoc!(
            "
            SELECT count(*)
            /*Comment"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              count(*)
              /*Comment"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_update_query_with_as_part() {
        let input = "UPDATE customers SET total_orders = order_summary.total  FROM ( SELECT * FROM bank) AS order_summary";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            UPDATE
              customers
            SET
              total_orders = order_summary.total
            FROM
              (
                SELECT
                  *
                FROM
                  bank
              ) AS order_summary"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_update_query_with_as_part_inline() {
        let options = FormatOptions::builder().inline(true);
        let expected = "UPDATE customers SET total_orders = order_summary.total FROM ( SELECT * FROM bank ) AS order_summary";
        let input = indoc!(
            "
            UPDATE
              customers
            SET
              total_orders = order_summary.total
            FROM
              (
                SELECT
                  *
                FROM
                  bank
              ) AS order_summary"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_top_level_and_newline_multi_word_reserved_words_with_inconsistent_spacing() {
        let input = "SELECT * FROM foo LEFT \t OUTER  \n JOIN bar ORDER \n BY blah";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              *
            FROM
              foo
              LEFT OUTER JOIN bar
            ORDER BY
              blah"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_long_double_parenthesized_queries_to_multiple_lines() {
        let input = "((foo = '0123456789-0123456789-0123456789-0123456789'))";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            (
              (
                foo = '0123456789-0123456789-0123456789-0123456789'
              )
            )"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_short_double_parenthesizes_queries_to_one_line() {
        let input = "((foo = 'bar'))";
        let options = FormatOptions::default();

        assert_eq!(options.format(input), input);
    }

    #[test]
    fn it_formats_single_char_operators() {
        let inputs = [
            "foo = bar",
            "foo < bar",
            "foo > bar",
            "foo + bar",
            "foo - bar",
            "foo * bar",
            "foo / bar",
            "foo % bar",
        ];
        let options = FormatOptions::default();
        for input in &inputs {
            assert_eq!(&options.format(input), input);
        }
    }

    #[test]
    fn it_formats_multi_char_operators() {
        let inputs = [
            "foo != bar",
            "foo <> bar",
            "foo == bar",
            "foo || bar",
            "foo <= bar",
            "foo >= bar",
            "foo !< bar",
            "foo !> bar",
        ];
        let options = FormatOptions::default();
        for input in &inputs {
            assert_eq!(&options.format(input), input);
        }
    }

    #[test]
    fn it_formats_logical_operators() {
        let inputs = [
            "foo ALL bar",
            "foo = ANY (1, 2, 3)",
            "EXISTS bar",
            "foo IN (1, 2, 3)",
            "foo LIKE 'hello%'",
            "foo IS NULL",
            "UNIQUE foo",
        ];
        let options = FormatOptions::default();
        for input in &inputs {
            assert_eq!(&options.format(input), input);
        }
    }

    #[test]
    fn it_formats_and_or_operators() {
        let strings = [
            ("foo BETWEEN bar AND baz", "foo BETWEEN bar AND baz"),
            ("foo BETWEEN\nbar\nAND baz", "foo BETWEEN bar AND baz"),
            ("foo AND bar", "foo\nAND bar"),
            ("foo OR bar", "foo\nOR bar"),
        ];
        let options = FormatOptions::default();
        for (input, output) in &strings {
            assert_eq!(&options.format(input), output);
        }
    }

    #[test]
    fn it_recognizes_strings() {
        let inputs = ["\"foo JOIN bar\"", "'foo JOIN bar'", "`foo JOIN bar`"];
        let options = FormatOptions::default();
        for input in &inputs {
            assert_eq!(&options.format(input), input);
        }
    }

    #[test]
    fn it_recognizes_escaped_strings() {
        let inputs = [
            r#""foo \" JOIN bar""#,
            r#"'foo \' JOIN bar'"#,
            r#"`foo `` JOIN bar`"#,
            r#"'foo '' JOIN bar'"#,
            r#"'two households"'"#,
            r#"'two households'''"#,
            r#"E'alice'''"#,
        ];
        let options = FormatOptions::default();
        for input in &inputs {
            assert_eq!(&options.format(input), input);
        }
    }

    #[test]
    fn it_formats_postgres_specific_operators() {
        let strings = [
            ("column::int", "column::int"),
            ("v->2", "v -> 2"),
            ("v->>2", "v ->> 2"),
            ("foo ~~ 'hello'", "foo ~~ 'hello'"),
            ("foo !~ 'hello'", "foo !~ 'hello'"),
            ("foo ~* 'hello'", "foo ~* 'hello'"),
            ("foo ~~* 'hello'", "foo ~~* 'hello'"),
            ("foo !~~ 'hello'", "foo !~~ 'hello'"),
            ("foo !~* 'hello'", "foo !~* 'hello'"),
            ("foo !~~* 'hello'", "foo !~~* 'hello'"),
        ];
        let options = FormatOptions::default();
        for (input, output) in &strings {
            assert_eq!(&options.format(input), output);
        }
    }

    #[test]
    fn it_keeps_separation_between_multiple_statements() {
        let strings = [
            ("foo;bar;", "foo;\nbar;"),
            ("foo\n;bar;", "foo;\nbar;"),
            ("foo\n\n\n;bar;\n\n", "foo;\nbar;"),
        ];
        let options = FormatOptions::default();
        for (input, output) in &strings {
            assert_eq!(&options.format(input), output);
        }

        let input = indoc!(
            "
            SELECT count(*),Column1 FROM Table1;
            SELECT count(*),Column1 FROM Table2;"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              count(*),
              Column1
            FROM
              Table1;
            SELECT
              count(*),
              Column1
            FROM
              Table2;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_unicode_correctly() {
        let input = "SELECT test, тест FROM table;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              test,
              тест
            FROM
              table;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_converts_keywords_to_uppercase_when_option_passed_in() {
        let input = "select distinct * frOM foo left join bar WHERe cola > 1 and colb = 3";
        let options = FormatOptions {
            uppercase: Some(true),
            ..FormatOptions::default()
        };
        let expected = indoc!(
            "
            SELECT DISTINCT
              *
            FROM
              foo
              LEFT JOIN bar
            WHERE
              cola > 1
              AND colb = 3"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_line_breaks_between_queries_with_config() {
        let input = "SELECT * FROM foo; SELECT * FROM bar;";
        let options = FormatOptions {
            lines_between_queries: 2,
            ..FormatOptions::default()
        };
        let expected = indoc!(
            "
            SELECT
              *
            FROM
              foo;

            SELECT
              *
            FROM
              bar;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_correctly_indents_create_statement_after_select() {
        let input = indoc!(
            "
            SELECT * FROM test;
            CREATE TABLE TEST(id NUMBER NOT NULL, col1 VARCHAR2(20), col2 VARCHAR2(20));
        "
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              *
            FROM
              test;
            CREATE TABLE TEST(
              id NUMBER NOT NULL,
              col1 VARCHAR2(20),
              col2 VARCHAR2(20)
            );"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_short_create_table() {
        let input = "CREATE TABLE items (a INT PRIMARY KEY, b TEXT);";
        let options = FormatOptions::default();

        assert_eq!(options.format(input), input);
    }

    #[test]
    fn it_formats_long_create_table() {
        let input =
            "CREATE TABLE items (a INT PRIMARY KEY, b TEXT, c INT NOT NULL, d INT NOT NULL);";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            CREATE TABLE items (
              a INT PRIMARY KEY,
              b TEXT,
              c INT NOT NULL,
              d INT NOT NULL
            );"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_insert_without_into() {
        let input =
            "INSERT Customers (ID, MoneyBalance, Address, City) VALUES (12,-123.4, 'Skagen 2111','Stv');";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            INSERT
              Customers (ID, MoneyBalance, Address, City)
            VALUES
              (12, -123.4, 'Skagen 2111', 'Stv');"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_alter_table_modify_query() {
        let input = "ALTER TABLE supplier MODIFY supplier_name char(100) NOT NULL;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            ALTER TABLE
              supplier
            MODIFY
              supplier_name char(100) NOT NULL;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_alter_table_alter_column_query() {
        let input = "ALTER TABLE supplier ALTER COLUMN supplier_name VARCHAR(100) NOT NULL;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            ALTER TABLE
              supplier
              ALTER COLUMN supplier_name VARCHAR(100) NOT NULL;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_alter_table_add_and_drop() {
        let input = r#"ALTER TABLE "public"."event" DROP CONSTRAINT "validate_date", ADD CONSTRAINT "validate_date" CHECK (end_date IS NULL
            OR (start_date IS NOT NULL AND end_date > start_date));"#;

        let options = FormatOptions::default();
        let expected = indoc!(
            r#"
            ALTER TABLE
              "public"."event"
              DROP CONSTRAINT "validate_date",
              ADD CONSTRAINT "validate_date" CHECK (
                end_date IS NULL
                OR (
                  start_date IS NOT NULL
                  AND end_date > start_date
                )
              );"#
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_bracketed_strings() {
        let inputs = ["[foo JOIN bar]", "[foo ]] JOIN bar]"];
        let options = FormatOptions {
            dialect: Dialect::SQLServer,
            ..Default::default()
        };
        for input in &inputs {
            assert_eq!(&options.format(input), input);
        }
    }

    #[test]
    fn it_recognizes_at_variables() {
        let input =
            "SELECT @variable, @a1_2.3$, @'var name', @\"var name\", @`var name`, @[var name];";
        let options = FormatOptions {
            dialect: Dialect::SQLServer,
            ..Default::default()
        };
        let expected = indoc!(
            "
            SELECT
              @variable,
              @a1_2.3$,
              @'var name',
              @\"var name\",
              @`var name`,
              @[var name];"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_at_variables_with_param_values() {
        let input =
            "SELECT @variable, @a1_2.3$, @'var name', @\"var name\", @`var name`, @[var name], @'var\\name';";
        let params = [
            ("variable".to_string(), "\"variable value\"".to_string()),
            ("a1_2.3$".to_string(), "'weird value'".to_string()),
            ("var name".to_string(), "'var value'".to_string()),
            ("var\\name".to_string(), "'var\\ value'".to_string()),
        ];
        let options = FormatOptions::builder()
            .dialect(Dialect::SQLServer)
            .params(params.as_ref());
        let expected = indoc!(
            "
            SELECT
              \"variable value\",
              'weird value',
              'var value',
              'var value',
              'var value',
              'var value',
              'var\\ value';"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_colon_variables() {
        let input =
            "SELECT :variable, :a1_2.3$, :'var name', :\"var name\", :`var name`, :[var name];";
        let options = FormatOptions {
            dialect: Dialect::SQLServer,
            ..Default::default()
        };
        let expected = indoc!(
            "
            SELECT
              :variable,
              :a1_2.3$,
              :'var name',
              :\"var name\",
              :`var name`,
              :[var name];"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_colon_variables_with_param_values() {
        let input = indoc!(
            "
            SELECT :variable, :a1_2.3$, :'var name', :\"var name\", :`var name`,
            :[var name], :'escaped \\'var\\'', :\"^*& weird \\\" var   \";
            "
        );
        let params = vec![
            ("variable".to_string(), "\"variable value\"".to_string()),
            ("a1_2.3$".to_string(), "'weird value'".to_string()),
            ("var name".to_string(), "'var value'".to_string()),
            ("escaped 'var'".to_string(), "'weirder value'".to_string()),
            (
                "^*& weird \" var   ".to_string(),
                "'super weird value'".to_string(),
            ),
        ];
        let options = FormatOptions::builder()
            .dialect(Dialect::SQLServer)
            .params(params);
        let expected = indoc!(
            "
            SELECT
              \"variable value\",
              'weird value',
              'var value',
              'var value',
              'var value',
              'var value',
              'weirder value',
              'super weird value';"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_question_numbered_placeholders() {
        let input = "SELECT ?1, ?25, ?;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              ?1,
              ?25,
              ?;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_question_numbered_placeholders_with_param_values() {
        let input = "SELECT ?1, ?2, ?0;";
        let params = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];
        let options = FormatOptions::builder().params(params);
        let expected = indoc!(
            "
            SELECT
              second,
              third,
              first;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_question_indexed_placeholders_with_param_values() {
        let input = "SELECT ?, ?, ?;";
        let params = [
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];
        let options = FormatOptions::builder().params(params.as_ref());
        let expected = indoc!(
            "
            SELECT
              first,
              second,
              third;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_dollar_sign_numbered_placeholders() {
        let input = "SELECT $1, $2;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              $1,
              $2;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_dollar_sign_alphanumeric_placeholders() {
        let input = "SELECT $hash, $foo, $bar;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              $hash,
              $foo,
              $bar;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_dollar_sign_numbered_placeholders_with_param_values() {
        let input = "SELECT $2, $3, $1, $named, $4, $alias;";
        let params = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
            "4th".to_string(),
        ];
        let options = FormatOptions::builder().params(params);
        let expected = indoc!(
            "
            SELECT
              second,
              third,
              first,
              $named,
              4th,
              $alias;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_dollar_sign_alphanumeric_placeholders_with_param_values() {
        let input = "SELECT $hash, $salt, $1, $2;";
        let params = vec![
            ("hash".to_string(), "hash value".to_string()),
            ("salt".to_string(), "salt value".to_string()),
            ("1".to_string(), "number 1".to_string()),
            ("2".to_string(), "number 2".to_string()),
        ];
        let options = FormatOptions::builder().params(params);
        let expected = indoc!(
            "
            SELECT
              hash value,
              salt value,
              number 1,
              number 2;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_braced_placeholders_with_param_values() {
        let input = "SELECT {a}, {b}, {c};";
        let params = vec![
            ("a".to_string(), "first".to_string()),
            ("b".to_string(), "second".to_string()),
            ("c".to_string(), "third".to_string()),
        ];
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              first,
              second,
              third;"
        );

        assert_eq!(options.with_params(params).format(input), expected);
    }

    #[test]
    fn it_formats_query_with_go_batch_separator() {
        let input = "SELECT 1 GO SELECT 2";
        let params = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];
        let options = FormatOptions::builder().params(params);
        let expected = indoc!(
            "
            SELECT
              1
            GO
            SELECT
              2"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_query_with_cross_join() {
        let input = "SELECT a, b FROM t CROSS JOIN t2 on t.id = t2.id_t";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              a,
              b
            FROM
              t
              CROSS JOIN t2 on t.id = t2.id_t"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_query_with_cross_apply() {
        let input = "SELECT a, b FROM t CROSS APPLY fn(t.id)";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              a,
              b
            FROM
              t
              CROSS APPLY fn(t.id)"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_simple_select() {
        let input = "SELECT N, M FROM t";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              N,
              M
            FROM
              t"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_simple_select_with_national_characters_mssql() {
        let input = "SELECT N'value'";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              N'value'"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_select_query_with_outer_apply() {
        let input = "SELECT a, b FROM t OUTER APPLY fn(t.id)";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              a,
              b
            FROM
              t
              OUTER APPLY fn(t.id)"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_fetch_first_like_limit() {
        let input = "SELECT * FETCH FIRST 2 ROWS ONLY;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              *
            FETCH FIRST
              2 ROWS ONLY;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_case_when_with_a_blank_expression() {
        let input = "CASE WHEN option = 'foo' THEN 1 WHEN option = 'bar' THEN 2 WHEN option = 'baz' THEN 3 ELSE 4 END;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            CASE
              WHEN option = 'foo' THEN 1
              WHEN option = 'bar' THEN 2
              WHEN option = 'baz' THEN 3
              ELSE 4
            END;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_case_when_inside_select() {
        let input =
            "SELECT foo, bar, CASE baz WHEN 'one' THEN 1 WHEN 'two' THEN 2 ELSE 3 END FROM table";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              foo,
              bar,
              CASE
                baz
                WHEN 'one' THEN 1
                WHEN 'two' THEN 2
                ELSE 3
              END
            FROM
              table"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_case_when_inside_select_inlined_top_level() {
        let input =
            "SELECT foo, bar, CASE baz WHEN 'one' THEN 1 WHEN 'two' THEN 2 ELSE 3 END FROM table";
        let options = FormatOptions {
            max_inline_top_level: Some(50),
            ..Default::default()
        };
        let expected = indoc!(
            "
            SELECT
              foo,
              bar,
              CASE
                baz
                WHEN 'one' THEN 1
                WHEN 'two' THEN 2
                ELSE 3
              END
            FROM table"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_case_when_with_an_expression() {
        let input = "CASE toString(getNumber()) WHEN 'one' THEN 1 WHEN 'two' THEN 2 WHEN 'three' THEN 3 ELSE 4 END;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            CASE
              toString(getNumber())
              WHEN 'one' THEN 1
              WHEN 'two' THEN 2
              WHEN 'three' THEN 3
              ELSE 4
            END;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_case_when_inside_an_order_by() {
        let input = "SELECT a, created_at FROM b ORDER BY (CASE $3 WHEN 'created_at_asc' THEN created_at END) ASC, (CASE $3 WHEN 'created_at_desc' THEN created_at END) DESC;";
        let max_line = 80;
        let options = FormatOptions {
            max_inline_block: max_line,
            max_inline_arguments: Some(max_line),
            ..Default::default()
        };

        let expected = indoc!(
            "
            SELECT
              a, created_at
            FROM
              b
            ORDER BY
              (CASE $3 WHEN 'created_at_asc' THEN created_at END) ASC,
              (CASE $3 WHEN 'created_at_desc' THEN created_at END) DESC;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_lowercase_case_end() {
        let input = "case when option = 'foo' then 1 else 2 end;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            case
              when option = 'foo' then 1
              else 2
            end;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_ignores_words_case_and_end_inside_other_strings() {
        let input = "SELECT CASEDATE, ENDDATE FROM table1;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              CASEDATE,
              ENDDATE
            FROM
              table1;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_tricky_line_comments() {
        let input = "SELECT a#comment, here\nFROM b--comment";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              a #comment, here
            FROM
              b --comment"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_line_comments_followed_by_semicolon() {
        let input = indoc!(
            "
            SELECT a FROM b
            --comment
            ;"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              a
            FROM
              b --comment
            ;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_line_comments_followed_by_comma() {
        let input = indoc!(
            "
            SELECT a --comment
            , b"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              a --comment
            ,
              b"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_line_comments_followed_by_close_paren() {
        let input = "SELECT ( a --comment\n )";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              (
                a --comment
              )"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_line_comments_followed_by_open_paren() {
        let input = "SELECT a --comment\n()";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              a --comment
              ()"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_lonely_semicolon() {
        let input = ";";
        let options = FormatOptions::default();

        assert_eq!(options.format(input), input);
    }

    #[test]
    fn it_formats_multibyte_chars() {
        let input = "\nSELECT 'главная'";
        let options = FormatOptions::default();
        let expected = "SELECT\n  'главная'";

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_scientific_notation() {
        let input = "SELECT *, 1e-7 as small, 1e2 as medium, 1e+7 as large FROM t";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              *,
              1e-7 as small,
              1e2 as medium,
              1e+7 as large
            FROM
              t"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_keeps_double_dollar_signs_together() {
        let input = "CREATE FUNCTION abc() AS $$ SELECT * FROM table $$ LANGUAGE plpgsql;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            CREATE FUNCTION abc() AS
            $$
            SELECT
              *
            FROM
              table
            $$
            LANGUAGE plpgsql;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_pgplsql() {
        let input = "CREATE FUNCTION abc() AS $$ DECLARE a int := 1; b int := 2; BEGIN SELECT * FROM table $$ LANGUAGE plpgsql;";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            CREATE FUNCTION abc() AS
            $$
            DECLARE
            a int := 1;
            b int := 2;
            BEGIN
            SELECT
              *
            FROM
              table
            $$
            LANGUAGE plpgsql;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_handles_comments_correctly() {
        let input = indoc!(
            "
                -- 创建一个外部表，存储销售数据
            CREATE EXTERNAL TABLE IF NOT EXISTS sales_data (
                -- 唯一标识订单ID
                order_id BIGINT COMMENT 'Unique identifier for the order',

                -- 客户ID
                customer_id BIGINT COMMENT 'Unique identifier for the customer',
            )
            COMMENT 'Sales data table for storing transaction records';

            -- 按销售日期和城市进行分区
            PARTITIONED BY (
                sale_year STRING COMMENT 'Year of the sale',
                sale_month STRING COMMENT 'Month of the sale'
            )

            -- 设置数据存储位置
            LOCATION '/user/hive/warehouse/sales_data'

            -- 使用 ORC 存储格式
            STORED AS ORC

            -- 设置表的行格式
            ROW FORMAT DELIMITED
            FIELDS TERMINATED BY ','
            LINES TERMINATED BY '\n'

            -- 设置表属性
            TBLPROPERTIES (
                'orc.compress' = 'SNAPPY',          -- 使用SNAPPY压缩
                'transactional' = 'true',           -- 启用事务支持
                'orc.create.index' = 'true',        -- 创建索引
                'skip.header.line.count' = '1',     -- 跳过CSV文件的第一行
                'external.table.purge' = 'true'     -- 在删除表时自动清理数据
            );

            -- 自动加载数据到 Hive 分区中
            ALTER TABLE sales_data
            ADD PARTITION (sale_year = '2024', sale_month = '08')
            LOCATION '/user/hive/warehouse/sales_data/2024/08';"
        );
        let options = FormatOptions {
            indent: Indent::Spaces(4),
            ..Default::default()
        };
        let expected = indoc!(
            "
            -- 创建一个外部表，存储销售数据
            CREATE EXTERNAL TABLE IF NOT EXISTS sales_data (
                -- 唯一标识订单ID
                order_id BIGINT COMMENT 'Unique identifier for the order',
                -- 客户ID
                customer_id BIGINT COMMENT 'Unique identifier for the customer',
            ) COMMENT 'Sales data table for storing transaction records';
            -- 按销售日期和城市进行分区
            PARTITIONED BY (
                sale_year STRING COMMENT 'Year of the sale',
                sale_month STRING COMMENT 'Month of the sale'
            )
            -- 设置数据存储位置
            LOCATION '/user/hive/warehouse/sales_data'
            -- 使用 ORC 存储格式
            STORED AS ORC
            -- 设置表的行格式
            ROW FORMAT DELIMITED FIELDS TERMINATED BY ',' LINES TERMINATED BY '\n'
            -- 设置表属性
            TBLPROPERTIES (
                'orc.compress' = 'SNAPPY',  -- 使用SNAPPY压缩
                'transactional' = 'true',  -- 启用事务支持
                'orc.create.index' = 'true',  -- 创建索引
                'skip.header.line.count' = '1',  -- 跳过CSV文件的第一行
                'external.table.purge' = 'true' -- 在删除表时自动清理数据
            );
            -- 自动加载数据到 Hive 分区中
            ALTER TABLE
                sales_data
                ADD PARTITION (sale_year = '2024', sale_month = '08') LOCATION '/user/hive/warehouse/sales_data/2024/08';"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_returning_clause() {
        let input = indoc!(
            "
          INSERT INTO
            users (name, email)
          VALUES
            ($1, $2) RETURNING name,
            email"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
          INSERT INTO
            users (name, email)
          VALUES
            ($1, $2)
          RETURNING
            name,
            email"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_on_update_clause() {
        let input = indoc!(
            "CREATE TABLE a (b integer REFERENCES c (id) ON                                     UPDATE RESTRICT, other integer);"
        );
        let options = FormatOptions::default();
        let expected = indoc!(
            "
          CREATE TABLE a (
            b integer REFERENCES c (id) ON UPDATE RESTRICT,
            other integer
          );"
        );
        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_except_on_columns() {
        let input = indoc!(
            "SELECT table_0.* EXCEPT (profit),
                    details.* EXCEPT (item_id),
                    table_0.profit
        FROM  table_0"
        );
        let options = FormatOptions {
            indent: Indent::Spaces(4),
            ..Default::default()
        };
        let expected = indoc!(
            "
            SELECT
                table_0.* EXCEPT (profit),
                details.* EXCEPT (item_id),
                table_0.profit
            FROM
                table_0"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_uses_given_ignore_case_convert_config() {
        let input = "select count(*),Column1 from Table1;";
        let options = FormatOptions {
            uppercase: Some(true),
            ignore_case_convert: Some(vec!["from"]),
            ..FormatOptions::default()
        };
        let expected = indoc!(
            "
            SELECT
              count(*),
              Column1
            from
              Table1;"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_recognizes_fmt_off() {
        let input = indoc!(
            "SELECT              *     FROM   sometable
            WHERE
            -- comment test here
                 -- fmt: off
                first_key.second_key = 1
                                -- json:first_key.second_key = 1
                      -- fmt: on
                AND
                   -- fm1t: off
                first_key.second_key = 1
                                    --  json:first_key.second_key = 1
                -- fmt:on"
        );
        let options = FormatOptions {
            indent: Indent::Spaces(4),
            ..Default::default()
        };
        let expected = indoc!(
            "
            SELECT
                *
            FROM
                sometable
            WHERE
                -- comment test here
                first_key.second_key = 1
                                -- json:first_key.second_key = 1
                AND
                -- fm1t: off
                first_key.second_key = 1
                --  json:first_key.second_key = 1"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_converts_keywords_to_lowercase_when_option_passed_in() {
        let input = "select distinct * frOM foo left join bar WHERe cola > 1 and colb = 3";
        let options = FormatOptions {
            uppercase: Some(false),
            ..FormatOptions::default()
        };
        let expected = indoc!(
            "
            select distinct
              *
            from
              foo
              left join bar
            where
              cola > 1
              and colb = 3"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn format_nested_select() {
        let input = "WITH a AS ( SELECT a, b, c FROM t WHERE a > 100 ), aa AS ( SELECT field FROM table ) SELECT b, field FROM a, aa;";
        let options = FormatOptions {
            max_inline_arguments: Some(10),
            max_inline_top_level: Some(9),
            ..Default::default()
        };
        let expected = indoc! {
            "
            WITH
            a AS (
              SELECT a, b, c
              FROM t
              WHERE a > 100
            ),
            aa AS (
              SELECT field
              FROM table
            )
            SELECT
              b, field
            FROM a, aa;"
        };
        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn format_short_with() {
        let input = "WITH a AS ( SELECT a, b, c FROM t WHERE a > 100 ) SELECT b, field FROM a, aa;";
        let max_line = 80;
        let options = FormatOptions {
            max_inline_block: max_line,
            max_inline_arguments: Some(max_line),
            max_inline_top_level: Some(max_line),
            joins_as_top_level: true,
            ..Default::default()
        };
        let expected = indoc! {
            "
            WITH a AS (SELECT a, b, c FROM t WHERE a > 100)
            SELECT b, field
            FROM a, aa;"
        };
        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn format_nested_select_nested_blocks() {
        let input =
            "WITH a AS ( SELECT a, b, c FROM t WHERE a > 100 ), aa AS ( SELECT field FROM table ),
            bb AS ( SELECT count(*) as c FROM d ), cc AS ( INSERT INTO C (a, b, c, d) VALUES (1 ,2 ,3 ,4) )
        SELECT b, field FROM a, aa;";
        let max_line = 20;
        let options = FormatOptions {
            max_inline_block: max_line,
            max_inline_arguments: Some(max_line),
            max_inline_top_level: Some(max_line / 2),
            joins_as_top_level: true,
            ..Default::default()
        };
        let expected = indoc! {
            "
            WITH
            a AS (
              SELECT a, b, c
              FROM t
              WHERE a > 100
            ),
            aa AS (
              SELECT field
              FROM table
            ),
            bb AS (
              SELECT
                count(*) as c
              FROM d
            ),
            cc AS (
              INSERT INTO
                C (a, b, c, d)
              VALUES
                (1, 2, 3, 4)
            )
            SELECT b, field
            FROM a, aa;"
        };
        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_converts_keywords_nothing_when_no_option_passed_in() {
        let input = "select distinct * frOM foo left join bar WHERe cola > 1 and colb = 3";
        let options = FormatOptions {
            uppercase: None,
            ..FormatOptions::default()
        };
        let expected = indoc!(
            "
            select distinct
              *
            frOM
              foo
              left join bar
            WHERe
              cola > 1
              and colb = 3"
        );

        assert_eq!(options.format(input), expected);
    }
    #[test]
    fn it_correctly_parses_all_operators() {
        let operators = [
            "!!", "!~~*", "!~~", "!~*", "!~", "##", "#>>", "#>", "#-", "&<|", "&<", "&>", "&&",
            "*<>", "*<=", "*>=", "*>", "*=", "*<", "<<|", "<<=", "<<", "<->", "<@", "<^", "<=",
            "<>", "<", ">=", ">>=", ">>", ">^", "->>", "->", "-|-", "-", "+", "/", "=", "%", "?||",
            "?|", "?-|", "?-", "?#", "?&", "?", "@@@", "@@", "@>", "@?", "@-@", "@", "^@", "^",
            "|&>", "|>>", "|/", "|", "||/", "||", "~>=~", "~>~", "~<=~", "~<~", "~=", "~*", "~~*",
            "~~", "~", "%", "<%", "%>", "<<%", "%>>", "<<->", "<->>", "<<<->", "<->>>",
        ];

        // Test each operator individually
        for &operator in &operators {
            let input = format!("left {} right", operator);
            let expected = format!("left {} right", operator);
            let options = FormatOptions {
                uppercase: None,
                ..FormatOptions::default()
            };

            assert_eq!(
                options.format(&input),
                expected,
                "Failed to parse operator: {}",
                operator
            );
        }
    }
    #[test]
    fn it_correctly_splits_operators() {
        let input = "
  SELECT
  left <@ right,
  left << right,
  left >> right,
  left &< right,
  left &> right,
  left -|- right,
  @@ left,
  @-@ left,
  left <-> right,
  left <<| right,
  left |>> right,
  left &<| right,
  left |>& right,
  left <^ right,
  left >^ right,
  left <% right,
  left %> right,
  ?- left,
  left ?-| right,
  left ?|| right,
  left ~= right";
        let options = FormatOptions {
            uppercase: None,
            ..FormatOptions::default()
        };
        let expected = indoc!(
            "
SELECT
  left <@ right,
  left << right,
  left >> right,
  left &< right,
  left &> right,
  left -|- right,
  @@ left,
  @-@ left,
  left <-> right,
  left <<| right,
  left |>> right,
  left &<| right,
  left |>& right,
  left <^ right,
  left >^ right,
  left <% right,
  left %> right,
  ?- left,
  left ?-| right,
  left ?|| right,
  left ~= right"
        );

        assert_eq!(options.format(input), expected);
    }
    #[test]
    fn it_formats_double_colons() {
        let input = "select text  ::  text, num::integer, data::json, (x - y)::integer  frOM foo";
        let options = FormatOptions {
            uppercase: Some(false),
            ..FormatOptions::default()
        };
        let expected = indoc!(
            "
select
  text::text,
  num::integer,
  data::json,
  (x - y)::integer
from
  foo"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn it_formats_blocks_inline_or_not() {
        let input = " UPDATE t


        SET o = ($5 + $6 + $7 + $8),a = CASE WHEN $2
            THEN NULL ELSE COALESCE($3, b) END, b = CASE WHEN $4 THEN NULL ELSE
            COALESCE($5, b) END, s = (SELECT true FROM bar WHERE bar.foo = $99 AND bar.foo > $100),
            c = CASE WHEN $6 THEN NULL ELSE COALESCE($7, c) END,
            d = CASE WHEN $8 THEN NULL ELSE COALESCE($9, dddddddd) + bbbbb END,
            e = (SELECT true FROM bar) WHERE id = $1";
        let options = FormatOptions {
            max_inline_arguments: Some(60),
            max_inline_block: 60,
            max_inline_top_level: Some(60),
            ..Default::default()
        };
        let expected = indoc!(
            "
          UPDATE t SET
            o = ($5 + $6 + $7 + $8),
            a = CASE WHEN $2 THEN NULL ELSE COALESCE($3, b) END,
            b = CASE WHEN $4 THEN NULL ELSE COALESCE($5, b) END,
            s = (
              SELECT true
              FROM bar
              WHERE bar.foo = $99
                AND bar.foo > $100
            ),
            c = CASE WHEN $6 THEN NULL ELSE COALESCE($7, c) END,
            d = CASE
              WHEN $8 THEN NULL
              ELSE COALESCE($9, dddddddd) + bbbbb
            END,
            e = (SELECT true FROM bar)
          WHERE id = $1"
        );

        assert_eq!(options.format(input), expected);
    }

    #[test]
    fn parse_union_all() {
        let input = "SELECT id FROM a UNION ALL SELECT id FROM b WHERE c = $12 AND f";
        let options = FormatOptions::default();
        let expected = indoc!(
            "
            SELECT
              id
            FROM
              a
            UNION ALL
            SELECT
              id
            FROM
              b
            WHERE
              c = $12
              AND f"
        );
        assert_eq!(options.format(input), expected);
    }
}
