use std::borrow::Cow;

use crate::indentation::Indentation;
use crate::inline_block::InlineBlock;
use crate::params::Params;
use crate::tokenizer::{Token, TokenKind};
use crate::{FormatOptions, QueryParams};

// -- fmt: off
// -- fmt: on
pub(crate) fn check_fmt_off(s: &str) -> Option<bool> {
    let mut state = 0;

    const ON: bool = false;
    const OFF: bool = true;

    const NEXT: u8 = 1;
    const STAY: u8 = 0;

    //             SPACE                SPACE  SPACE      n
    //              ┌┐                   ┌┐     ┌┐      ┌───────────► ON
    //    -      -   ▼  f      m      t   ▼  :   ▼  o   │ N
    // 0 ───► 1 ───► 2 ───► 3 ───► 4 ───► 5 ───► 6 ───► 7
    //                  F      M      T             O   │ f       f
    //                                                  └────► 8 ───► OFF
    //                                                    F       F
    for c in s.bytes() {
        state += match (state, c) {
            (0 | 1, b'-') => NEXT,
            (2, b' ') => STAY,
            (2, b'f' | b'F') => NEXT,
            (3, b'm' | b'M') => NEXT,
            (4, b't' | b'T') => NEXT,
            (5, b' ') => STAY,
            (5, b':') => NEXT,
            (6, b' ') => STAY,
            (6, b'o' | b'O') => NEXT,
            (7, b'n' | b'N') => {
                return Some(ON);
            }
            (7, b'f' | b'F') => NEXT,
            (8, b'f' | b'F') => {
                return Some(OFF);
            }
            _ => return None,
        };
    }

    None
}

pub(crate) fn format(
    tokens: &[Token<'_>],
    params: &QueryParams,
    options: &FormatOptions,
) -> String {
    let mut formatter = Formatter::new(tokens, params, options);
    let mut formatted_query = String::new();
    let mut is_fmt_enabled = true;
    let mut is_prev_token_fmt_switch = false;
    for (index, token) in tokens.iter().enumerate() {
        if is_prev_token_fmt_switch {
            is_prev_token_fmt_switch = false;
            continue;
        }
        if matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment) {
            if let Some(is_fmt_off) = check_fmt_off(token.value) {
                is_fmt_enabled = !is_fmt_off;
                is_prev_token_fmt_switch = true;
                continue;
            }
        }
        formatter.index = index;

        if !is_fmt_enabled {
            formatter.format_no_change(token, &mut formatted_query);
            continue;
        }

        match token.kind {
            TokenKind::Whitespace => {
                // ignore (we do our own whitespace formatting)
            }
            TokenKind::LineComment => {
                formatter.format_line_comment(token, &mut formatted_query);
            }
            TokenKind::BlockComment => {
                formatter.format_block_comment(token, &mut formatted_query);
            }
            TokenKind::ReservedTopLevel => {
                formatter.format_top_level_reserved_word(token, &mut formatted_query);
                formatter.previous_reserved_word = Some(token);
                formatter.previous_top_level_reserved_word = Some(token);
            }
            TokenKind::ReservedTopLevelNoIndent => {
                formatter.format_top_level_reserved_word_no_indent(token, &mut formatted_query);
                formatter.previous_reserved_word = Some(token);
                formatter.previous_top_level_reserved_word = Some(token);
            }
            TokenKind::ReservedNewline => {
                formatter.format_newline_reserved_word(token, &mut formatted_query);
                formatter.previous_reserved_word = Some(token);
            }
            TokenKind::Reserved => {
                formatter.format_with_spaces(token, &mut formatted_query);
                formatter.previous_reserved_word = Some(token);
            }
            TokenKind::OpenParen => {
                formatter.format_opening_parentheses(token, &mut formatted_query);
            }
            TokenKind::CloseParen => {
                formatter.format_closing_parentheses(token, &mut formatted_query);
            }
            TokenKind::Placeholder => {
                formatter.format_placeholder(token, &mut formatted_query);
            }
            TokenKind::DoubleColon => {
                formatter.format_double_colon(token, &mut formatted_query);
            }
            _ => match token.value {
                "," => {
                    formatter.format_comma(token, &mut formatted_query);
                }
                ":" => {
                    formatter.format_with_space_after(token, &mut formatted_query);
                }
                "." => {
                    formatter.format_without_spaces(token, &mut formatted_query);
                }
                ";" => {
                    formatter.format_query_separator(token, &mut formatted_query);
                }
                _ => {
                    formatter.format_with_spaces(token, &mut formatted_query);
                }
            },
        }
    }
    formatted_query.trim().to_string()
}

struct Formatter<'a> {
    index: usize,
    previous_reserved_word: Option<&'a Token<'a>>,
    previous_top_level_reserved_word: Option<&'a Token<'a>>,
    tokens: &'a [Token<'a>],
    params: Params<'a>,
    options: &'a FormatOptions<'a>,
    indentation: Indentation<'a>,
    inline_block: InlineBlock,
    line_start: usize,
}

impl<'a> Formatter<'a> {
    fn new(tokens: &'a [Token<'a>], params: &'a QueryParams, options: &'a FormatOptions) -> Self {
        Formatter {
            index: 0,
            previous_reserved_word: None,
            previous_top_level_reserved_word: None,
            tokens,
            params: Params::new(params),
            options,
            indentation: Indentation::new(options),
            inline_block: InlineBlock::new(
                options.max_inline_block,
                options.max_inline_arguments.is_none(),
            ),
            line_start: 0,
        }
    }

    fn format_line_comment(&mut self, token: &Token<'_>, query: &mut String) {
        let is_whitespace_followed_by_special_token =
            self.next_token(1).map_or(false, |current_token| {
                current_token.kind == TokenKind::Whitespace
                    && self.next_token(2).map_or(false, |next_token| {
                        !matches!(next_token.kind, TokenKind::Operator)
                    })
            });

        let previous_token = self.previous_token(1);
        if previous_token.is_some()
            && previous_token.unwrap().value.contains('\n')
            && is_whitespace_followed_by_special_token
        {
            self.add_new_line(query);
        } else if let Some(Token { value, .. }) = self.previous_token(2) {
            if *value == "," {
                self.trim_all_spaces_end(query);
                query.push_str("  ");
            }
        }
        query.push_str(token.value);
        self.add_new_line(query);
    }

    fn format_double_colon(&self, _token: &Token<'_>, query: &mut String) {
        self.trim_all_spaces_end(query);
        query.push_str("::");
    }
    fn format_block_comment(&mut self, token: &Token<'_>, query: &mut String) {
        self.add_new_line(query);
        query.push_str(&self.indent_comment(token.value));
        self.add_new_line(query);
    }

    fn format_top_level_reserved_word(&mut self, token: &Token<'_>, query: &mut String) {
        self.indentation.decrease_top_level();
        self.add_new_line(query);
        self.indentation.increase_top_level();
        query.push_str(&self.equalize_whitespace(&self.format_reserved_word(token.value)));
        self.add_new_line(query);
    }

    fn format_top_level_reserved_word_no_indent(&mut self, token: &Token<'_>, query: &mut String) {
        self.indentation.decrease_top_level();
        self.add_new_line(query);
        query.push_str(&self.equalize_whitespace(&self.format_reserved_word(token.value)));
        self.add_new_line(query);
    }

    fn format_newline_reserved_word(&mut self, token: &Token<'_>, query: &mut String) {
        if self
            .options
            .max_inline_arguments
            .map_or(true, |limit| limit < self.line_len_next(query))
        {
            self.add_new_line(query);
        } else {
            self.trim_spaces_end(query);
            query.push(' ');
        }
        query.push_str(&self.equalize_whitespace(&self.format_reserved_word(token.value)));
        query.push(' ');
    }

    fn format_with_spaces(&self, token: &Token<'_>, query: &mut String) {
        if token.kind == TokenKind::Reserved {
            let value = self.equalize_whitespace(&self.format_reserved_word(token.value));
            query.push_str(&value);
            query.push(' ');
        } else {
            query.push_str(token.value);
            query.push(' ');
        };
    }

    // Opening parentheses increase the block indent level and start a new line
    fn format_opening_parentheses(&mut self, token: &Token<'_>, query: &mut String) {
        const PRESERVE_WHITESPACE_FOR: &[TokenKind] = &[
            TokenKind::Whitespace,
            TokenKind::OpenParen,
            TokenKind::LineComment,
        ];

        // Take out the preceding space unless there was whitespace there in the original query
        // or another opening parens or line comment
        let previous_token = self.previous_token(1);
        if previous_token.is_none()
            || !PRESERVE_WHITESPACE_FOR.contains(&previous_token.unwrap().kind)
        {
            self.trim_spaces_end(query);
        }

        let value = match (
            self.options.uppercase,
            self.options.ignore_case_convert.as_ref(),
        ) {
            (Some(uppercase), Some(values)) if !values.contains(&token.value) => {
                if uppercase {
                    Cow::Owned(token.value.to_uppercase())
                } else {
                    Cow::Owned(token.value.to_lowercase())
                }
            }
            (Some(uppercase), None) => {
                if uppercase {
                    Cow::Owned(token.value.to_uppercase())
                } else {
                    Cow::Owned(token.value.to_lowercase())
                }
            }
            _ => Cow::Borrowed(token.value),
        };

        query.push_str(&value);

        self.inline_block.begin_if_possible(self.tokens, self.index);

        if !self.inline_block.is_active() {
            self.indentation.increase_block_level();
            self.add_new_line(query);
        }
    }

    // Closing parentheses decrease the block indent level
    fn format_closing_parentheses(&mut self, token: &Token<'_>, query: &mut String) {
        let mut token = token.clone();
        let value = match (
            self.options.uppercase,
            self.options.ignore_case_convert.as_ref(),
        ) {
            (Some(uppercase), Some(values)) if !values.contains(&token.value) => {
                if uppercase {
                    Cow::Owned(token.value.to_uppercase())
                } else {
                    Cow::Owned(token.value.to_lowercase())
                }
            }
            (Some(uppercase), None) => {
                if uppercase {
                    Cow::Owned(token.value.to_uppercase())
                } else {
                    Cow::Owned(token.value.to_lowercase())
                }
            }
            _ => Cow::Borrowed(token.value),
        };

        token.value = &value;

        if self.inline_block.is_active() {
            self.inline_block.end();
            if token.value.to_lowercase() == "end" {
                self.trim_spaces_end(query);
                query.push(' ');
                self.format_with_spaces(&token, query);
            } else {
                self.format_with_space_after(&token, query);
            }
        } else {
            self.indentation.decrease_block_level();
            self.add_new_line(query);
            self.format_with_spaces(&token, query);
        }
    }

    fn format_placeholder(&mut self, token: &'a Token<'a>, query: &mut String) {
        query.push_str(self.params.get(token));
        query.push(' ');
    }

    // Commas start a new line (unless within inline parentheses or SQL "LIMIT" clause)
    fn format_comma(&mut self, token: &Token<'_>, query: &mut String) {
        self.trim_spaces_end(query);
        query.push_str(token.value);
        query.push(' ');

        if self.inline_block.is_active() {
            return;
        }
        if self
            .previous_reserved_word
            .map(|word| word.value.to_lowercase() == "limit")
            .unwrap_or(false)
        {
            return;
        }

        if matches!((self.previous_top_level_reserved_word, self.options.max_inline_arguments),
            (Some(word), Some(limit)) if word.value.to_lowercase() == "select" && limit > self.line_len_next(query))
        {
            return;
        }
        self.add_new_line(query);
    }

    fn format_with_space_after(&self, token: &Token<'_>, query: &mut String) {
        self.trim_spaces_end(query);
        query.push_str(token.value);
        query.push(' ');
    }

    fn format_without_spaces(&self, token: &Token<'_>, query: &mut String) {
        self.trim_spaces_end(query);
        query.push_str(token.value);
    }

    fn format_query_separator(&mut self, token: &Token<'_>, query: &mut String) {
        self.indentation.reset_indentation();
        self.trim_spaces_end(query);
        query.push_str(token.value);
        for _ in 0..self.options.lines_between_queries {
            query.push('\n');
        }
    }

    fn add_new_line(&mut self, query: &mut String) {
        self.trim_spaces_end(query);
        if self.options.inline {
            query.push(' ');
            return;
        }
        if !query.ends_with('\n') {
            query.push('\n');
        }
        self.line_start = query.len();
        query.push_str(&self.indentation.get_indent());
    }

    fn trim_spaces_end(&self, query: &mut String) {
        query.truncate(query.trim_end_matches([' ', '\t']).len());
    }

    fn trim_all_spaces_end(&self, query: &mut String) {
        query.truncate(query.trim_end_matches(|c: char| c.is_whitespace()).len());
    }

    fn indent_comment(&self, token: &str) -> String {
        let mut combined = String::with_capacity(token.len() + 4);
        for (i, line) in token.split('\n').enumerate() {
            if i == 0 {
                combined.push_str(line)
            } else if line.starts_with([' ', '\t']) {
                let indent = self.indentation.get_indent();
                let start_trimmed = line.trim_start_matches([' ', '\t']);
                combined.reserve(indent.len() + start_trimmed.len() + 2);
                combined.push('\n');
                combined.push_str(&indent);
                combined.push(' ');
                combined.push_str(start_trimmed);
            } else {
                combined.reserve(line.len() + 1);
                combined.push('\n');
                combined.push_str(line);
            }
        }
        combined
    }

    fn format_reserved_word<'t>(&self, token: &'t str) -> Cow<'t, str> {
        match (
            self.options.uppercase,
            self.options.ignore_case_convert.as_ref(),
        ) {
            (Some(uppercase), Some(values)) if !values.contains(&token) => {
                if uppercase {
                    Cow::Owned(token.to_uppercase())
                } else {
                    Cow::Owned(token.to_lowercase())
                }
            }
            (Some(uppercase), None) => {
                if uppercase {
                    Cow::Owned(token.to_uppercase())
                } else {
                    Cow::Owned(token.to_lowercase())
                }
            }
            _ => Cow::Borrowed(token),
        }
    }

    /// Replace any sequence of whitespace characters with single space
    fn equalize_whitespace(&self, token: &str) -> String {
        let mut combined = String::with_capacity(token.len());
        for s in token.split(char::is_whitespace).filter(|s| !s.is_empty()) {
            if !combined.is_empty() {
                combined.push(' ');
            }
            combined.push_str(s);
        }
        combined
    }

    fn previous_token(&self, idx: usize) -> Option<&Token<'_>> {
        let index = self.index.checked_sub(idx);
        if let Some(index) = index {
            self.tokens.get(index)
        } else {
            None
        }
    }

    fn next_token(&self, idx: usize) -> Option<&Token<'_>> {
        let index = self.index.checked_add(idx);
        if let Some(index) = index {
            self.tokens.get(index)
        } else {
            None
        }
    }

    fn line_len_next(&self, query: &str) -> usize {
        query.len() - self.line_start + self.next_token(1).map_or(0, |t| t.value.len())
    }

    fn format_no_change(&self, token: &Token<'_>, query: &mut String) {
        query.push_str(token.value);
    }
}
