//
// Copyright (c) 2018, Jason Felice and Contributors
//
// Permission to use, copy, modify, and/or distribute this software for any purpose
// with or without fee is hereby granted, provided that the above copyright notice
// and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
// REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
// FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
// INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
// OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
// TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
// THIS SOFTWARE.

use std::fmt;

pub type LineNumber = usize;
pub type Column = usize;
pub type Delta = i64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub x: Column,
    pub line_no: LineNumber,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub cursor_x: Option<Column>,
    pub cursor_line: Option<LineNumber>,
    pub prev_cursor_x: Option<Column>,
    pub prev_cursor_line: Option<LineNumber>,
    pub prev_text: Option<String>,
    pub selection_start_line: Option<LineNumber>,
    pub changes: Vec<Change>,
    pub comment_char: char,
    pub string_delimiters: Vec<String>,
    pub lisp_vline_symbols: bool,
    pub lisp_block_comments: bool,
    pub guile_block_comments: bool,
    pub scheme_sexp_comments: bool,
    pub janet_long_strings: bool,
    pub hy_bracket_strings: bool,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub mode: String,
    pub text: String,
    pub options: Options,
}

#[derive(Clone, Debug)]
pub struct TabStop<'a> {
    pub ch: &'a str,
    pub x: Column,
    pub line_no: LineNumber,
    pub arg_x: Option<Column>,
}

#[derive(Clone, Debug)]
pub struct ParenTrail {
    pub line_no: LineNumber,
    pub start_x: Column,
    pub end_x: Column,
}

#[derive(Clone, Debug)]
pub struct Closer {
    pub trail: Option<ParenTrail>,
}

#[derive(Clone, Debug)]
pub struct Paren<'a> {
    pub line_no: LineNumber,
    pub ch: &'a str,
    pub x: Column,
    pub indent_delta: Delta,
    pub max_child_indent: Option<Column>,
    pub arg_x: Option<Column>,
    pub input_line_no: LineNumber,
    pub input_x: Column,

    pub closer: Option<Closer>,
    pub children: Vec<Paren<'a>>,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Answer<'a> {
    pub text: std::borrow::Cow<'a, str>,
    pub success: bool,
    pub error: Option<Error>,
    pub cursor_x: Option<Column>,
    pub cursor_line: Option<LineNumber>,
    pub tab_stops: Vec<TabStop<'a>>,
    pub paren_trails: Vec<ParenTrail>,
    pub parens: Vec<Paren<'a>>,
}

impl<'a> From<Error> for Answer<'a> {
    fn from(error: Error) -> Answer<'a> {
        Answer {
            text: std::borrow::Cow::from(""),
            success: false,
            error: Some(error),
            cursor_x: None,
            cursor_line: None,
            tab_stops: vec![],
            paren_trails: vec![],
            parens: vec![],
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Default)]
pub enum ErrorName {
    QuoteDanger,
    EolBackslash,
    UnclosedQuote,
    UnclosedParen,
    UnmatchedCloseParen,
    UnmatchedOpenParen,
    LeadingCloseParen,
    Utf8EncodingError,
    Panic,
    #[default]
    Restart,
}

impl fmt::Display for ErrorName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorName::QuoteDanger => f.write_str("quote-danger"),
            ErrorName::EolBackslash => f.write_str("eol-backslash"),
            ErrorName::UnclosedQuote => f.write_str("unclosed-quote"),
            ErrorName::UnclosedParen => f.write_str("unclosed-paren"),
            ErrorName::UnmatchedCloseParen => f.write_str("unmatched-close-paren"),
            ErrorName::UnmatchedOpenParen => f.write_str("unmatched-open-paren"),
            ErrorName::LeadingCloseParen => f.write_str("leading-close-paren"),
            ErrorName::Utf8EncodingError => f.write_str("utf8-error"),
            ErrorName::Panic => f.write_str("panic"),
            _ => f.write_str("??"),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Error {
    pub name: ErrorName,
    pub message: String,
    pub x: Column,
    pub line_no: LineNumber,
    pub input_x: Column,
    pub input_line_no: LineNumber,
}

impl From<std::str::Utf8Error> for Error {
    fn from(error: std::str::Utf8Error) -> Error {
        Error {
            name: ErrorName::Utf8EncodingError,
            message: format!("Error decoding UTF8: {}", error),
            ..Error::default()
        }
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(error: std::ffi::NulError) -> Error {
        Error {
            name: ErrorName::Panic,
            message: format!("{}", error),
            ..Error::default()
        }
    }
}
