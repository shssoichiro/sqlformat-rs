use crate::tokenizer::{Token, TokenKind};

pub(crate) struct InlineBlock {
    level: usize,
    inline_max_length: usize,
}

impl Default for InlineBlock {
    fn default() -> Self {
        InlineBlock {
            level: 0,
            inline_max_length: 50,
        }
    }
}

impl InlineBlock {
    pub fn new(inline_max_length: usize) -> Self {
        InlineBlock {
            level: 0,
            inline_max_length,
        }
    }

    pub fn begin_if_possible(&mut self, tokens: &[Token<'_>], index: usize) {
        if self.level == 0 && self.is_inline_block(tokens, index) {
            self.level = 1;
        } else if self.level > 0 {
            self.level += 1;
        } else {
            self.level = 0;
        }
    }

    pub fn end(&mut self) {
        self.level -= 1;
    }

    pub fn is_active(&self) -> bool {
        self.level > 0
    }

    fn is_inline_block(&self, tokens: &[Token<'_>], index: usize) -> bool {
        let mut length = 0;
        let mut level = 0;

        for token in &tokens[index..] {
            length += token.value.len();

            // Overran max length
            if length > self.inline_max_length {
                return false;
            }
            if token.kind == TokenKind::OpenParen {
                level += 1;
            } else if token.kind == TokenKind::CloseParen {
                level -= 1;
                if level == 0 {
                    return true;
                }
            }

            if self.is_forbidden_token(token) {
                return false;
            }
        }

        false
    }

    fn is_forbidden_token(&self, token: &Token<'_>) -> bool {
        token.kind == TokenKind::ReservedTopLevel
            || token.kind == TokenKind::ReservedNewline
            || token.kind == TokenKind::LineComment
            || token.kind == TokenKind::BlockComment
            || token.value == ";"
    }
}
