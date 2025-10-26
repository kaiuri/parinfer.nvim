use crate::types;

pub fn dialect_options(lang: &str) -> types::Options {
    let mut options = types::Options {
        cursor_line: None,
        cursor_x: None,
        prev_cursor_x: None,
        prev_cursor_line: None,
        prev_text: None,
        selection_start_line: None,
        changes: vec![],
        comment_char: ';',
        lisp_vline_symbols: false,
        lisp_block_comments: false,
        guile_block_comments: false,
        scheme_sexp_comments: false,
        janet_long_strings: false,
        hy_bracket_strings: false,
        string_delimiters: vec!["\"".to_string()],
    };
    match lang {
        "hy" => options.hy_bracket_strings = true,
        "yuck" => {
            options.string_delimiters.push("'".to_string());
            options.string_delimiters.push("`".to_string());
        }
        "janet" => {
            options.comment_char = '#';
            options.janet_long_strings = true;
        }
        s if s.contains("lisp") => {
            options.lisp_vline_symbols = true;
            options.lisp_block_comments = true;
        }
        "racket" | "scheme" | "chicken" | "query" => {
            options.lisp_vline_symbols = true;
            options.lisp_block_comments = true;
            options.scheme_sexp_comments = true;
            options.guile_block_comments = true;
        }
        "clojure" | "fennel" | "carp" | "wast" => (),
        _ => (),
    }
    options
}
