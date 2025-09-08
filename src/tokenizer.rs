use std::borrow::Cow;
use unicode_categories::UnicodeCategories;
use winnow::ascii::{digit0, digit1, till_line_ending, Caseless};
use winnow::combinator::{alt, dispatch, eof, fail, opt, peek, terminated};
use winnow::error::ContextError;
use winnow::error::ParserError;
use winnow::prelude::*;
use winnow::token::{any, one_of, rest, take, take_until, take_while};
use winnow::Result;

use crate::FormatOptions;

pub(crate) fn tokenize<'a>(
    mut input: &'a str,
    named_placeholders: bool,
    options: &FormatOptions,
) -> Vec<Token<'a>> {
    let mut tokens: Vec<Token> = Vec::new();

    let mut last_non_whitespace_token = None;
    let mut last_reserved_token = None;
    let mut last_reserved_top_level_token = None;

    if let Ok(Some(result)) = opt(get_whitespace_token).parse_next(&mut input) {
        tokens.push(result);
    }

    // Keep processing the string until it is empty
    while let Ok(mut result) = get_next_token(
        &mut input,
        last_non_whitespace_token.clone(),
        last_reserved_token.clone(),
        last_reserved_top_level_token.clone(),
        named_placeholders,
    ) {
        match result.kind {
            TokenKind::Reserved => {
                last_reserved_token = Some(result.clone());
            }
            TokenKind::ReservedTopLevel => {
                last_reserved_top_level_token = Some(result.clone());
            }
            TokenKind::Join => {
                if options.joins_as_top_level {
                    result.kind = TokenKind::ReservedTopLevel;
                } else {
                    result.kind = TokenKind::ReservedNewline;
                }
            }
            _ => {}
        }

        if result.kind != TokenKind::Whitespace {
            last_non_whitespace_token = Some(result.clone());
        }

        tokens.push(result);

        if let Ok(Some(result)) = opt(get_whitespace_token).parse_next(&mut input) {
            tokens.push(result);
        }
    }
    tokens
}

#[derive(Debug, Clone)]
pub(crate) struct Token<'a> {
    pub kind: TokenKind,
    pub value: &'a str,
    // Only used for placeholder--there is a reason this isn't on the enum
    pub key: Option<PlaceholderKind<'a>>,
    /// Used to group the behaviour of variants of tokens
    pub alias: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenKind {
    TypeSpecifier,
    Whitespace,
    String,
    Reserved,
    ReservedTopLevel,
    ReservedTopLevelNoIndent,
    ReservedNewline,
    ReservedNewlineAfter,
    Operator,
    OpenParen,
    CloseParen,
    LineComment,
    BlockComment,
    Number,
    Placeholder,
    Word,
    Join,
}

#[derive(Debug, Clone)]
pub(crate) enum PlaceholderKind<'a> {
    Named(Cow<'a, str>),
    ZeroIndexed(usize),
    OneIndexed(usize),
}

impl<'a> PlaceholderKind<'a> {
    pub fn named(&'a self) -> &'a str {
        match self {
            PlaceholderKind::Named(val) => val.as_ref(),
            _ => "",
        }
    }

    pub fn indexed(&self) -> Option<usize> {
        match self {
            PlaceholderKind::ZeroIndexed(val) => Some(*val),
            PlaceholderKind::OneIndexed(val) => Some(*val - 1),
            _ => None,
        }
    }
}

fn get_next_token<'a>(
    input: &mut &'a str,
    previous_token: Option<Token<'a>>,
    last_reserved_token: Option<Token<'a>>,
    last_reserved_top_level_token: Option<Token<'a>>,
    named_placeholders: bool,
) -> Result<Token<'a>> {
    alt((
        get_comment_token,
        |input: &mut _| get_type_specifier_token(input, previous_token.clone()),
        get_string_token,
        get_open_paren_token,
        get_close_paren_token,
        get_number_token,
        |input: &mut _| {
            get_reserved_word_token(
                input,
                previous_token.clone(),
                last_reserved_token.clone(),
                last_reserved_top_level_token.clone(),
            )
        },
        get_operator_token,
        |input: &mut _| get_placeholder_token(input, named_placeholders),
        get_word_token,
        get_any_other_char,
    ))
    .parse_next(input)
}
fn get_type_specifier_token<'i>(
    input: &mut &'i str,
    previous_token: Option<Token<'i>>,
) -> Result<Token<'i>> {
    if previous_token.is_some_and(|token| {
        ![
            TokenKind::CloseParen,
            TokenKind::Placeholder,
            TokenKind::Reserved,
            TokenKind::String,
            TokenKind::TypeSpecifier,
            TokenKind::Word,
        ]
        .contains(&token.kind)
    }) {
        fail.parse_next(input)
    } else {
        alt(("::", "[]")).parse_next(input).map(|token| Token {
            kind: TokenKind::TypeSpecifier,
            value: token,
            key: None,
            alias: token,
        })
    }
}
fn get_whitespace_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    take_while(1.., char::is_whitespace)
        .parse_next(input)
        .map(|token| Token {
            kind: TokenKind::Whitespace,
            value: token,
            key: None,
            alias: token,
        })
}

fn get_comment_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    dispatch! {any;
        '#' => till_line_ending.value(TokenKind::LineComment),
        '-' => ('-', till_line_ending).value(TokenKind::LineComment),
        '/' => ('*', alt((take_until(0.., "*/"), rest)), opt(take(2usize))).value(TokenKind::BlockComment),
        _ => fail,
    }
        .with_taken()
        .parse_next(input)
        .map(|(kind, token)| Token {
            kind,
            value: token,
            key: None,
            alias: token,
        })
}

pub fn take_till_escaping<'a>(
    desired: char,
    escapes: &'static [char],
) -> impl Parser<&'a str, &'a str, ContextError> {
    move |input: &mut &'a str| {
        let mut chars = input.char_indices().peekable();
        loop {
            let item = chars.next();
            let next = chars.peek().map(|item| item.1);
            match item {
                Some((byte_pos, item)) => {
                    // escape?
                    if escapes.contains(&item) && next.map(|n| n == desired).unwrap_or(false) {
                        // consume this and next char
                        chars.next();
                        continue;
                    }

                    if item == desired {
                        return Ok(input.next_slice(byte_pos));
                    }
                }
                None => {
                    return rest.parse_next(input);
                }
            }
        }
    }
}

// This enables the following string patterns:
// 1. backtick quoted string using `` to escape
// 2. square bracket quoted string (SQL Server) using ]] to escape
// 3. double quoted string using "" or \" to escape
// 4. single quoted string using '' or \' to escape
// 5. national character quoted string using N'' or N\' to escape
// 6. hex(blob literal) does not need to escape
fn get_string_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    dispatch! {any;
        '`' => (take_till_escaping('`', &['`']), any).void(),
        '[' => (take_till_escaping(']', &[']']), any).void(),
        '"' => (take_till_escaping('"', &['"', '\\']), any).void(),
        '\'' => (take_till_escaping('\'', &['\'', '\\']), any).void(),
        'N' => ('\'', take_till_escaping('\'', &['\'', '\\']), any).void(),
        'E' => ('\'', take_till_escaping('\'', &['\'', '\\']), any).void(),
        'x' => ('\'', take_till_escaping('\'', &[]), any).void(),
        'X' => ('\'', take_till_escaping('\'', &[]), any).void(),
        _ => fail,
    }
    .take()
    .parse_next(input)
    .map(|token| Token {
        kind: TokenKind::String,
        value: token,
        key: None,
        alias: token,
    })
}

// Like above but it doesn't replace double quotes
fn get_placeholder_string_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    dispatch! {any;
        '`'=>( take_till_escaping('`', &['`']), any).void(),
        '['=>( take_till_escaping(']', &[']']), any).void(),
        '"'=>( take_till_escaping('"', &['\\']), any).void(),
        '\''=>( take_till_escaping('\'', &['\\']), any).void(),
        'N' =>('\'', take_till_escaping('\'', &['\\']), any).void(),
        _ => fail,
    }
    .take()
    .parse_next(input)
    .map(|token| Token {
        kind: TokenKind::String,
        value: token,
        key: None,
        alias: token,
    })
}

fn get_open_paren_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    alt(("(", terminated(Caseless("CASE"), end_of_word)))
        .parse_next(input)
        .map(|token| Token {
            kind: TokenKind::OpenParen,
            value: token,
            key: None,
            alias: token,
        })
}

fn get_close_paren_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    alt((")", terminated(Caseless("END"), end_of_word)))
        .parse_next(input)
        .map(|token| Token {
            kind: TokenKind::CloseParen,
            value: token,
            key: None,
            alias: token,
        })
}

fn get_placeholder_token<'i>(input: &mut &'i str, named_placeholders: bool) -> Result<Token<'i>> {
    // The precedence changes based on 'named_placeholders' but not the exhaustiveness.
    // This is to ensure the formatting is the same even if parameters aren't used.

    if named_placeholders {
        alt((
            get_ident_named_placeholder_token,
            get_string_named_placeholder_token,
            get_indexed_placeholder_token,
        ))
        .parse_next(input)
    } else {
        alt((
            get_indexed_placeholder_token,
            get_ident_named_placeholder_token,
            get_string_named_placeholder_token,
        ))
        .parse_next(input)
    }
}

fn get_indexed_placeholder_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    alt(((one_of(('?', '$')), digit1).take(), "?"))
        .parse_next(input)
        .map(|token| Token {
            kind: TokenKind::Placeholder,
            value: token,
            key: if token.len() > 1 {
                if let Ok(index) = token[1..].parse::<usize>() {
                    Some(if token.starts_with('$') {
                        PlaceholderKind::OneIndexed(index)
                    } else {
                        PlaceholderKind::ZeroIndexed(index)
                    })
                } else {
                    None
                }
            } else {
                None
            },
            alias: token,
        })
}

fn get_ident_named_placeholder_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    (
        one_of(('@', ':', '$')),
        take_while(1.., |item: char| {
            item.is_alphanumeric() || item == '.' || item == '_' || item == '$'
        }),
    )
        .take()
        .parse_next(input)
        .map(|token| {
            let index = Cow::Borrowed(&token[1..]);
            Token {
                kind: TokenKind::Placeholder,
                value: token,
                key: Some(PlaceholderKind::Named(index)),
                alias: token,
            }
        })
}

fn get_string_named_placeholder_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    (one_of(('@', ':')), get_placeholder_string_token)
        .take()
        .parse_next(input)
        .map(|token| {
            let index =
                get_escaped_placeholder_key(&token[2..token.len() - 1], &token[token.len() - 1..]);
            Token {
                kind: TokenKind::Placeholder,
                value: token,
                key: Some(PlaceholderKind::Named(index)),
                alias: token,
            }
        })
}

fn get_escaped_placeholder_key<'a>(key: &'a str, quote_char: &str) -> Cow<'a, str> {
    Cow::Owned(key.replace(&format!("\\{}", quote_char), quote_char))
}

fn get_number_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    (opt("-"), alt((scientific_notation, decimal_number, digit1)))
        .take()
        .parse_next(input)
        .map(|token| Token {
            kind: TokenKind::Number,
            value: token,
            key: None,
            alias: token,
        })
}

fn decimal_number<'i>(input: &mut &'i str) -> Result<&'i str> {
    (digit1, ".", digit0).take().parse_next(input)
}

fn scientific_notation<'i>(input: &mut &'i str) -> Result<&'i str> {
    (
        alt((decimal_number, digit1)),
        "e",
        opt(one_of(('-', '+'))),
        digit1,
    )
        .take()
        .parse_next(input)
}

fn get_reserved_word_token<'a>(
    input: &mut &'a str,
    previous_token: Option<Token<'a>>,
    last_reserved_token: Option<Token<'a>>,
    last_reserved_top_level_token: Option<Token<'a>>,
) -> Result<Token<'a>> {
    // A reserved word cannot be preceded by a "."
    // this makes it so in "my_table.from", "from" is not considered a reserved word
    if let Some(token) = previous_token {
        if token.value == "." {
            return Err(ParserError::from_input(input));
        }
    }

    if !('a'..='z', 'A'..='Z', '$').contains_token(input.chars().next().unwrap_or('\0')) {
        return Err(ParserError::from_input(input));
    }

    alt((
        get_top_level_reserved_token(last_reserved_top_level_token),
        get_newline_after_reserved_token(),
        get_newline_reserved_token(last_reserved_token),
        get_join_token(),
        get_top_level_reserved_token_no_indent,
        get_plain_reserved_token,
    ))
    .parse_next(input)
}

// We have to be a bit creative here for performance reasons
fn get_uc_words(input: &str, words: usize) -> String {
    input
        .split_whitespace()
        .take(words)
        .collect::<Vec<&str>>()
        .join(" ")
        .to_ascii_uppercase()
}

fn finalize<'a>(input: &mut &'a str, token: &str) -> &'a str {
    let final_word = token.split_whitespace().last().unwrap_or(token);
    let input_end_pos = input.to_ascii_uppercase().find(final_word).unwrap_or(0) + final_word.len();
    input.next_slice(input_end_pos)
}

fn get_top_level_reserved_token<'a>(
    last_reserved_top_level_token: Option<Token<'a>>,
) -> impl Parser<&'a str, Token<'a>, ContextError> {
    move |input: &mut &'a str| {
        let uc_input: String = get_uc_words(input, 3);
        let mut uc_input = uc_input.as_str();

        // First peek at the first character to determine which group to check
        let first_char = peek(any).parse_next(input)?.to_ascii_uppercase();

        // Match keywords based on their first letter
        let result: Result<&str> = match first_char {
            'A' => alt((
                terminated("ADD", end_of_word),
                terminated("AFTER", end_of_word),
                terminated("ALTER COLUMN", end_of_word),
                terminated("ALTER TABLE", end_of_word),
            ))
            .parse_next(&mut uc_input),

            'C' => terminated(
                (
                    "CREATE ",
                    opt(alt((
                        "UNLOGGED ",
                        (
                            alt(("GLOBAL ", "LOCAL ")),
                            opt(alt(("TEMPORARY ", "TEMP "))),
                        )
                            .take(),
                    ))),
                    "TABLE",
                )
                    .take(),
                end_of_word,
            )
            .parse_next(&mut uc_input),

            'D' => terminated("DELETE FROM", end_of_word).parse_next(&mut uc_input),

            'E' => terminated("EXCEPT", end_of_word).parse_next(&mut uc_input),

            'F' => alt((
                terminated("FETCH FIRST", end_of_word),
                terminated("FROM", end_of_word),
            ))
            .parse_next(&mut uc_input),

            'G' => alt((
                terminated("GROUP BY", end_of_word),
                terminated("GO", end_of_word),
            ))
            .parse_next(&mut uc_input),

            'H' => terminated("HAVING", end_of_word).parse_next(&mut uc_input),

            'I' => alt((
                terminated("INSERT INTO", end_of_word),
                terminated("INSERT", end_of_word),
            ))
            .parse_next(&mut uc_input),

            'L' => terminated("LIMIT", end_of_word).parse_next(&mut uc_input),

            'M' => terminated("MODIFY", end_of_word).parse_next(&mut uc_input),

            'O' => alt((
                terminated("ORDER BY", end_of_word),
                terminated("ON CONFLICT", end_of_word),
            ))
            .parse_next(&mut uc_input),

            'P' => terminated("PARTITION BY", end_of_word).parse_next(&mut uc_input),

            'R' => terminated("RETURNING", end_of_word).parse_next(&mut uc_input),

            'S' => alt((
                terminated("SELECT DISTINCT", end_of_word),
                terminated("SELECT ALL", end_of_word),
                terminated("SELECT", end_of_word),
                terminated("SET CURRENT SCHEMA", end_of_word),
                terminated("SET SCHEMA", end_of_word),
                terminated("SET", end_of_word),
            ))
            .parse_next(&mut uc_input),

            'U' => terminated("UPDATE", end_of_word).parse_next(&mut uc_input),

            'V' => terminated("VALUES", end_of_word).parse_next(&mut uc_input),

            'W' => alt((
                terminated("WHERE", end_of_word),
                terminated("WINDOW", end_of_word),
            ))
            .parse_next(&mut uc_input),

            // If the first character doesn't match any of our keywords, fail early
            _ => Err(ParserError::from_input(&uc_input)),
        };

        if let Ok(token) = result {
            let token = finalize(input, token);

            let kind = match token {
                "EXCEPT"
                    if last_reserved_top_level_token.is_some()
                        && last_reserved_top_level_token.as_ref().unwrap().alias == "SELECT" =>
                // If the query state doesn't allow EXCEPT, treat it as a regular word
                {
                    TokenKind::Reserved
                }
                "SET"
                    if last_reserved_top_level_token.is_some()
                        && last_reserved_top_level_token.as_ref().unwrap().value == "UPDATE" =>
                {
                    TokenKind::ReservedNewlineAfter
                }
                _ => TokenKind::ReservedTopLevel,
            };

            let alias = if token.starts_with("CREATE") {
                "CREATE"
            } else if token.starts_with("SELECT") {
                "SELECT"
            } else {
                token
            };

            Ok(Token {
                kind,
                value: token,
                key: None,
                alias,
            })
        } else {
            Err(ParserError::from_input(input))
        }
    }
}

fn get_join_token<'a>() -> impl Parser<&'a str, Token<'a>, ContextError> {
    move |input: &mut &'a str| {
        let uc_input: String = get_uc_words(input, 3);
        let mut uc_input = uc_input.as_str();

        // Standard SQL joins
        let standard_joins = alt((
            terminated("JOIN", end_of_word),
            terminated("INNER JOIN", end_of_word),
            terminated("LEFT JOIN", end_of_word),
            terminated("RIGHT JOIN", end_of_word),
            terminated("FULL JOIN", end_of_word),
            terminated("CROSS JOIN", end_of_word),
            terminated("LEFT OUTER JOIN", end_of_word),
            terminated("RIGHT OUTER JOIN", end_of_word),
            terminated("FULL OUTER JOIN", end_of_word),
        ));

        // Warehouse-specific ANY/SEMI/ANTI joins
        let specific_joins = alt((
            terminated("INNER ANY JOIN", end_of_word),
            terminated("LEFT ANY JOIN", end_of_word),
            terminated("RIGHT ANY JOIN", end_of_word),
            terminated("ANY JOIN", end_of_word),
            terminated("SEMI JOIN", end_of_word),
            terminated("LEFT SEMI JOIN", end_of_word),
            terminated("RIGHT SEMI JOIN", end_of_word),
            terminated("LEFT ANTI JOIN", end_of_word),
            terminated("RIGHT ANTI JOIN", end_of_word),
        ));

        // Special joins and GLOBAL variants
        let special_joins = alt((
            terminated("ASOF JOIN", end_of_word),
            terminated("LEFT ASOF JOIN", end_of_word),
            terminated("PASTE JOIN", end_of_word),
            terminated("GLOBAL INNER JOIN", end_of_word),
            terminated("GLOBAL LEFT JOIN", end_of_word),
            terminated("GLOBAL RIGHT JOIN", end_of_word),
            terminated("GLOBAL FULL JOIN", end_of_word),
        ));

        // Combine all parsers
        let result: Result<&str> =
            alt((standard_joins, specific_joins, special_joins)).parse_next(&mut uc_input);

        if let Ok(token) = result {
            let final_word = token.split(' ').next_back().unwrap();
            let input_end_pos =
                input.to_ascii_uppercase().find(final_word).unwrap() + final_word.len();
            let token = input.next_slice(input_end_pos);
            let kind = TokenKind::Join;
            Ok(Token {
                kind,
                value: token,
                key: None,
                alias: token,
            })
        } else {
            Err(ParserError::from_input(input))
        }
    }
}

fn get_newline_after_reserved_token<'a>() -> impl Parser<&'a str, Token<'a>, ContextError> {
    move |input: &mut &'a str| {
        let uc_input: String = get_uc_words(input, 3);
        let mut uc_input = uc_input.as_str();

        let mut on_conflict = alt((
            terminated("DO NOTHING", end_of_word),
            terminated("DO UPDATE SET", end_of_word),
        ));

        let result: Result<&str> = on_conflict.parse_next(&mut uc_input);

        if let Ok(token) = result {
            let value = finalize(input, token);
            Ok(Token {
                kind: TokenKind::ReservedNewlineAfter,
                value,
                key: None,
                alias: value,
            })
        } else {
            Err(ParserError::from_input(input))
        }
    }
}

fn get_newline_reserved_token<'a>(
    last_reserved_token: Option<Token<'a>>,
) -> impl Parser<&'a str, Token<'a>, ContextError> {
    move |input: &mut &'a str| {
        let uc_input: String = get_uc_words(input, 3);
        let mut uc_input = uc_input.as_str();

        // We have to break up the alternatives into multiple subsets
        // to avoid exceeding the alt() 21 element limit.

        // Legacy and logical operators
        let operators = alt((
            terminated("CROSS APPLY", end_of_word),
            terminated("OUTER APPLY", end_of_word),
            terminated("AND", end_of_word),
            terminated("OR", end_of_word),
            terminated("XOR", end_of_word),
            terminated("WHEN", end_of_word),
            terminated("ELSE", end_of_word),
        ));

        let alter_table_actions = alt((
            terminated("ADD", end_of_word),
            terminated("DROP", end_of_word),
            terminated("ALTER", end_of_word),
            terminated("VALIDATE", end_of_word),
            terminated("ENABLE", end_of_word),
            terminated("DISABLE", end_of_word),
        ));

        // Combine all parsers
        let result: Result<&str> = alt((operators, alter_table_actions)).parse_next(&mut uc_input);

        if let Ok(token) = result {
            let token = finalize(input, token);
            let kind = if token == "AND"
                && last_reserved_token.is_some()
                && last_reserved_token.as_ref().unwrap().value == "BETWEEN"
            {
                // If the "AND" is part of a "BETWEEN" clause, we want to handle it as one clause by not adding a new line.
                TokenKind::Reserved
            } else {
                TokenKind::ReservedNewline
            };
            Ok(Token {
                kind,
                value: token,
                key: None,
                alias: token,
            })
        } else {
            Err(ParserError::from_input(input))
        }
    }
}

fn get_top_level_reserved_token_no_indent<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    let uc_input = get_uc_words(input, 2);
    let mut uc_input = uc_input.as_str();

    let result: Result<&str> = alt((
        terminated("BEGIN", end_of_word),
        terminated("DECLARE", end_of_word),
        terminated("INTERSECT ALL", end_of_word),
        terminated("INTERSECT", end_of_word),
        terminated("MINUS", end_of_word),
        terminated("UNION ALL", end_of_word),
        terminated("UNION", end_of_word),
        terminated("WITH", end_of_word),
        terminated("$$", end_of_word),
    ))
    .parse_next(&mut uc_input);
    if let Ok(token) = result {
        let value = finalize(input, token);
        Ok(Token {
            kind: TokenKind::ReservedTopLevelNoIndent,
            value,
            key: None,
            alias: value,
        })
    } else {
        Err(ParserError::from_input(input))
    }
}
fn get_plain_reserved_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    alt((get_plain_reserved_two_token, get_plain_reserved_one_token)).parse_next(input)
}
fn get_plain_reserved_one_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    let uc_input = get_uc_words(input, 1);
    let mut uc_input = uc_input.as_str();

    let first_char = peek(any).parse_next(input)?.to_ascii_uppercase();

    let result: Result<&str> = match first_char {
        'A' => alt((
            terminated("ACCESSIBLE", end_of_word),
            terminated("ACTION", end_of_word),
            terminated("AGAINST", end_of_word),
            terminated("AGGREGATE", end_of_word),
            terminated("ALGORITHM", end_of_word),
            terminated("ALL", end_of_word),
            terminated("ALTER", end_of_word),
            terminated("ANALYSE", end_of_word),
            terminated("ANALYZE", end_of_word),
            terminated("AS", end_of_word),
            terminated("ASC", end_of_word),
            terminated("AUTOCOMMIT", end_of_word),
            terminated("AUTO_INCREMENT", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'B' => alt((
            terminated("BACKUP", end_of_word),
            terminated("BETWEEN", end_of_word),
            terminated("BINLOG", end_of_word),
            terminated("BOTH", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'C' => alt((
            terminated("CASCADE", end_of_word),
            terminated("CASE", end_of_word),
            terminated("CHANGE", end_of_word),
            terminated("CHANGED", end_of_word),
            terminated("CHARSET", end_of_word),
            terminated("CHECK", end_of_word),
            terminated("CHECKSUM", end_of_word),
            terminated("COLLATE", end_of_word),
            terminated("COLLATION", end_of_word),
            terminated("COLUMN", end_of_word),
            terminated("COLUMNS", end_of_word),
            terminated("COMMENT", end_of_word),
            terminated("COMMIT", end_of_word),
            terminated("COMMITTED", end_of_word),
            terminated("COMPRESSED", end_of_word),
            terminated("CONCURRENT", end_of_word),
            terminated("CONSTRAINT", end_of_word),
            terminated("CONTAINS", end_of_word),
            alt((
                terminated("CONVERT", end_of_word),
                terminated("CREATE", end_of_word),
                terminated("CROSS", end_of_word),
                terminated("CURRENT_TIMESTAMP", end_of_word),
            )),
        ))
        .parse_next(&mut uc_input),

        'D' => alt((
            terminated("DATABASE", end_of_word),
            terminated("DATABASES", end_of_word),
            terminated("DAY", end_of_word),
            terminated("DAY_HOUR", end_of_word),
            terminated("DAY_MINUTE", end_of_word),
            terminated("DAY_SECOND", end_of_word),
            terminated("DEFAULT", end_of_word),
            terminated("DEFINER", end_of_word),
            terminated("DELAYED", end_of_word),
            terminated("DELETE", end_of_word),
            terminated("DESC", end_of_word),
            terminated("DESCRIBE", end_of_word),
            terminated("DETERMINISTIC", end_of_word),
            terminated("DISTINCT", end_of_word),
            terminated("DISTINCTROW", end_of_word),
            terminated("DIV", end_of_word),
            terminated("DO", end_of_word),
            terminated("DROP", end_of_word),
            terminated("DUMPFILE", end_of_word),
            terminated("DUPLICATE", end_of_word),
            terminated("DYNAMIC", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'E' => alt((
            terminated("ELSE", end_of_word),
            terminated("ENCLOSED", end_of_word),
            terminated("END", end_of_word),
            terminated("ENGINE", end_of_word),
            terminated("ENGINES", end_of_word),
            terminated("ENGINE_TYPE", end_of_word),
            terminated("ESCAPE", end_of_word),
            terminated("ESCAPED", end_of_word),
            terminated("EVENTS", end_of_word),
            terminated("EXEC", end_of_word),
            terminated("EXECUTE", end_of_word),
            terminated("EXISTS", end_of_word),
            terminated("EXPLAIN", end_of_word),
            terminated("EXTENDED", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'F' => alt((
            terminated("FAST", end_of_word),
            terminated("FETCH", end_of_word),
            terminated("FIELDS", end_of_word),
            terminated("FILE", end_of_word),
            terminated("FIRST", end_of_word),
            terminated("FIXED", end_of_word),
            terminated("FLUSH", end_of_word),
            terminated("FOR", end_of_word),
            terminated("FORCE", end_of_word),
            terminated("FOREIGN", end_of_word),
            terminated("FULL", end_of_word),
            terminated("FULLTEXT", end_of_word),
            terminated("FUNCTION", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'G' => alt((
            terminated("GLOBAL", end_of_word),
            terminated("GRANT", end_of_word),
            terminated("GRANTS", end_of_word),
            terminated("GROUP_CONCAT", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'H' => alt((
            terminated("HEAP", end_of_word),
            terminated("HIGH_PRIORITY", end_of_word),
            terminated("HOSTS", end_of_word),
            terminated("HOUR", end_of_word),
            terminated("HOUR_MINUTE", end_of_word),
            terminated("HOUR_SECOND", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'I' => alt((
            terminated("IDENTIFIED", end_of_word),
            terminated("IF", end_of_word),
            terminated("IFNULL", end_of_word),
            terminated("IGNORE", end_of_word),
            terminated("IN", end_of_word),
            terminated("INDEX", end_of_word),
            terminated("INDEXES", end_of_word),
            terminated("INFILE", end_of_word),
            terminated("INSERT", end_of_word),
            terminated("INSERT_ID", end_of_word),
            terminated("INSERT_METHOD", end_of_word),
            terminated("INTERVAL", end_of_word),
            terminated("INTO", end_of_word),
            terminated("INVOKER", end_of_word),
            terminated("IS", end_of_word),
            terminated("ISOLATION", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'K' => alt((
            terminated("KEY", end_of_word),
            terminated("KEYS", end_of_word),
            terminated("KILL", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'L' => alt((
            terminated("LAST_INSERT_ID", end_of_word),
            terminated("LEADING", end_of_word),
            terminated("LEVEL", end_of_word),
            terminated("LIKE", end_of_word),
            terminated("LINEAR", end_of_word),
            terminated("LINES", end_of_word),
            terminated("LOAD", end_of_word),
            terminated("LOCAL", end_of_word),
            terminated("LOCK", end_of_word),
            terminated("LOCKS", end_of_word),
            terminated("LOGS", end_of_word),
            terminated("LOW_PRIORITY", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'M' => alt((
            terminated("MARIA", end_of_word),
            terminated("MASTER", end_of_word),
            terminated("MASTER_CONNECT_RETRY", end_of_word),
            terminated("MASTER_HOST", end_of_word),
            terminated("MASTER_LOG_FILE", end_of_word),
            terminated("MATCH", end_of_word),
            terminated("MAX_CONNECTIONS_PER_HOUR", end_of_word),
            terminated("MAX_QUERIES_PER_HOUR", end_of_word),
            terminated("MAX_ROWS", end_of_word),
            terminated("MAX_UPDATES_PER_HOUR", end_of_word),
            terminated("MAX_USER_CONNECTIONS", end_of_word),
            terminated("MEDIUM", end_of_word),
            terminated("MERGE", end_of_word),
            terminated("MINUTE", end_of_word),
            terminated("MINUTE_SECOND", end_of_word),
            terminated("MIN_ROWS", end_of_word),
            terminated("MODE", end_of_word),
            terminated("MODIFY", end_of_word),
            terminated("MONTH", end_of_word),
            terminated("MRG_MYISAM", end_of_word),
            terminated("MYISAM", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'N' => alt((
            terminated("NAMES", end_of_word),
            terminated("NATURAL", end_of_word),
            terminated("NOT", end_of_word),
            terminated("NOW()", end_of_word),
            terminated("NULL", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'O' => alt((
            terminated("OFFSET", end_of_word),
            terminated("ON", end_of_word),
            terminated("ONLY", end_of_word),
            terminated("OPEN", end_of_word),
            terminated("OPTIMIZE", end_of_word),
            terminated("OPTION", end_of_word),
            terminated("OPTIONALLY", end_of_word),
            terminated("OUTFILE", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'P' => alt((
            terminated("PACK_KEYS", end_of_word),
            terminated("PAGE", end_of_word),
            terminated("PARTIAL", end_of_word),
            terminated("PARTITION", end_of_word),
            terminated("PARTITIONS", end_of_word),
            terminated("PASSWORD", end_of_word),
            terminated("PRIMARY", end_of_word),
            terminated("PRIVILEGES", end_of_word),
            terminated("PROCEDURE", end_of_word),
            terminated("PROCESS", end_of_word),
            terminated("PROCESSLIST", end_of_word),
            terminated("PURGE", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'Q' => terminated("QUICK", end_of_word).parse_next(&mut uc_input),

        'R' => alt((
            terminated("RAID0", end_of_word),
            terminated("RAID_CHUNKS", end_of_word),
            terminated("RAID_CHUNKSIZE", end_of_word),
            terminated("RAID_TYPE", end_of_word),
            terminated("RANGE", end_of_word),
            terminated("READ", end_of_word),
            terminated("READ_ONLY", end_of_word),
            terminated("READ_WRITE", end_of_word),
            terminated("REFERENCES", end_of_word),
            terminated("REGEXP", end_of_word),
            terminated("RELOAD", end_of_word),
            terminated("RENAME", end_of_word),
            terminated("REPAIR", end_of_word),
            terminated("REPEATABLE", end_of_word),
            terminated("REPLACE", end_of_word),
            terminated("REPLICATION", end_of_word),
            terminated("RESET", end_of_word),
            alt((
                terminated("RESTORE", end_of_word),
                terminated("RESTRICT", end_of_word),
                terminated("RETURN", end_of_word),
                terminated("RETURNS", end_of_word),
                terminated("REVOKE", end_of_word),
                terminated("RLIKE", end_of_word),
                terminated("ROLLBACK", end_of_word),
                terminated("ROW", end_of_word),
                terminated("ROWS", end_of_word),
                terminated("ROW_FORMAT", end_of_word),
            )),
        ))
        .parse_next(&mut uc_input),

        'S' => alt((
            terminated("SECOND", end_of_word),
            terminated("SECURITY", end_of_word),
            terminated("SEPARATOR", end_of_word),
            terminated("SERIALIZABLE", end_of_word),
            terminated("SESSION", end_of_word),
            terminated("SHARE", end_of_word),
            terminated("SHOW", end_of_word),
            terminated("SHUTDOWN", end_of_word),
            terminated("SLAVE", end_of_word),
            terminated("SONAME", end_of_word),
            terminated("SOUNDS", end_of_word),
            terminated("SQL", end_of_word),
            terminated("SQL_AUTO_IS_NULL", end_of_word),
            terminated("SQL_BIG_RESULT", end_of_word),
            terminated("SQL_BIG_SELECTS", end_of_word),
            terminated("SQL_BIG_TABLES", end_of_word),
            terminated("SQL_BUFFER_RESULT", end_of_word),
            terminated("SQL_CACHE", end_of_word),
            alt((
                terminated("SQL_CALC_FOUND_ROWS", end_of_word),
                terminated("SQL_LOG_BIN", end_of_word),
                terminated("SQL_LOG_OFF", end_of_word),
                terminated("SQL_LOG_UPDATE", end_of_word),
                terminated("SQL_LOW_PRIORITY_UPDATES", end_of_word),
                terminated("SQL_MAX_JOIN_SIZE", end_of_word),
                terminated("SQL_NO_CACHE", end_of_word),
                terminated("SQL_QUOTE_SHOW_CREATE", end_of_word),
                terminated("SQL_BIG_RESULT", end_of_word),
                terminated("SQL_BIG_SELECTS", end_of_word),
                terminated("SQL_BIG_TABLES", end_of_word),
                terminated("SQL_BUFFER_RESULT", end_of_word),
                terminated("SQL_CACHE", end_of_word),
                terminated("SQL_CALC_FOUND_ROWS", end_of_word),
                terminated("SQL_LOG_BIN", end_of_word),
                terminated("SQL_LOG_OFF", end_of_word),
                terminated("SQL_LOG_UPDATE", end_of_word),
                terminated("SQL_LOW_PRIORITY_UPDATES", end_of_word),
                terminated("SQL_MAX_JOIN_SIZE", end_of_word),
                alt((
                    terminated("SQL_NO_CACHE", end_of_word),
                    terminated("SQL_QUOTE_SHOW_CREATE", end_of_word),
                    terminated("SQL_SAFE_UPDATES", end_of_word),
                    terminated("SQL_SELECT_LIMIT", end_of_word),
                    terminated("SQL_SLAVE_SKIP_COUNTER", end_of_word),
                    terminated("SQL_SMALL_RESULT", end_of_word),
                    terminated("SQL_WARNINGS", end_of_word),
                    terminated("START", end_of_word),
                    terminated("STARTING", end_of_word),
                    terminated("STATUS", end_of_word),
                    terminated("STOP", end_of_word),
                    terminated("STORAGE", end_of_word),
                    terminated("STRAIGHT_JOIN", end_of_word),
                    terminated("STRING", end_of_word),
                    terminated("STRIPED", end_of_word),
                    terminated("SUPER", end_of_word),
                )),
            )),
        ))
        .parse_next(&mut uc_input),

        'T' => alt((
            terminated("TABLE", end_of_word),
            terminated("TABLES", end_of_word),
            terminated("TEMPORARY", end_of_word),
            terminated("TERMINATED", end_of_word),
            terminated("THEN", end_of_word),
            terminated("TO", end_of_word),
            terminated("TRAILING", end_of_word),
            terminated("TRANSACTIONAL", end_of_word),
            terminated("TRUE", end_of_word),
            terminated("TRUNCATE", end_of_word),
            terminated("TYPE", end_of_word),
            terminated("TYPES", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'U' => alt((
            terminated("UNCOMMITTED", end_of_word),
            terminated("UNIQUE", end_of_word),
            terminated("UNLOCK", end_of_word),
            terminated("UNSIGNED", end_of_word),
            terminated("USAGE", end_of_word),
            terminated("USE", end_of_word),
            terminated("USING", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'V' => alt((
            terminated("VARIABLES", end_of_word),
            terminated("VIEW", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'W' => alt((
            terminated("WHEN", end_of_word),
            terminated("WORK", end_of_word),
            terminated("WRITE", end_of_word),
        ))
        .parse_next(&mut uc_input),

        'Y' => alt((terminated("YEAR_MONTH", end_of_word),)).parse_next(&mut uc_input),
        // If the first character doesn't match any of our keywords, fail early
        _ => Err(ParserError::from_input(&uc_input)),
    };
    if let Ok(token) = result {
        let input_end_pos = token.len();
        let token = input.next_slice(input_end_pos);
        Ok(Token {
            kind: TokenKind::Reserved,
            value: token,
            key: None,
            alias: token,
        })
    } else {
        Err(ParserError::from_input(input))
    }
}

fn get_plain_reserved_two_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    let uc_input = get_uc_words(input, 2);
    let mut uc_input = uc_input.as_str();
    let result: Result<&str> = alt((
        terminated("CHARACTER SET", end_of_word),
        terminated("ON CONFLICT", end_of_word),
        terminated("ON CONSTRAINT", end_of_word),
        terminated("ON DELETE", end_of_word),
        terminated("ON UPDATE", end_of_word),
        terminated("DISTINCT FROM", end_of_word),
    ))
    .parse_next(&mut uc_input);
    if let Ok(token) = result {
        let value = finalize(input, token);
        Ok(Token {
            kind: TokenKind::Reserved,
            value,
            key: None,
            alias: value,
        })
    } else {
        Err(ParserError::from_input(input))
    }
}

fn get_word_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    take_while(1.., is_word_character)
        .parse_next(input)
        .map(|token| Token {
            kind: TokenKind::Word,
            value: token,
            key: None,
            alias: token,
        })
}

fn get_operator_token<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    // Define the allowed operator characters
    let allowed_operators = (
        '!', '<', '>', '=', '|', ':', '-', '~', '*', '&', '@', '^', '?', '#', '/', '%',
    );

    take_while(2..=5, allowed_operators)
        .map(|token: &str| Token {
            kind: TokenKind::Operator,
            value: token,
            key: None,
            alias: token,
        })
        .parse_next(input)
}
fn get_any_other_char<'i>(input: &mut &'i str) -> Result<Token<'i>> {
    one_of(|token| token != '\n' && token != '\r')
        .take()
        .parse_next(input)
        .map(|token| Token {
            kind: TokenKind::Operator,
            value: token,
            key: None,
            alias: token,
        })
}

fn end_of_word<'i>(input: &mut &'i str) -> Result<&'i str> {
    peek(alt((
        eof,
        one_of(|val: char| !is_word_character(val)).take(),
    )))
    .parse_next(input)
}

fn is_word_character(item: char) -> bool {
    item.is_alphanumeric() || item.is_mark() || item.is_punctuation_connector()
}
