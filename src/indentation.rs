use crate::{tokenizer::Token, FormatOptions, Indent, SpanInfo};

#[derive(Debug, Default)]
struct PreviousTokens<'a> {
    top_level_reserved: Option<(&'a Token<'a>, SpanInfo)>,
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
    Top,
    Block,
    FoldedBlock,
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

    pub fn get_indent(&self, folded: bool) -> String {
        // TODO compute in place?
        let level = self
            .indent_types
            .iter()
            .copied()
            .filter(|t| *t != IndentType::FoldedBlock)
            .count()
            - if folded { 1 } else { 0 };
        match self.options.indent {
            Indent::Spaces(num_spaces) => " ".repeat(num_spaces as usize).repeat(level),
            Indent::Tabs => "\t".repeat(level),
        }
    }

    pub fn increase_top_level(&mut self, span: SpanInfo) {
        self.indent_types.push(IndentType::Top);
        self.top_level_span.push(span);
    }

    pub fn increase_block_level(&mut self, folded: bool) {
        self.indent_types.push(if folded {
            IndentType::FoldedBlock
        } else {
            IndentType::Block
        });
        self.previous.push(Default::default());
    }

    pub fn decrease_top_level(&mut self) {
        if self.indent_types.last() == Some(&IndentType::Top) {
            self.indent_types.pop();
            self.top_level_span.pop();
            self.previous.pop();
        }
    }

    /// Return true if the block was folded
    pub fn decrease_block_level(&mut self) -> bool {
        let mut folded = false;
        while !self.indent_types.is_empty() {
            let kind = self.indent_types.pop();
            self.previous.pop();
            folded = kind == Some(IndentType::FoldedBlock);
            if kind != Some(IndentType::Top) {
                break;
            }
        }
        folded
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

    pub fn set_previous_top_level(&mut self, token: &'a Token<'a>, span_info: SpanInfo) {
        if let Some(previous) = self.previous.last_mut() {
            previous.top_level_reserved = Some((token, span_info));
        } else {
            self.previous.push(PreviousTokens {
                top_level_reserved: Some((token, span_info)),
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

    pub fn previous_top_level_reserved(&'a self) -> Option<(&'a Token<'a>, &'a SpanInfo)> {
        if let Some(PreviousTokens {
            top_level_reserved,
            reserved: _,
        }) = self.previous.last()
        {
            top_level_reserved.as_ref().map(|&(t, ref s)| (t, s))
        } else {
            None
        }
    }

    /// The full span between two top level tokens
    pub fn span(&self) -> usize {
        self.top_level_span.last().map_or(0, |span| span.full_span)
    }
}
