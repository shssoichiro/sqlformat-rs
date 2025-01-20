use crate::{FormatOptions, Indent};

pub(crate) struct Indentation<'a> {
    options: &'a FormatOptions<'a>,
    indent_types: Vec<IndentType>,
    top_level_span: Vec<usize>,
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

    pub fn increase_top_level(&mut self, span: usize) {
        self.indent_types.push(IndentType::TopLevel);
        self.top_level_span.push(span);
    }

    pub fn increase_block_level(&mut self) {
        self.indent_types.push(IndentType::BlockLevel);
    }

    pub fn decrease_top_level(&mut self) {
        if self.indent_types.last() == Some(&IndentType::TopLevel) {
            self.indent_types.pop();
            self.top_level_span.pop();
        }
    }

    pub fn decrease_block_level(&mut self) {
        while !self.indent_types.is_empty() {
            let kind = self.indent_types.pop();
            if kind != Some(IndentType::TopLevel) {
                break;
            }
        }
    }

    pub fn reset_indentation(&mut self) {
        self.indent_types.clear();
        self.top_level_span.clear();
    }

    pub fn top_level_span(&self) -> usize {
        self.top_level_span.last().map_or(0, |span| *span)
    }
}
