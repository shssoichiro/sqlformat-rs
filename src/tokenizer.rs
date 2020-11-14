use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take, take_until, take_while1};
use nom::character::complete::{anychar, char, digit0, digit1, not_line_ending, space1};
use nom::combinator::{map, map_res, opt, peek, recognize};
use nom::error::ParseError;
use nom::error::{Error, ErrorKind};
use nom::multi::many0;
use nom::sequence::{terminated, tuple};
use nom::{Err, IResult};
use std::borrow::Cow;
use unicode_categories::UnicodeCategories;

pub(crate) fn tokenize<'a>(mut input: &'a str) -> Vec<Token<'a>> {
    let mut tokens = Vec::new();
    let mut token = None;

    // Keep processing the string until it is empty
    while let Ok(result) = get_next_token(input, token.as_ref()) {
        token = Some(result.1.clone());
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

fn get_next_token<'a>(
    input: &'a str,
    previous_token: Option<&Token<'a>>,
) -> IResult<&'a str, Token<'a>> {
    get_whitespace_token(input)
        .or_else(|_| get_comment_token(input))
        .or_else(|_| get_string_token(input))
        .or_else(|_| get_open_paren_token(input))
        .or_else(|_| get_close_paren_token(input))
        .or_else(|_| get_placeholder_token(input))
        .or_else(|_| get_number_token(input))
        .or_else(|_| get_reserved_word_token(input, previous_token))
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
    recognize(tuple((alt((tag("#"), tag("--"))), not_line_ending)))(input).map(|(input, token)| {
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
    recognize(tuple((
        tag("/*"),
        alt((take_until("*/"), recognize(many0(anychar)))),
        opt(take(2usize)),
    )))(input)
    .map(|(input, token)| {
        (
            input,
            Token {
                kind: TokenKind::BlockComment,
                value: token,
                key: None,
            },
        )
    })
}

pub fn take_till_escaping<'a, Error: ParseError<&'a str>>(
    desired: char,
    escapes: &'static [char],
) -> impl Fn(&'a str) -> IResult<&'a str, &'a str, Error> {
    move |input: &str| {
        let mut chars = input.chars().enumerate().peekable();
        let mut last = None;
        loop {
            let item = chars.next();
            let next = chars.peek().map(|item| item.1);
            match item {
                Some(item) => {
                    if item.1 == desired
                        && !last.map(|item| escapes.contains(&item)).unwrap_or(false)
                        && !(escapes.contains(&item.1) && Some(desired) == next)
                    {
                        return Ok((&input[item.0..], &input[..item.0]));
                    }

                    last = Some(item.1);
                    continue;
                }
                None => {
                    return Ok(("", input));
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
fn get_string_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        recognize(tuple((
            char('`'),
            take_till_escaping('`', &['`']),
            take(1usize),
        ))),
        recognize(tuple((
            char('['),
            take_till_escaping(']', &[']']),
            take(1usize),
        ))),
        recognize(tuple((
            char('"'),
            take_till_escaping('"', &['"', '\\']),
            take(1usize),
        ))),
        recognize(tuple((
            char('\''),
            take_till_escaping('\'', &['\'', '\\']),
            take(1usize),
        ))),
        recognize(tuple((
            tag("N'"),
            take_till_escaping('\'', &['\'', '\\']),
            take(1usize),
        ))),
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
    alt((tag("("), terminated(tag_no_case("CASE"), end_of_word)))(input).map(|(input, token)| {
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
    alt((tag(")"), terminated(tag_no_case("END"), end_of_word)))(input).map(|(input, token)| {
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
        take_while1(|item: char| {
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

fn get_reserved_word_token<'a>(
    input: &'a str,
    previous_token: Option<&Token<'a>>,
) -> IResult<&'a str, Token<'a>> {
    // A reserved word cannot be preceded by a "."
    // this makes it so in "my_table.from", "from" is not considered a reserved word
    if let Some(token) = previous_token {
        if token.value == "." {
            return Err(Err::Error(Error::new(input, ErrorKind::IsNot)));
        }
    }

    alt((
        get_top_level_reserved_token,
        get_newline_reserved_token,
        get_top_level_reserved_token_no_indent,
        get_plain_reserved_token,
    ))(input)
}

fn get_top_level_reserved_token<'a>(input: &'a str) -> IResult<&'a str, Token<'a>> {
    alt((
        terminated(tag_no_case("ADD"), end_of_word),
        terminated(tag_no_case("AFTER"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("ALTER"),
                many_whitespace_to_1,
                tag_no_case("COLUMN"),
            ))),
            end_of_word,
        ),
        terminated(
            recognize(tuple((
                tag_no_case("ALTER"),
                many_whitespace_to_1,
                tag_no_case("TABLE"),
            ))),
            end_of_word,
        ),
        terminated(
            recognize(tuple((
                tag_no_case("DELETE"),
                many_whitespace_to_1,
                tag_no_case("FROM"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("EXCEPT"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("FETCH"),
                many_whitespace_to_1,
                tag_no_case("FIRST"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("FROM"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("GROUP"),
                many_whitespace_to_1,
                tag_no_case("BY"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("GO"), end_of_word),
        terminated(tag_no_case("HAVING"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("INSERT"),
                many_whitespace_to_1,
                tag_no_case("INTO"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("INSERT"), end_of_word),
        terminated(tag_no_case("LIMIT"), end_of_word),
        terminated(tag_no_case("MODIFY"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("ORDER"),
                many_whitespace_to_1,
                tag_no_case("BY"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("SELECT"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("SET"),
                many_whitespace_to_1,
                tag_no_case("CURRENT"),
                many_whitespace_to_1,
                tag_no_case("SCHEMA"),
            ))),
            end_of_word,
        ),
        terminated(
            recognize(tuple((
                tag_no_case("SET"),
                many_whitespace_to_1,
                tag_no_case("SCHEMA"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("SET"), end_of_word),
        alt((
            terminated(tag_no_case("UPDATE"), end_of_word),
            terminated(tag_no_case("VALUES"), end_of_word),
            terminated(tag_no_case("WHERE"), end_of_word),
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
        terminated(tag_no_case("AND"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("CROSS"),
                many_whitespace_to_1,
                tag_no_case("APPLY"),
            ))),
            end_of_word,
        ),
        terminated(
            recognize(tuple((
                tag_no_case("CROSS"),
                many_whitespace_to_1,
                tag_no_case("JOIN"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("ELSE"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("INNER"),
                many_whitespace_to_1,
                tag_no_case("JOIN"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("JOIN"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("LEFT"),
                many_whitespace_to_1,
                tag_no_case("JOIN"),
            ))),
            end_of_word,
        ),
        terminated(
            recognize(tuple((
                tag_no_case("LEFT"),
                many_whitespace_to_1,
                tag_no_case("OUTER"),
                many_whitespace_to_1,
                tag_no_case("JOIN"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("OR"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("OUTER"),
                many_whitespace_to_1,
                tag_no_case("APPLY"),
            ))),
            end_of_word,
        ),
        terminated(
            recognize(tuple((
                tag_no_case("OUTER"),
                many_whitespace_to_1,
                tag_no_case("JOIN"),
            ))),
            end_of_word,
        ),
        terminated(
            recognize(tuple((
                tag_no_case("RIGHT"),
                many_whitespace_to_1,
                tag_no_case("JOIN"),
            ))),
            end_of_word,
        ),
        terminated(
            recognize(tuple((
                tag_no_case("RIGHT"),
                space1,
                tag_no_case("OUTER"),
                space1,
                tag_no_case("JOIN"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("WHEN"), end_of_word),
        terminated(tag_no_case("XOR"), end_of_word),
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
        terminated(tag_no_case("INTERSECT"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("INTERSECT"),
                many_whitespace_to_1,
                tag_no_case("ALL"),
            ))),
            end_of_word,
        ),
        terminated(tag_no_case("MINUS"), end_of_word),
        terminated(tag_no_case("UNION"), end_of_word),
        terminated(
            recognize(tuple((
                tag_no_case("UNION"),
                many_whitespace_to_1,
                tag_no_case("ALL"),
            ))),
            end_of_word,
        ),
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
        terminated(tag_no_case("ACCESSIBLE"), end_of_word),
        terminated(tag_no_case("ACTION"), end_of_word),
        terminated(tag_no_case("AGAINST"), end_of_word),
        terminated(tag_no_case("AGGREGATE"), end_of_word),
        terminated(tag_no_case("ALGORITHM"), end_of_word),
        terminated(tag_no_case("ALL"), end_of_word),
        terminated(tag_no_case("ALTER"), end_of_word),
        terminated(tag_no_case("ANALYSE"), end_of_word),
        terminated(tag_no_case("ANALYZE"), end_of_word),
        terminated(tag_no_case("AS"), end_of_word),
        terminated(tag_no_case("ASC"), end_of_word),
        terminated(tag_no_case("AUTOCOMMIT"), end_of_word),
        terminated(tag_no_case("AUTO_INCREMENT"), end_of_word),
        terminated(tag_no_case("BACKUP"), end_of_word),
        terminated(tag_no_case("BEGIN"), end_of_word),
        terminated(tag_no_case("BETWEEN"), end_of_word),
        terminated(tag_no_case("BINLOG"), end_of_word),
        terminated(tag_no_case("BOTH"), end_of_word),
        terminated(tag_no_case("CASCADE"), end_of_word),
        terminated(tag_no_case("CASE"), end_of_word),
        alt((
            terminated(tag_no_case("CHANGE"), end_of_word),
            terminated(tag_no_case("CHANGED"), end_of_word),
            terminated(tag_no_case("CHARACTER SET"), end_of_word),
            terminated(tag_no_case("CHARSET"), end_of_word),
            terminated(tag_no_case("CHECK"), end_of_word),
            terminated(tag_no_case("CHECKSUM"), end_of_word),
            terminated(tag_no_case("COLLATE"), end_of_word),
            terminated(tag_no_case("COLLATION"), end_of_word),
            terminated(tag_no_case("COLUMN"), end_of_word),
            terminated(tag_no_case("COLUMNS"), end_of_word),
            terminated(tag_no_case("COMMENT"), end_of_word),
            terminated(tag_no_case("COMMIT"), end_of_word),
            terminated(tag_no_case("COMMITTED"), end_of_word),
            terminated(tag_no_case("COMPRESSED"), end_of_word),
            terminated(tag_no_case("CONCURRENT"), end_of_word),
            terminated(tag_no_case("CONSTRAINT"), end_of_word),
            terminated(tag_no_case("CONTAINS"), end_of_word),
            terminated(tag_no_case("CONVERT"), end_of_word),
            terminated(tag_no_case("CREATE"), end_of_word),
            terminated(tag_no_case("CROSS"), end_of_word),
            alt((
                terminated(tag_no_case("CURRENT_TIMESTAMP"), end_of_word),
                terminated(tag_no_case("DATABASE"), end_of_word),
                terminated(tag_no_case("DATABASES"), end_of_word),
                terminated(tag_no_case("DAY"), end_of_word),
                terminated(tag_no_case("DAY_HOUR"), end_of_word),
                terminated(tag_no_case("DAY_MINUTE"), end_of_word),
                terminated(tag_no_case("DAY_SECOND"), end_of_word),
                terminated(tag_no_case("DEFAULT"), end_of_word),
                terminated(tag_no_case("DEFINER"), end_of_word),
                terminated(tag_no_case("DELAYED"), end_of_word),
                terminated(tag_no_case("DELETE"), end_of_word),
                terminated(tag_no_case("DESC"), end_of_word),
                terminated(tag_no_case("DESCRIBE"), end_of_word),
                terminated(tag_no_case("DETERMINISTIC"), end_of_word),
                terminated(tag_no_case("DISTINCT"), end_of_word),
                terminated(tag_no_case("DISTINCTROW"), end_of_word),
                terminated(tag_no_case("DIV"), end_of_word),
                terminated(tag_no_case("DO"), end_of_word),
                terminated(tag_no_case("DROP"), end_of_word),
                terminated(tag_no_case("DUMPFILE"), end_of_word),
                alt((
                    terminated(tag_no_case("DUPLICATE"), end_of_word),
                    terminated(tag_no_case("DYNAMIC"), end_of_word),
                    terminated(tag_no_case("ELSE"), end_of_word),
                    terminated(tag_no_case("ENCLOSED"), end_of_word),
                    terminated(tag_no_case("END"), end_of_word),
                    terminated(tag_no_case("ENGINE"), end_of_word),
                    terminated(tag_no_case("ENGINES"), end_of_word),
                    terminated(tag_no_case("ENGINE_TYPE"), end_of_word),
                    terminated(tag_no_case("ESCAPE"), end_of_word),
                    terminated(tag_no_case("ESCAPED"), end_of_word),
                    terminated(tag_no_case("EVENTS"), end_of_word),
                    terminated(tag_no_case("EXEC"), end_of_word),
                    terminated(tag_no_case("EXECUTE"), end_of_word),
                    terminated(tag_no_case("EXISTS"), end_of_word),
                    terminated(tag_no_case("EXPLAIN"), end_of_word),
                    terminated(tag_no_case("EXTENDED"), end_of_word),
                    terminated(tag_no_case("FAST"), end_of_word),
                    terminated(tag_no_case("FETCH"), end_of_word),
                    terminated(tag_no_case("FIELDS"), end_of_word),
                    alt((
                        terminated(tag_no_case("FILE"), end_of_word),
                        terminated(tag_no_case("FIRST"), end_of_word),
                        terminated(tag_no_case("FIXED"), end_of_word),
                        terminated(tag_no_case("FLUSH"), end_of_word),
                        terminated(tag_no_case("FOR"), end_of_word),
                        terminated(tag_no_case("FORCE"), end_of_word),
                        terminated(tag_no_case("FOREIGN"), end_of_word),
                        terminated(tag_no_case("FULL"), end_of_word),
                        terminated(tag_no_case("FULLTEXT"), end_of_word),
                        terminated(tag_no_case("FUNCTION"), end_of_word),
                        terminated(tag_no_case("GLOBAL"), end_of_word),
                        terminated(tag_no_case("GRANT"), end_of_word),
                        terminated(tag_no_case("GRANTS"), end_of_word),
                        terminated(tag_no_case("GROUP_CONCAT"), end_of_word),
                        terminated(tag_no_case("HEAP"), end_of_word),
                        terminated(tag_no_case("HIGH_PRIORITY"), end_of_word),
                        terminated(tag_no_case("HOSTS"), end_of_word),
                        terminated(tag_no_case("HOUR"), end_of_word),
                        terminated(tag_no_case("HOUR_MINUTE"), end_of_word),
                        terminated(tag_no_case("HOUR_SECOND"), end_of_word),
                        alt((
                            terminated(tag_no_case("IDENTIFIED"), end_of_word),
                            terminated(tag_no_case("IF"), end_of_word),
                            terminated(tag_no_case("IFNULL"), end_of_word),
                            terminated(tag_no_case("IGNORE"), end_of_word),
                            terminated(tag_no_case("IN"), end_of_word),
                            terminated(tag_no_case("INDEX"), end_of_word),
                            terminated(tag_no_case("INDEXES"), end_of_word),
                            terminated(tag_no_case("INFILE"), end_of_word),
                            terminated(tag_no_case("INSERT"), end_of_word),
                            terminated(tag_no_case("INSERT_ID"), end_of_word),
                            terminated(tag_no_case("INSERT_METHOD"), end_of_word),
                            terminated(tag_no_case("INTERVAL"), end_of_word),
                            terminated(tag_no_case("INTO"), end_of_word),
                            terminated(tag_no_case("INVOKER"), end_of_word),
                            terminated(tag_no_case("IS"), end_of_word),
                            terminated(tag_no_case("ISOLATION"), end_of_word),
                            terminated(tag_no_case("KEY"), end_of_word),
                            terminated(tag_no_case("KEYS"), end_of_word),
                            terminated(tag_no_case("KILL"), end_of_word),
                            terminated(tag_no_case("LAST_INSERT_ID"), end_of_word),
                            alt((
                                terminated(tag_no_case("LEADING"), end_of_word),
                                terminated(tag_no_case("LEVEL"), end_of_word),
                                terminated(tag_no_case("LIKE"), end_of_word),
                                terminated(tag_no_case("LINEAR"), end_of_word),
                                terminated(tag_no_case("LINES"), end_of_word),
                                terminated(tag_no_case("LOAD"), end_of_word),
                                terminated(tag_no_case("LOCAL"), end_of_word),
                                terminated(tag_no_case("LOCK"), end_of_word),
                                terminated(tag_no_case("LOCKS"), end_of_word),
                                terminated(tag_no_case("LOGS"), end_of_word),
                                terminated(tag_no_case("LOW_PRIORITY"), end_of_word),
                                terminated(tag_no_case("MARIA"), end_of_word),
                                terminated(tag_no_case("MASTER"), end_of_word),
                                terminated(tag_no_case("MASTER_CONNECT_RETRY"), end_of_word),
                                terminated(tag_no_case("MASTER_HOST"), end_of_word),
                                terminated(tag_no_case("MASTER_LOG_FILE"), end_of_word),
                                terminated(tag_no_case("MATCH"), end_of_word),
                                terminated(tag_no_case("MAX_CONNECTIONS_PER_HOUR"), end_of_word),
                                terminated(tag_no_case("MAX_QUERIES_PER_HOUR"), end_of_word),
                                terminated(tag_no_case("MAX_ROWS"), end_of_word),
                                alt((
                                    terminated(tag_no_case("MAX_UPDATES_PER_HOUR"), end_of_word),
                                    terminated(tag_no_case("MAX_USER_CONNECTIONS"), end_of_word),
                                    terminated(tag_no_case("MEDIUM"), end_of_word),
                                    terminated(tag_no_case("MERGE"), end_of_word),
                                    terminated(tag_no_case("MINUTE"), end_of_word),
                                    terminated(tag_no_case("MINUTE_SECOND"), end_of_word),
                                    terminated(tag_no_case("MIN_ROWS"), end_of_word),
                                    terminated(tag_no_case("MODE"), end_of_word),
                                    terminated(tag_no_case("MODIFY"), end_of_word),
                                    terminated(tag_no_case("MONTH"), end_of_word),
                                    terminated(tag_no_case("MRG_MYISAM"), end_of_word),
                                    terminated(tag_no_case("MYISAM"), end_of_word),
                                    terminated(tag_no_case("NAMES"), end_of_word),
                                    terminated(tag_no_case("NATURAL"), end_of_word),
                                    terminated(tag_no_case("NOT"), end_of_word),
                                    terminated(tag_no_case("NOW()"), end_of_word),
                                    terminated(tag_no_case("NULL"), end_of_word),
                                    terminated(tag_no_case("OFFSET"), end_of_word),
                                    terminated(tag_no_case("ON DELETE"), end_of_word),
                                    terminated(tag_no_case("ON UPDATE"), end_of_word),
                                    alt((
                                        terminated(tag_no_case("ON"), end_of_word),
                                        terminated(tag_no_case("ONLY"), end_of_word),
                                        terminated(tag_no_case("OPEN"), end_of_word),
                                        terminated(tag_no_case("OPTIMIZE"), end_of_word),
                                        terminated(tag_no_case("OPTION"), end_of_word),
                                        terminated(tag_no_case("OPTIONALLY"), end_of_word),
                                        terminated(tag_no_case("OUTFILE"), end_of_word),
                                        terminated(tag_no_case("PACK_KEYS"), end_of_word),
                                        terminated(tag_no_case("PAGE"), end_of_word),
                                        terminated(tag_no_case("PARTIAL"), end_of_word),
                                        terminated(tag_no_case("PARTITION"), end_of_word),
                                        terminated(tag_no_case("PARTITIONS"), end_of_word),
                                        terminated(tag_no_case("PASSWORD"), end_of_word),
                                        terminated(tag_no_case("PRIMARY"), end_of_word),
                                        terminated(tag_no_case("PRIVILEGES"), end_of_word),
                                        terminated(tag_no_case("PROCEDURE"), end_of_word),
                                        terminated(tag_no_case("PROCESS"), end_of_word),
                                        terminated(tag_no_case("PROCESSLIST"), end_of_word),
                                        terminated(tag_no_case("PURGE"), end_of_word),
                                        terminated(tag_no_case("QUICK"), end_of_word),
                                        alt((
                                            terminated(tag_no_case("RAID0"), end_of_word),
                                            terminated(tag_no_case("RAID_CHUNKS"), end_of_word),
                                            terminated(tag_no_case("RAID_CHUNKSIZE"), end_of_word),
                                            terminated(tag_no_case("RAID_TYPE"), end_of_word),
                                            terminated(tag_no_case("RANGE"), end_of_word),
                                            terminated(tag_no_case("READ"), end_of_word),
                                            terminated(tag_no_case("READ_ONLY"), end_of_word),
                                            terminated(tag_no_case("READ_WRITE"), end_of_word),
                                            terminated(tag_no_case("REFERENCES"), end_of_word),
                                            terminated(tag_no_case("REGEXP"), end_of_word),
                                            terminated(tag_no_case("RELOAD"), end_of_word),
                                            terminated(tag_no_case("RENAME"), end_of_word),
                                            terminated(tag_no_case("REPAIR"), end_of_word),
                                            terminated(tag_no_case("REPEATABLE"), end_of_word),
                                            terminated(tag_no_case("REPLACE"), end_of_word),
                                            terminated(tag_no_case("REPLICATION"), end_of_word),
                                            terminated(tag_no_case("RESET"), end_of_word),
                                            terminated(tag_no_case("RESTORE"), end_of_word),
                                            terminated(tag_no_case("RESTRICT"), end_of_word),
                                            terminated(tag_no_case("RETURN"), end_of_word),
                                            alt((
                                                terminated(tag_no_case("RETURNS"), end_of_word),
                                                terminated(tag_no_case("REVOKE"), end_of_word),
                                                terminated(tag_no_case("RLIKE"), end_of_word),
                                                terminated(tag_no_case("ROLLBACK"), end_of_word),
                                                terminated(tag_no_case("ROW"), end_of_word),
                                                terminated(tag_no_case("ROWS"), end_of_word),
                                                terminated(tag_no_case("ROW_FORMAT"), end_of_word),
                                                terminated(tag_no_case("SECOND"), end_of_word),
                                                terminated(tag_no_case("SECURITY"), end_of_word),
                                                terminated(tag_no_case("SEPARATOR"), end_of_word),
                                                terminated(
                                                    tag_no_case("SERIALIZABLE"),
                                                    end_of_word,
                                                ),
                                                terminated(tag_no_case("SESSION"), end_of_word),
                                                terminated(tag_no_case("SHARE"), end_of_word),
                                                terminated(tag_no_case("SHOW"), end_of_word),
                                                terminated(tag_no_case("SHUTDOWN"), end_of_word),
                                                terminated(tag_no_case("SLAVE"), end_of_word),
                                                terminated(tag_no_case("SONAME"), end_of_word),
                                                terminated(tag_no_case("SOUNDS"), end_of_word),
                                                terminated(tag_no_case("SQL"), end_of_word),
                                                terminated(
                                                    tag_no_case("SQL_AUTO_IS_NULL"),
                                                    end_of_word,
                                                ),
                                                alt((
                                                    terminated(
                                                        tag_no_case("SQL_BIG_RESULT"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_BIG_SELECTS"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_BIG_TABLES"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_BUFFER_RESULT"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_CACHE"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_CALC_FOUND_ROWS"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_LOG_BIN"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_LOG_OFF"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_LOG_UPDATE"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_LOW_PRIORITY_UPDATES"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_MAX_JOIN_SIZE"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_NO_CACHE"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_QUOTE_SHOW_CREATE"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_SAFE_UPDATES"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_SELECT_LIMIT"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_SLAVE_SKIP_COUNTER"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_SMALL_RESULT"),
                                                        end_of_word,
                                                    ),
                                                    terminated(
                                                        tag_no_case("SQL_WARNINGS"),
                                                        end_of_word,
                                                    ),
                                                    terminated(tag_no_case("START"), end_of_word),
                                                    terminated(
                                                        tag_no_case("STARTING"),
                                                        end_of_word,
                                                    ),
                                                    alt((
                                                        terminated(
                                                            tag_no_case("STATUS"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("STOP"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("STORAGE"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("STRAIGHT_JOIN"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("STRING"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("STRIPED"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("SUPER"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TABLE"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TABLES"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TEMPORARY"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TERMINATED"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("THEN"),
                                                            end_of_word,
                                                        ),
                                                        terminated(tag_no_case("TO"), end_of_word),
                                                        terminated(
                                                            tag_no_case("TRAILING"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TRANSACTIONAL"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TRUE"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TRUNCATE"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TYPE"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("TYPES"),
                                                            end_of_word,
                                                        ),
                                                        terminated(
                                                            tag_no_case("UNCOMMITTED"),
                                                            end_of_word,
                                                        ),
                                                        alt((
                                                            terminated(
                                                                tag_no_case("UNIQUE"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("UNLOCK"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("UNSIGNED"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("USAGE"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("USE"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("USING"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("VARIABLES"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("VIEW"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("WHEN"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("WITH"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("WORK"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("WRITE"),
                                                                end_of_word,
                                                            ),
                                                            terminated(
                                                                tag_no_case("YEAR_MONTH"),
                                                                end_of_word,
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
    take_while1(is_word_character)(input).map(|(input, token)| {
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
        recognize(map_res(take(1usize), |token| {
            if token == "\n" || token == "\r\n" {
                Err(())
            } else {
                Ok(token)
            }
        })),
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

fn end_of_word<'a>(input: &'a str) -> IResult<&'a str, &'a str> {
    peek(take_while1(|c: char| !is_word_character(c)))(input)
}

fn is_word_character(item: char) -> bool {
    item.is_alphanumeric() || item.is_mark() || item.is_punctuation_connector()
}

fn many_whitespace_to_1<'a>(input: &'a str) -> IResult<&'a str, &'a str> {
    map(take_while1(|c: char| c.is_whitespace()), |_| " ")(input)
}
