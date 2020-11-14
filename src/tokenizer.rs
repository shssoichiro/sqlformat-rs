use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take_while, take_while1};
use nom::character::complete::{char, digit0, digit1, not_line_ending, space1};
use nom::combinator::{eof, opt, recognize};
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::IResult;
use std::borrow::Cow;
use unicode_categories::UnicodeCategories;

pub(crate) fn tokenize<'a>(mut input: &'a str) -> Vec<Token<'a>> {
    let mut tokens = Vec::new();

    // Keep processing the string until it is empty
    while let Ok(result) = get_next_token(input) {
        input = result.0;
        tokens.push(result.1);
    }
    tokens
}

#[derive(Debug, Clone)]
pub(crate) struct Token<'a> {
    pub kind: TokenKind,
    pub value: &'a str,
    // Only used for placeholder--there is a reason this isn't on the enum
    pub key: Option<PlaceholderKind<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Whitespace,
    String,
    Reserved,
    ReservedTopLevel,
    ReservedTopLevelNoIndent,
    ReservedNewline,
    Operator,
    OpenParen,
    CloseParen,
    LineComment,
    BlockComment,
    Number,
    Placeholder,
    Word,
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

fn get_next_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    get_whitespace_token(input)
        .or_else(|_| get_comment_token(input))
        .or_else(|_| get_string_token(input))
        .or_else(|_| get_open_paren_token(input))
        .or_else(|_| get_close_paren_token(input))
        .or_else(|_| get_placeholder_token(input))
        .or_else(|_| get_number_token(input))
        .or_else(|_| get_reserved_word_token(input))
        .or_else(|_| get_word_token(input))
        .or_else(|_| get_operator_token(input))
}

fn get_whitespace_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    take_while1(char::is_whitespace)(input).map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::Whitespace,
                value: token,
                key: None,
            },
        )
    })
}

fn get_comment_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    get_line_comment_token(input).or_else(|_| get_block_comment_token(input))
}

fn get_line_comment_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    preceded(alt((tag("#"), tag("--"))), not_line_ending)(input).map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::LineComment,
                value: token,
                key: None,
            },
        )
    })
}

fn get_block_comment_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    delimited(tag("/*"), take_while(|_| true), alt((tag("*/"), eof)))(input).map(
        |(input, token)| {
            (
                input,
                Token {
                    kind: TokenKind::BlockComment,
                    value: token,
                    key: None,
                },
            )
        },
    )
}

// This enables the following string patterns:
// 1. backtick quoted string using `` to escape
// 2. square bracket quoted string (SQL Server) using ]] to escape
// 3. double quoted string using "" or \" to escape
// 4. single quoted string using '' or \' to escape
// 5. national character quoted string using N'' or N\' to escape
fn get_string_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        delimited(char('`'), take_while(|_| true), char('`')),
        delimited(char('['), take_while(|_| true), char(']')),
        delimited(char('"'), take_while(|_| true), char('"')),
        delimited(char('\''), take_while(|_| true), char('\'')),
        delimited(tag("N'"), take_while(|_| true), char('\'')),
    ))(input)
    .map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::String,
                value: token,
                key: None,
            },
        )
    })
}

fn get_open_paren_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((tag("("), tag_no_case("CASE")))(input).map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::OpenParen,
                value: token,
                key: None,
            },
        )
    })
}

fn get_close_paren_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((tag(")"), tag_no_case("END")))(input).map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::CloseParen,
                value: token,
                key: None,
            },
        )
    })
}

fn get_placeholder_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        get_indexed_placeholder_token,
        get_ident_named_placeholder_token,
        get_string_named_placeholder_token,
    ))(input)
}

//pub(crate) const INDEXED_PLACEHOLDER_TYPES: &[&str] = &["?", "$"];
//
//pub(crate) const NAMED_PLACEHOLDER_TYPES: &[&str] = &["@", ":"];

fn get_indexed_placeholder_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    recognize(tuple((alt((char('?'), char('$'))), digit1)))(input).map(|(input, token)| {
        let index = token[1..].parse::<usize>().unwrap();
        (
            input,
            Token {
                kind: TokenKind::Placeholder,
                value: token,
                key: if token.starts_with('$') {
                    Some(PlaceholderKind::OneIndexed(index))
                } else {
                    Some(PlaceholderKind::ZeroIndexed(index))
                },
            },
        )
    })
}

fn get_ident_named_placeholder_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    recognize(tuple((
        alt((char('@'), char(':'))),
        take_while(|item: char| {
            item.is_alphanumeric() || item == '.' || item == '_' || item == '$'
        }),
    )))(input)
    .map(|(input, token)| {
        let index = Cow::Borrowed(&token[1..]);
        (
            input,
            Token {
                kind: TokenKind::Placeholder,
                value: token,
                key: Some(PlaceholderKind::Named(index)),
            },
        )
    })
}

fn get_string_named_placeholder_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    recognize(tuple((alt((char('@'), char(':'))), get_string_token)))(input).map(
        |(input, token)| {
            let index = Cow::Borrowed(&token[1..]);
            (
                input,
                Token {
                    kind: TokenKind::Placeholder,
                    value: token,
                    key: Some(PlaceholderKind::Named(index)),
                },
            )
        },
    )
}

fn get_number_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    recognize(tuple((opt(tag("-")), alt((decimal_number, digit1)))))(input).map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::Number,
                value: token,
                key: None,
            },
        )
    })
}

fn decimal_number<'a>(input: &'a str) -> IResult<&'a str, &'a str> {
    recognize(tuple((digit1, tag("."), digit0)))(input)
}

fn get_reserved_word_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        get_top_level_reserved_token,
        get_newline_reserved_token,
        get_top_level_reserved_token_no_indent,
        get_plain_reserved_token,
    ))(input)
}

fn get_top_level_reserved_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        terminated(tag_no_case("ADD"), space1),
        terminated(tag_no_case("AFTER"), space1),
        terminated(tag_no_case("ALTER COLUMN"), space1),
        terminated(tag_no_case("ALTER TABLE"), space1),
        terminated(tag_no_case("DELETE FROM"), space1),
        terminated(tag_no_case("EXCEPT"), space1),
        terminated(tag_no_case("FETCH FIRST"), space1),
        terminated(tag_no_case("FROM"), space1),
        terminated(tag_no_case("GROUP BY"), space1),
        terminated(tag_no_case("GO"), space1),
        terminated(tag_no_case("HAVING"), space1),
        terminated(tag_no_case("INSERT INTO"), space1),
        terminated(tag_no_case("INSERT"), space1),
        terminated(tag_no_case("LIMIT"), space1),
        terminated(tag_no_case("MODIFY"), space1),
        terminated(tag_no_case("ORDER BY"), space1),
        terminated(tag_no_case("SELECT"), space1),
        terminated(tag_no_case("SET CURRENT SCHEMA"), space1),
        terminated(tag_no_case("SET SCHEMA"), space1),
        terminated(tag_no_case("SET"), space1),
        alt((
            terminated(tag_no_case("UPDATE"), space1),
            terminated(tag_no_case("VALUES"), space1),
            terminated(tag_no_case("WHERE"), space1),
        )),
    ))(input)
    .map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::ReservedTopLevel,
                value: token,
                key: None,
            },
        )
    })
}

fn get_newline_reserved_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        terminated(tag_no_case("AND"), space1),
        terminated(tag_no_case("CROSS APPLY"), space1),
        terminated(tag_no_case("CROSS JOIN"), space1),
        terminated(tag_no_case("ELSE"), space1),
        terminated(tag_no_case("INNER JOIN"), space1),
        terminated(tag_no_case("JOIN"), space1),
        terminated(tag_no_case("LEFT JOIN"), space1),
        terminated(tag_no_case("LEFT OUTER JOIN"), space1),
        terminated(tag_no_case("OR"), space1),
        terminated(tag_no_case("OUTER APPLY"), space1),
        terminated(tag_no_case("OUTER JOIN"), space1),
        terminated(tag_no_case("RIGHT JOIN"), space1),
        terminated(tag_no_case("RIGHT OUTER JOIN"), space1),
        terminated(tag_no_case("WHEN"), space1),
        terminated(tag_no_case("XOR"), space1),
    ))(input)
    .map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::ReservedNewline,
                value: token,
                key: None,
            },
        )
    })
}

fn get_top_level_reserved_token_no_indent<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        terminated(tag_no_case("INTERSECT"), space1),
        terminated(tag_no_case("INTERSECT ALL"), space1),
        terminated(tag_no_case("MINUS"), space1),
        terminated(tag_no_case("UNION"), space1),
        terminated(tag_no_case("UNION ALL"), space1),
    ))(input)
    .map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::ReservedTopLevelNoIndent,
                value: token,
                key: None,
            },
        )
    })
}

fn get_plain_reserved_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        terminated(tag_no_case("ACCESSIBLE"), space1),
        terminated(tag_no_case("ACTION"), space1),
        terminated(tag_no_case("AGAINST"), space1),
        terminated(tag_no_case("AGGREGATE"), space1),
        terminated(tag_no_case("ALGORITHM"), space1),
        terminated(tag_no_case("ALL"), space1),
        terminated(tag_no_case("ALTER"), space1),
        terminated(tag_no_case("ANALYSE"), space1),
        terminated(tag_no_case("ANALYZE"), space1),
        terminated(tag_no_case("AS"), space1),
        terminated(tag_no_case("ASC"), space1),
        terminated(tag_no_case("AUTOCOMMIT"), space1),
        terminated(tag_no_case("AUTO_INCREMENT"), space1),
        terminated(tag_no_case("BACKUP"), space1),
        terminated(tag_no_case("BEGIN"), space1),
        terminated(tag_no_case("BETWEEN"), space1),
        terminated(tag_no_case("BINLOG"), space1),
        terminated(tag_no_case("BOTH"), space1),
        terminated(tag_no_case("CASCADE"), space1),
        terminated(tag_no_case("CASE"), space1),
        alt((
            terminated(tag_no_case("CHANGE"), space1),
            terminated(tag_no_case("CHANGED"), space1),
            terminated(tag_no_case("CHARACTER SET"), space1),
            terminated(tag_no_case("CHARSET"), space1),
            terminated(tag_no_case("CHECK"), space1),
            terminated(tag_no_case("CHECKSUM"), space1),
            terminated(tag_no_case("COLLATE"), space1),
            terminated(tag_no_case("COLLATION"), space1),
            terminated(tag_no_case("COLUMN"), space1),
            terminated(tag_no_case("COLUMNS"), space1),
            terminated(tag_no_case("COMMENT"), space1),
            terminated(tag_no_case("COMMIT"), space1),
            terminated(tag_no_case("COMMITTED"), space1),
            terminated(tag_no_case("COMPRESSED"), space1),
            terminated(tag_no_case("CONCURRENT"), space1),
            terminated(tag_no_case("CONSTRAINT"), space1),
            terminated(tag_no_case("CONTAINS"), space1),
            terminated(tag_no_case("CONVERT"), space1),
            terminated(tag_no_case("CREATE"), space1),
            terminated(tag_no_case("CROSS"), space1),
            alt((
                terminated(tag_no_case("CURRENT_TIMESTAMP"), space1),
                terminated(tag_no_case("DATABASE"), space1),
                terminated(tag_no_case("DATABASES"), space1),
                terminated(tag_no_case("DAY"), space1),
                terminated(tag_no_case("DAY_HOUR"), space1),
                terminated(tag_no_case("DAY_MINUTE"), space1),
                terminated(tag_no_case("DAY_SECOND"), space1),
                terminated(tag_no_case("DEFAULT"), space1),
                terminated(tag_no_case("DEFINER"), space1),
                terminated(tag_no_case("DELAYED"), space1),
                terminated(tag_no_case("DELETE"), space1),
                terminated(tag_no_case("DESC"), space1),
                terminated(tag_no_case("DESCRIBE"), space1),
                terminated(tag_no_case("DETERMINISTIC"), space1),
                terminated(tag_no_case("DISTINCT"), space1),
                terminated(tag_no_case("DISTINCTROW"), space1),
                terminated(tag_no_case("DIV"), space1),
                terminated(tag_no_case("DO"), space1),
                terminated(tag_no_case("DROP"), space1),
                terminated(tag_no_case("DUMPFILE"), space1),
                alt((
                    terminated(tag_no_case("DUPLICATE"), space1),
                    terminated(tag_no_case("DYNAMIC"), space1),
                    terminated(tag_no_case("ELSE"), space1),
                    terminated(tag_no_case("ENCLOSED"), space1),
                    terminated(tag_no_case("END"), space1),
                    terminated(tag_no_case("ENGINE"), space1),
                    terminated(tag_no_case("ENGINES"), space1),
                    terminated(tag_no_case("ENGINE_TYPE"), space1),
                    terminated(tag_no_case("ESCAPE"), space1),
                    terminated(tag_no_case("ESCAPED"), space1),
                    terminated(tag_no_case("EVENTS"), space1),
                    terminated(tag_no_case("EXEC"), space1),
                    terminated(tag_no_case("EXECUTE"), space1),
                    terminated(tag_no_case("EXISTS"), space1),
                    terminated(tag_no_case("EXPLAIN"), space1),
                    terminated(tag_no_case("EXTENDED"), space1),
                    terminated(tag_no_case("FAST"), space1),
                    terminated(tag_no_case("FETCH"), space1),
                    terminated(tag_no_case("FIELDS"), space1),
                    alt((
                        terminated(tag_no_case("FILE"), space1),
                        terminated(tag_no_case("FIRST"), space1),
                        terminated(tag_no_case("FIXED"), space1),
                        terminated(tag_no_case("FLUSH"), space1),
                        terminated(tag_no_case("FOR"), space1),
                        terminated(tag_no_case("FORCE"), space1),
                        terminated(tag_no_case("FOREIGN"), space1),
                        terminated(tag_no_case("FULL"), space1),
                        terminated(tag_no_case("FULLTEXT"), space1),
                        terminated(tag_no_case("FUNCTION"), space1),
                        terminated(tag_no_case("GLOBAL"), space1),
                        terminated(tag_no_case("GRANT"), space1),
                        terminated(tag_no_case("GRANTS"), space1),
                        terminated(tag_no_case("GROUP_CONCAT"), space1),
                        terminated(tag_no_case("HEAP"), space1),
                        terminated(tag_no_case("HIGH_PRIORITY"), space1),
                        terminated(tag_no_case("HOSTS"), space1),
                        terminated(tag_no_case("HOUR"), space1),
                        terminated(tag_no_case("HOUR_MINUTE"), space1),
                        terminated(tag_no_case("HOUR_SECOND"), space1),
                        alt((
                            terminated(tag_no_case("IDENTIFIED"), space1),
                            terminated(tag_no_case("IF"), space1),
                            terminated(tag_no_case("IFNULL"), space1),
                            terminated(tag_no_case("IGNORE"), space1),
                            terminated(tag_no_case("IN"), space1),
                            terminated(tag_no_case("INDEX"), space1),
                            terminated(tag_no_case("INDEXES"), space1),
                            terminated(tag_no_case("INFILE"), space1),
                            terminated(tag_no_case("INSERT"), space1),
                            terminated(tag_no_case("INSERT_ID"), space1),
                            terminated(tag_no_case("INSERT_METHOD"), space1),
                            terminated(tag_no_case("INTERVAL"), space1),
                            terminated(tag_no_case("INTO"), space1),
                            terminated(tag_no_case("INVOKER"), space1),
                            terminated(tag_no_case("IS"), space1),
                            terminated(tag_no_case("ISOLATION"), space1),
                            terminated(tag_no_case("KEY"), space1),
                            terminated(tag_no_case("KEYS"), space1),
                            terminated(tag_no_case("KILL"), space1),
                            terminated(tag_no_case("LAST_INSERT_ID"), space1),
                            alt((
                                terminated(tag_no_case("LEADING"), space1),
                                terminated(tag_no_case("LEVEL"), space1),
                                terminated(tag_no_case("LIKE"), space1),
                                terminated(tag_no_case("LINEAR"), space1),
                                terminated(tag_no_case("LINES"), space1),
                                terminated(tag_no_case("LOAD"), space1),
                                terminated(tag_no_case("LOCAL"), space1),
                                terminated(tag_no_case("LOCK"), space1),
                                terminated(tag_no_case("LOCKS"), space1),
                                terminated(tag_no_case("LOGS"), space1),
                                terminated(tag_no_case("LOW_PRIORITY"), space1),
                                terminated(tag_no_case("MARIA"), space1),
                                terminated(tag_no_case("MASTER"), space1),
                                terminated(tag_no_case("MASTER_CONNECT_RETRY"), space1),
                                terminated(tag_no_case("MASTER_HOST"), space1),
                                terminated(tag_no_case("MASTER_LOG_FILE"), space1),
                                terminated(tag_no_case("MATCH"), space1),
                                terminated(tag_no_case("MAX_CONNECTIONS_PER_HOUR"), space1),
                                terminated(tag_no_case("MAX_QUERIES_PER_HOUR"), space1),
                                terminated(tag_no_case("MAX_ROWS"), space1),
                                alt((
                                    terminated(tag_no_case("MAX_UPDATES_PER_HOUR"), space1),
                                    terminated(tag_no_case("MAX_USER_CONNECTIONS"), space1),
                                    terminated(tag_no_case("MEDIUM"), space1),
                                    terminated(tag_no_case("MERGE"), space1),
                                    terminated(tag_no_case("MINUTE"), space1),
                                    terminated(tag_no_case("MINUTE_SECOND"), space1),
                                    terminated(tag_no_case("MIN_ROWS"), space1),
                                    terminated(tag_no_case("MODE"), space1),
                                    terminated(tag_no_case("MODIFY"), space1),
                                    terminated(tag_no_case("MONTH"), space1),
                                    terminated(tag_no_case("MRG_MYISAM"), space1),
                                    terminated(tag_no_case("MYISAM"), space1),
                                    terminated(tag_no_case("NAMES"), space1),
                                    terminated(tag_no_case("NATURAL"), space1),
                                    terminated(tag_no_case("NOT"), space1),
                                    terminated(tag_no_case("NOW()"), space1),
                                    terminated(tag_no_case("NULL"), space1),
                                    terminated(tag_no_case("OFFSET"), space1),
                                    terminated(tag_no_case("ON DELETE"), space1),
                                    terminated(tag_no_case("ON UPDATE"), space1),
                                    alt((
                                        terminated(tag_no_case("ON"), space1),
                                        terminated(tag_no_case("ONLY"), space1),
                                        terminated(tag_no_case("OPEN"), space1),
                                        terminated(tag_no_case("OPTIMIZE"), space1),
                                        terminated(tag_no_case("OPTION"), space1),
                                        terminated(tag_no_case("OPTIONALLY"), space1),
                                        terminated(tag_no_case("OUTFILE"), space1),
                                        terminated(tag_no_case("PACK_KEYS"), space1),
                                        terminated(tag_no_case("PAGE"), space1),
                                        terminated(tag_no_case("PARTIAL"), space1),
                                        terminated(tag_no_case("PARTITION"), space1),
                                        terminated(tag_no_case("PARTITIONS"), space1),
                                        terminated(tag_no_case("PASSWORD"), space1),
                                        terminated(tag_no_case("PRIMARY"), space1),
                                        terminated(tag_no_case("PRIVILEGES"), space1),
                                        terminated(tag_no_case("PROCEDURE"), space1),
                                        terminated(tag_no_case("PROCESS"), space1),
                                        terminated(tag_no_case("PROCESSLIST"), space1),
                                        terminated(tag_no_case("PURGE"), space1),
                                        terminated(tag_no_case("QUICK"), space1),
                                        alt((
                                            terminated(tag_no_case("RAID0"), space1),
                                            terminated(tag_no_case("RAID_CHUNKS"), space1),
                                            terminated(tag_no_case("RAID_CHUNKSIZE"), space1),
                                            terminated(tag_no_case("RAID_TYPE"), space1),
                                            terminated(tag_no_case("RANGE"), space1),
                                            terminated(tag_no_case("READ"), space1),
                                            terminated(tag_no_case("READ_ONLY"), space1),
                                            terminated(tag_no_case("READ_WRITE"), space1),
                                            terminated(tag_no_case("REFERENCES"), space1),
                                            terminated(tag_no_case("REGEXP"), space1),
                                            terminated(tag_no_case("RELOAD"), space1),
                                            terminated(tag_no_case("RENAME"), space1),
                                            terminated(tag_no_case("REPAIR"), space1),
                                            terminated(tag_no_case("REPEATABLE"), space1),
                                            terminated(tag_no_case("REPLACE"), space1),
                                            terminated(tag_no_case("REPLICATION"), space1),
                                            terminated(tag_no_case("RESET"), space1),
                                            terminated(tag_no_case("RESTORE"), space1),
                                            terminated(tag_no_case("RESTRICT"), space1),
                                            terminated(tag_no_case("RETURN"), space1),
                                            alt((
                                                terminated(tag_no_case("RETURNS"), space1),
                                                terminated(tag_no_case("REVOKE"), space1),
                                                terminated(tag_no_case("RLIKE"), space1),
                                                terminated(tag_no_case("ROLLBACK"), space1),
                                                terminated(tag_no_case("ROW"), space1),
                                                terminated(tag_no_case("ROWS"), space1),
                                                terminated(tag_no_case("ROW_FORMAT"), space1),
                                                terminated(tag_no_case("SECOND"), space1),
                                                terminated(tag_no_case("SECURITY"), space1),
                                                terminated(tag_no_case("SEPARATOR"), space1),
                                                terminated(tag_no_case("SERIALIZABLE"), space1),
                                                terminated(tag_no_case("SESSION"), space1),
                                                terminated(tag_no_case("SHARE"), space1),
                                                terminated(tag_no_case("SHOW"), space1),
                                                terminated(tag_no_case("SHUTDOWN"), space1),
                                                terminated(tag_no_case("SLAVE"), space1),
                                                terminated(tag_no_case("SONAME"), space1),
                                                terminated(tag_no_case("SOUNDS"), space1),
                                                terminated(tag_no_case("SQL"), space1),
                                                terminated(tag_no_case("SQL_AUTO_IS_NULL"), space1),
                                                alt((
                                                    terminated(
                                                        tag_no_case("SQL_BIG_RESULT"),
                                                        space1,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_BIG_SELECTS"),
                                                        space1,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_BIG_TABLES"),
                                                        space1,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_BUFFER_RESULT"),
                                                        space1,
                                                    ),
                                                    terminated(tag_no_case("SQL_CACHE"), space1),
                                                    terminated(
                                                        tag_no_case("SQL_CALC_FOUND_ROWS"),
                                                        space1,
                                                    ),
                                                    terminated(tag_no_case("SQL_LOG_BIN"), space1),
                                                    terminated(tag_no_case("SQL_LOG_OFF"), space1),
                                                    terminated(
                                                        tag_no_case("SQL_LOG_UPDATE"),
                                                        space1,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_LOW_PRIORITY_UPDATES"),
                                                        space1,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_MAX_JOIN_SIZE"),
                                                        space1,
                                                    ),
                                                    terminated(tag_no_case("SQL_NO_CACHE"), space1),
                                                    terminated(
                                                        tag_no_case("SQL_QUOTE_SHOW_CREATE"),
                                                        space1,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_SAFE_UPDATES"),
                                                        space1,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_SELECT_LIMIT"),
                                                        space1,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_SLAVE_SKIP_COUNTER"),
                                                        space1,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_SMALL_RESULT"),
                                                        space1,
                                                    ),
                                                    terminated(tag_no_case("SQL_WARNINGS"), space1),
                                                    terminated(tag_no_case("START"), space1),
                                                    terminated(tag_no_case("STARTING"), space1),
                                                    alt((
                                                        terminated(tag_no_case("STATUS"), space1),
                                                        terminated(tag_no_case("STOP"), space1),
                                                        terminated(tag_no_case("STORAGE"), space1),
                                                        terminated(
                                                            tag_no_case("STRAIGHT_JOIN"),
                                                            space1,
                                                        ),
                                                        terminated(tag_no_case("STRING"), space1),
                                                        terminated(tag_no_case("STRIPED"), space1),
                                                        terminated(tag_no_case("SUPER"), space1),
                                                        terminated(tag_no_case("TABLE"), space1),
                                                        terminated(tag_no_case("TABLES"), space1),
                                                        terminated(
                                                            tag_no_case("TEMPORARY"),
                                                            space1,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TERMINATED"),
                                                            space1,
                                                        ),
                                                        terminated(tag_no_case("THEN"), space1),
                                                        terminated(tag_no_case("TO"), space1),
                                                        terminated(tag_no_case("TRAILING"), space1),
                                                        terminated(
                                                            tag_no_case("TRANSACTIONAL"),
                                                            space1,
                                                        ),
                                                        terminated(tag_no_case("TRUE"), space1),
                                                        terminated(tag_no_case("TRUNCATE"), space1),
                                                        terminated(tag_no_case("TYPE"), space1),
                                                        terminated(tag_no_case("TYPES"), space1),
                                                        terminated(
                                                            tag_no_case("UNCOMMITTED"),
                                                            space1,
                                                        ),
                                                        alt((
                                                            terminated(
                                                                tag_no_case("UNIQUE"),
                                                                space1,
                                                            ),
                                                            terminated(
                                                                tag_no_case("UNLOCK"),
                                                                space1,
                                                            ),
                                                            terminated(
                                                                tag_no_case("UNSIGNED"),
                                                                space1,
                                                            ),
                                                            terminated(
                                                                tag_no_case("USAGE"),
                                                                space1,
                                                            ),
                                                            terminated(tag_no_case("USE"), space1),
                                                            terminated(
                                                                tag_no_case("USING"),
                                                                space1,
                                                            ),
                                                            terminated(
                                                                tag_no_case("VARIABLES"),
                                                                space1,
                                                            ),
                                                            terminated(tag_no_case("VIEW"), space1),
                                                            terminated(tag_no_case("WHEN"), space1),
                                                            terminated(tag_no_case("WITH"), space1),
                                                            terminated(tag_no_case("WORK"), space1),
                                                            terminated(
                                                                tag_no_case("WRITE"),
                                                                space1,
                                                            ),
                                                            terminated(
                                                                tag_no_case("YEAR_MONTH"),
                                                                space1,
                                                            ),
                                                        )),
                                                    )),
                                                )),
                                            )),
                                        )),
                                    )),
                                )),
                            )),
                        )),
                    )),
                )),
            )),
        )),
    ))(input)
    .map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::Reserved,
                value: token,
                key: None,
            },
        )
    })
}

fn get_word_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    take_while1(|item: char| {
        item.is_alphanumeric()
            || item.is_mark()
            || item.is_punctuation_connector()
            || item.is_other_control()
    })(input)
    .map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::Word,
                value: token,
                key: None,
            },
        )
    })
}

fn get_operator_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        tag("!="),
        tag("<>"),
        tag("=="),
        tag("<="),
        tag(">="),
        tag("!<"),
        tag("!>"),
        tag("||"),
        tag("::"),
        tag("->>"),
        tag("->"),
        tag("~~*"),
        tag("~~"),
        tag("!~~*"),
        tag("!~~"),
        tag("~*"),
        tag("!~*"),
        tag("!~"),
        tag(":="),
        tag("."),
    ))(input)
    .map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::Operator,
                value: token,
                key: None,
            },
        )
    })
}
