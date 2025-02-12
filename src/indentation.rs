use crate::{tokenizer::Token, FormatOptions, Indent, SpanInfo};

#[derive(Debug, Default)]
struct PreviousTokens<'a> {
    top_level_reserved: Option<&'a Token<'a>>,
    reserved: Option<&'a Token<'a>>,
}

pub(crate) struct Indentation<'a> {
    options: &'a FormatOptions<'a>,
    indent_types: Vec<IndentType>,
    top_level_span: Vec<SpanInfo>,
    previous: Vec<PreviousTokens<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndentType {
    TopLevel,
    BlockLevel,
}

impl<'a> Indentation<'a> {
    pub fn new(options: &'a FormatOptions) -> Self {
        Indentation {
            options,
            indent_types: Vec::new(),
            top_level_span: Vec::new(),
            previous: Vec::new(),
        }
    }

    pub fn get_indent(&self) -> String {
        match self.options.indent {
            Indent::Spaces(num_spaces) => " "
                .repeat(num_spaces as usize)
                .repeat(self.indent_types.len()),
            Indent::Tabs => "\t".repeat(self.indent_types.len()),
        }
    }

    pub fn increase_top_level(&mut self, span: SpanInfo) {
        self.indent_types.push(IndentType::TopLevel);
        self.top_level_span.push(span);
    }

    pub fn increase_block_level(&mut self) {
        self.indent_types.push(IndentType::BlockLevel);
        self.previous.push(Default::default());
    }

    pub fn decrease_top_level(&mut self) {
        if self.indent_types.last() == Some(&IndentType::TopLevel) {
            self.indent_types.pop();
            self.top_level_span.pop();
            self.previous.pop();
        }
    }

    pub fn decrease_block_level(&mut self) {
        while !self.indent_types.is_empty() {
            let kind = self.indent_types.pop();
            self.previous.pop();
            if kind != Some(IndentType::TopLevel) {
                break;
            }
        }
    }

    pub fn reset_indentation(&mut self) {
        self.indent_types.clear();
        self.top_level_span.clear();
        self.previous.clear();
    }

    pub fn set_previous_reserved(&mut self, token: &'a Token<'a>) {
        if let Some(previous) = self.previous.last_mut() {
            previous.reserved = Some(token);
        } else {
            self.previous.push(PreviousTokens {
                top_level_reserved: None,
                reserved: Some(token),
            });
        }
    }

    pub fn set_previous_top_level(&mut self, token: &'a Token<'a>) {
        if let Some(previous) = self.previous.last_mut() {
            previous.top_level_reserved = Some(token);
        } else {
            self.previous.push(PreviousTokens {
                top_level_reserved: Some(token),
                reserved: Some(token),
            });
        }
    }

    pub fn previous_reserved(&'a self) -> Option<&'a Token<'a>> {
        if let Some(PreviousTokens {
            reserved,
            top_level_reserved: _,
        }) = self.previous.last()
        {
            reserved.as_deref()
        } else {
            None
        }
    }

    pub fn previous_top_level_reserved(&'a self) -> Option<&'a Token<'a>> {
        if let Some(PreviousTokens {
            top_level_reserved,
            reserved: _,
        }) = self.previous.last()
        {
            top_level_reserved.as_deref()
        } else {
            None
        }
    }

    /// The full span between two top level tokens
    pub fn span(&self) -> usize {
        self.top_level_span.last().map_or(0, |span| span.full_span)
    }
}
