use crate::keywords::*;
use lazy_static::lazy_static;
use regex::Regex;

pub(crate) fn tokenize<'a>(mut input: &'a str) -> Vec<Token<'a>> {
    let mut tokens = Vec::new();
    let mut token = None;

    // Keep processing the string until it is empty
    while !input.is_empty() {
        // Get the next token and the token type
        token = Some(get_next_token(input, token.as_ref()));
        // Advance the string
        input = &input[token.as_ref().unwrap().value.len()..];

        tokens.push(token.clone().unwrap());
    }
    tokens
}

#[derive(Debug, Clone)]
pub(crate) struct Token<'a> {
    pub kind: TokenKind,
    pub value: &'a str,
    // Only used for placeholder tokens
    pub key: Option<String>,
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

fn get_next_token<'a>(input: &'a str, previous_token: Option<&Token<'a>>) -> Token<'a> {
    get_whitespace_token(input)
        .or_else(|| get_comment_token(input))
        .or_else(|| get_string_token(input))
        .or_else(|| get_open_paren_token(input))
        .or_else(|| get_close_paren_token(input))
        .or_else(|| get_placeholder_token(input))
        .or_else(|| get_number_token(input))
        .or_else(|| get_reserved_word_token(input, previous_token))
        .or_else(|| get_word_token(input))
        .or_else(|| get_operator_token(input))
        .expect("get_next_token received empty input")
}

fn get_whitespace_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::Whitespace, &WHITESPACE_REGEX)
}

fn get_comment_token(input: &str) -> Option<Token<'_>> {
    get_line_comment_token(input).or_else(|| get_block_comment_token(input))
}

fn get_line_comment_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::LineComment, &LINE_COMMENT_REGEX)
}

fn get_block_comment_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::BlockComment, &BLOCK_COMMENT_REGEX)
}

fn get_string_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::String, &STRING_REGEX)
}

fn get_open_paren_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::OpenParen, &OPEN_PAREN_REGEX)
}

fn get_close_paren_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::CloseParen, &CLOSE_PAREN_REGEX)
}

fn get_placeholder_token(input: &str) -> Option<Token<'_>> {
    get_ident_named_placeholder_token(input)
        .or_else(|| get_string_named_placeholder_token(input))
        .or_else(|| get_indexed_placeholder_token(input))
}

fn get_ident_named_placeholder_token(input: &str) -> Option<Token<'_>> {
    get_placeholder_token_with_key(input, &IDENT_NAMED_PLACEHOLDER_REGEX, |v| {
        v[1..].to_string()
    })
}

fn get_string_named_placeholder_token(input: &str) -> Option<Token<'_>> {
    get_placeholder_token_with_key(input, &STRING_NAMED_PLACEHOLDER_REGEX, |v| {
        get_escaped_placeholder_key(&v[2..v.len() - 1], &v[v.len() - 1..])
    })
}

fn get_indexed_placeholder_token(input: &str) -> Option<Token<'_>> {
    get_placeholder_token_with_key(input, &INDEXED_PLACEHOLDER_REGEX, |v| v[1..].to_string())
}

fn get_placeholder_token_with_key<'a>(
    input: &'a str,
    regex: &Regex,
    parse_key: fn(&str) -> String,
) -> Option<Token<'a>> {
    let mut token = get_token_on_first_match(input, TokenKind::Placeholder, regex);
    if let Some(token) = token.as_mut() {
        token.key = Some(parse_key(token.value));
    }
    token
}

fn get_escaped_placeholder_key<'a>(key: &'a str, quote_char: &str) -> String {
    let regex = Regex::new(&regex::escape(&format!("\\{}", quote_char))).unwrap();
    regex.replace_all(key, quote_char).to_string()
}

fn get_number_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::Number, &NUMBER_REGEX)
}

fn get_word_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::Word, &WORD_REGEX)
}

fn get_operator_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::Operator, &OPERATOR_REGEX)
}

fn get_reserved_word_token<'a>(
    input: &'a str,
    previous_token: Option<&Token<'a>>,
) -> Option<Token<'a>> {
    // A reserved word cannot be preceded by a "."
    // this makes it so in "my_table.from", "from" is not considered a reserved word
    if let Some(token) = previous_token {
        if token.value == "." {
            return None;
        }
    }

    get_top_level_reserved_token(input)
        .or_else(|| get_newline_reserved_token(input))
        .or_else(|| get_top_level_reserved_token_no_indent(input))
        .or_else(|| get_plain_reserved_token(input))
}

fn get_top_level_reserved_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(
        input,
        TokenKind::ReservedTopLevel,
        &RESERVED_TOP_LEVEL_REGEX,
    )
}

fn get_newline_reserved_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::ReservedNewline, &RESERVED_NEWLINE_REGEX)
}

fn get_top_level_reserved_token_no_indent(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(
        input,
        TokenKind::ReservedTopLevelNoIndent,
        &RESERVED_TOP_LEVEL_NO_INDENT_REGEX,
    )
}

fn get_plain_reserved_token(input: &str) -> Option<Token<'_>> {
    get_token_on_first_match(input, TokenKind::Reserved, &RESERVED_PLAIN_REGEX)
}

fn get_token_on_first_match<'a>(
    input: &'a str,
    kind: TokenKind,
    regex: &Regex,
) -> Option<Token<'a>> {
    let matches = regex.captures(input);
    if let Some(matches) = matches {
        Some(Token {
            kind,
            value: matches.get(1).unwrap().as_str(),
            key: None,
        })
    } else {
        None
    }
}

lazy_static! {
    static ref WHITESPACE_REGEX: Regex = Regex::new(r"^(\s+)").unwrap();
    static ref NUMBER_REGEX: Regex =
        Regex::new(r"^((-\s*)?[0-9]+(\.[0-9]+)?|0x[0-9a-fA-F]+|0b[01]+)\b").unwrap();
    static ref OPERATOR_REGEX: Regex =
        Regex::new(r"^(!=|<>|==|<=|>=|!<|!>|\|\||::|->>|->|~~\*|~~|!~~\*|!~~|~\*|!~\*|!~|:=|.)")
            .unwrap();
    static ref BLOCK_COMMENT_REGEX: Regex = Regex::new(r"^(/\*.*?(?:\*/|$))").unwrap();
    static ref LINE_COMMENT_REGEX: Regex = create_line_comment_regex(LINE_COMMENT_TYPES);
    static ref RESERVED_TOP_LEVEL_REGEX: Regex =
        create_reserved_word_regex(RESERVED_TOP_LEVEL_WORDS);
    static ref RESERVED_TOP_LEVEL_NO_INDENT_REGEX: Regex =
        create_reserved_word_regex(RESERVED_TOP_LEVEL_WORDS_NO_INDENT);
    static ref RESERVED_NEWLINE_REGEX: Regex = create_reserved_word_regex(RESERVED_NEWLINE_WORDS);
    static ref RESERVED_PLAIN_REGEX: Regex = create_reserved_word_regex(RESERVED_WORDS);
    static ref WORD_REGEX: Regex = Regex::new("^([\\p{Alphabetic}\\p{Mark}\\p{Decimal_Number}\\p{Connector_Punctuation}\\p{Join_Control}]+)").unwrap();
    static ref STRING_REGEX: Regex = create_string_regex(STRING_TYPES);
    static ref OPEN_PAREN_REGEX: Regex = create_paren_regex(OPEN_PARENS);
    static ref CLOSE_PAREN_REGEX: Regex = create_paren_regex(CLOSE_PARENS);
    static ref INDEXED_PLACEHOLDER_REGEX: Regex =
        create_placeholder_regex(INDEXED_PLACEHOLDER_TYPES, "[0-9]*");
    static ref IDENT_NAMED_PLACEHOLDER_REGEX: Regex =
        create_placeholder_regex(NAMED_PLACEHOLDER_TYPES, "[a-zA-Z0-9._$]+");
    static ref STRING_NAMED_PLACEHOLDER_REGEX: Regex = create_placeholder_regex(
        NAMED_PLACEHOLDER_TYPES,
        &create_string_pattern(STRING_TYPES)
    );
}

fn create_line_comment_regex(items: &[&str]) -> Regex {
    Regex::new(&format!(
        "^((?:{}).*?(?:\r\n|\r|\n|$))",
        items
            .iter()
            .map(|item| regex::escape(item))
            .collect::<Vec<String>>()
            .join("|")
    ))
    .unwrap()
}

fn create_reserved_word_regex(items: &[&str]) -> Regex {
    Regex::new(&format!(
        "^((?i){})\\b",
        items.join("|").replace(' ', "\\s+")
    ))
    .unwrap()
}

fn create_string_regex(items: &[&str]) -> Regex {
    Regex::new(&format!("^({})", create_string_pattern(items))).unwrap()
}

// This enables the following string patterns:
// 1. backtick quoted string using `` to escape
// 2. square bracket quoted string (SQL Server) using ]] to escape
// 3. double quoted string using "" or \" to escape
// 4. single quoted string using '' or \' to escape
// 5. national character quoted string using N'' or N\' to escape
fn create_string_pattern(items: &[&str]) -> String {
    let patterns = maplit::hashmap! {
      "``" => "((`[^`]*($|`))+)",
      "[]" => "((\\[[^\\]]*($|\\]))(\\][^\\]]*($|\\]))*)",
      "\"\"" => "((\"[^\"\\\\]*(?:\\\\.[^\"\\\\]*)*(\"|$))+)",
      "''" => "(('[^'\\\\]*(?:\\\\.[^'\\\\]*)*('|$))+)",
      "N''" => "((N'[^N'\\\\]*(?:\\\\.[^N'\\\\]*)*('|$))+)"
    };

    items
        .iter()
        .map(|item| patterns[item])
        .collect::<Vec<&str>>()
        .join("|")
}

fn create_paren_regex(items: &[&str]) -> Regex {
    Regex::new(&format!(
        "^((?i){})",
        items
            .iter()
            .map(|item| escape_paren(item))
            .collect::<Vec<String>>()
            .join("|")
    ))
    .unwrap()
}

fn escape_paren(paren: &str) -> String {
    if paren.len() == 1 {
        regex::escape(paren)
    } else {
        format!("\\b{}\\b", paren)
    }
}

fn create_placeholder_regex(items: &[&str], pattern: &str) -> Regex {
    Regex::new(&format!(
        "^((?:{})(?:{}))",
        items
            .iter()
            .map(|item| regex::escape(item))
            .collect::<Vec<String>>()
            .join("|"),
        pattern
    ))
    .unwrap()
}
