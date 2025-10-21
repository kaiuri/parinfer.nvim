use unicode_segmentation::UnicodeSegmentation;

use crate::parinfer::{self};
use unicode_width::UnicodeWidthStr;

use crate::types::{self};
use mlua::prelude::*;
use std::str::FromStr;

macro_rules! runtime_error {
    ($msg:expr) => {
        mlua::Error::RuntimeError($msg.into())
    };
}

macro_rules! conversion_error {
    ($from:literal, $to:literal, $message:expr) => {
        mlua::Error::FromLuaConversionError {
            from: $from,
            to: $to.into(),
            message: $message,
        }
    };
}

pub(crate) fn parinfer_shift_indent(
    lua: &mlua::Lua,
    (tab_stops, dx, x): (Vec<IntegrationTabStop>, i32, i32),
) -> LuaResult<LuaValue> {
    lua.pack(shift_indent(&tab_stops, dx, x))
}

pub(crate) fn parinfer_to_bytepos(
    lua: &mlua::Lua,
    (line, charpos): (mlua::String, usize),
) -> LuaResult<LuaValue> {
    if let Ok(line) = line.to_str() {
        charpos_to_bytepos(&line, charpos).into_lua(lua)
    } else {
        Err(runtime_error!(format!("invalid line {line:?}")))
    }
}

pub(crate) fn parinfer_to_charpos(
    lua: &mlua::Lua,
    (line, bytepos): (mlua::String, usize),
) -> LuaResult<LuaValue> {
    if let Ok(line) = line.to_str() {
        bytepos_to_charpos(&line, bytepos).into_lua(lua)
    } else {
        Err(runtime_error!(format!("invalid line {line:?}")))
    }
}

// Input is a table with the following keys:
//   [1] string (dialect)
//   [2..3] (string[], [number, number]) (current state)
//   [4..5] (string[], [number, number])? (previous state)
// Will shift 2. and 3. to 4. and 5. respectively and set 2. and 3. to a new
// state IF there's one, else, keeps it unchanged it avoids needlessly creating
// new tables this function does not check if cursors are valid given their
// lines.
// Returns { tab_stops: [number;4], paren_trails: [number;3], error: { name: string, message: string, row: number, col: number } }
// and, if there's any difference between the new state and the previous state, a 4-element array describing their difference (See `diff_slice` for what that means).
pub(crate) fn parinfer_run(lua: &mlua::Lua, value: mlua::Value) -> mlua::Result<mlua::MultiValue> {
    let table = if let Some(table) = value.as_table() {
        table
    } else {
        return lua.pack_multi(runtime_error!("expected table"));
    };

    let dialect: Dialect = table.get(1)?;
    // it's an error to provide one and not the other
    let current_state = table
        .get::<Vec<String>>(2)
        .ok()
        .zip(table.get::<Cursor>(3).ok())
        .map(Ok)
        .unwrap_or(Err(runtime_error!(
            "invalid state, current_lines (2) or current_cursor (3) is missing"
        )))?;
    // previous lines and cursor, must be both present or both absent
    // anything else is ignored since it's invalid
    let previous_state = table
        .get::<Option<Vec<String>>>(4)?
        .zip(table.get::<Option<Cursor>>(5)?);
    let (response, changes, new_state) = run(&dialect, current_state, previous_state);

    // current_state is now also the previous_state
    table.set(4, table.get::<mlua::Value>(2)?)?;
    table.set(5, table.get::<mlua::Value>(3)?)?;

    // if there's a new state, set it
    if let Some((lines, cursor)) = new_state {
        table.set(2, lines)?;
        table.set(3, cursor)?;
    }
    lua.pack_multi((response, changes))
}

// 0: lines, 1: cursor
type State = (Vec<String>, Cursor);

fn run(
    dialect: &Dialect,
    current_state: State,
    previous_state: Option<State>,
) -> (IntegrationResponse, Option<[usize; 4]>, Option<State>) {
    let mut options = dialect.options();
    let mut mode = "paren";
    let cur_lines = {
        let (lines, [lnum, bytepos]) = current_state;
        let row = lnum - 1;
        options.cursor_line = Some(row);
        options.cursor_x = Some(bytepos_to_charpos(&lines[row], bytepos));
        lines
    };

    if let Some((lines, [lnum, bytepos])) = previous_state {
        mode = "smart";
        options.prev_text = Some(lines.join("\n"));
        if !lines.is_empty() {
            options.prev_cursor_line = Some(lnum - 1);
            options.prev_cursor_x = Some(bytepos_to_charpos(&lines[lnum - 1], bytepos));
        }
    }
    let request = types::Request {
        mode: mode.into(),
        text: cur_lines.join("\n"),
        options,
    };
    let answer = parinfer::process(&request);
    let response = IntegrationResponse::new(&answer);
    if answer.success && answer.text != request.text {
        let new_lines = answer
            .text
            .split('\n')
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let changes = diff_slice(&cur_lines, &new_lines);
        let (row, charpos) = answer.cursor_line.zip(answer.cursor_x).unwrap();
        let lnum = row + 1;
        let new_cursor = [lnum, charpos_to_bytepos(&new_lines[row], charpos)];
        (response, changes, Some((new_lines, new_cursor)))
    } else {
        (response, None, None)
    }
}

// calculates the positions in a and b where they start and stop to be different
// e.g.
// given a = [0,1,2,3] and b = [3,2,1,0], diff = [0, 3, 0, 3]
// where
//   a[diff[0]..=diff[1]] is which slice of a is different from b
//   b[diff[2]..=diff[3]] is which slice of b is different from a
fn diff_slice<T: std::cmp::Eq>(a: &[T], b: &[T]) -> Option<[usize; 4]> {
    if a.is_empty() && b.is_empty() {
        // they're equally empty
        return None;
    } else if a.is_empty() || b.is_empty() {
        // one is empty, other is not
        return Some([0, a.len().saturating_sub(1), 0, b.len().saturating_sub(1)]);
    }
    let different = |(left, right)| left != right;
    let mut ab = a.iter().zip(b.iter());
    let mut ba = b.iter().zip(a.iter());
    let stop_a = ab.rposition(different)?; // they're not different, otherwise it'd be `Some`thing
    // because we're not cloning here, and we know that a and b are different,
    // we also know that if position returns None it means that .position would be the same as .rposition
    let start_a = ab.position(different).unwrap_or(stop_a);
    let stop_b = ba.rposition(different)?; // should never happen
    let start_b = ba.position(different).unwrap_or(stop_b);
    Some([start_a, stop_a, start_b, stop_b])
}

#[test]
fn test_diff_slice() {
    let a = vec![1, 2, 3];
    let b = vec![3, 2, 1];
    assert_eq!(diff_slice(&a, &b), Some([0, 2, 0, 2]));
    let a = vec![1, 2, 3, 4];
    let b = vec![1, 2, 4, 3];
    assert_eq!(diff_slice(&a, &b), Some([2, 3, 2, 3]));
    let a = vec![1, 2, 3, 4];
    let b = vec![1, 2, 4, 4];
    assert_eq!(diff_slice(&a, &b), Some([2, 2, 2, 2]));
    let a = vec![];
    let b = vec![1, 2, 4, 4];
    assert_eq!(diff_slice(&a, &b), Some([0, 0, 0, 3]));
    let a = vec![1, 2, 4, 4];
    let b = vec![];
    assert_eq!(diff_slice(&a, &b), Some([0, 3, 0, 0]));
}

// neovim rounds byteindexes up, meaning, even if cursor is over a multibyte character,
// i.e. if cursor is at the start of a multibyte-character, moving it even one byte right,
// will move the cursor to the next character, not keep it inside the character
// e.g. given line "😀 ok!" and cursor [1, 0] --> inside the 😀 setting cursor to [1, 1] gets rounded up to [1, 4]
// therefore, when converting from byteindex to charindex, we should look for
// the first grapheme whose byteindex is either exactly equal to byteidx or greater than it
fn bytepos_to_charpos(line: &str, byteidx: usize) -> usize {
    line.grapheme_indices(true)
        .position(|(i, _)| i >= byteidx)
        .unwrap_or(line.width_cjk())
}
#[test]
fn test_bytepos_to_charpos() {
    assert_eq!(bytepos_to_charpos("abc", 0), 0);
    assert_eq!(bytepos_to_charpos("abc", 1), 1);
    assert_eq!(bytepos_to_charpos("abc", 2), 2);
    assert_eq!(bytepos_to_charpos("åbc", 0), 0);
    assert_eq!(bytepos_to_charpos("åbc", 1), 1);
    assert_eq!(bytepos_to_charpos("åbc", 2), 1);
    assert_eq!(bytepos_to_charpos("åbc", 3), 2);
    assert_eq!(bytepos_to_charpos("ｗｏa", 0), 0);
    assert_eq!(bytepos_to_charpos("ｗｏa", 1), 1);
    assert_eq!(bytepos_to_charpos("ｗｏa", 2), 1);
    assert_eq!(bytepos_to_charpos("ｗｏa", 3), 1);
    assert_eq!(bytepos_to_charpos("ｗｏa", 4), 2);
    assert_eq!(bytepos_to_charpos("ｗｏa", 5), 2);
    assert_eq!(bytepos_to_charpos("ｗｏa", 6), 2);
}
//  ...so, when converting from charindex to byteindex, unless the cursor is
//  exactly at the starting byte of character, its at the next character's byte
fn charpos_to_bytepos(line: &str, charidx: usize) -> usize {
    line.grapheme_indices(true)
        .nth(charidx)
        .map(|(i, _)| i)
        .unwrap_or(line.len())
}

#[test]
fn test_charpos_to_bytepos() {
    assert_eq!(charpos_to_bytepos("", 0), 0);
    assert_eq!(charpos_to_bytepos("", 1), 0);
    assert_eq!(charpos_to_bytepos("abc", 0), 0);
    assert_eq!(charpos_to_bytepos("abc", 1), 1);
    assert_eq!(charpos_to_bytepos("abc", 2), 2);
    assert_eq!(charpos_to_bytepos("åbc", 0), 0);
    assert_eq!(charpos_to_bytepos("åbc", 1), 2);
    assert_eq!(charpos_to_bytepos("åbc", 2), 3);
    assert_eq!(charpos_to_bytepos("ｗｏa", 0), 0);
    assert_eq!(charpos_to_bytepos("ｗｏa", 1), 3);
    assert_eq!(charpos_to_bytepos("ｗｏa", 1), 3);
    assert_eq!(charpos_to_bytepos("ｗｏa", 1), 3);
    assert_eq!(charpos_to_bytepos("ｗｏa", 2), 6);
}

fn shift_indent(tab_stops: &[IntegrationTabStop], dx: i32, x: i32) -> i32 {
    if x == 0 && dx < 0 {
        return 0;
    }
    let mut tabs: Vec<i32> =
        tab_stops
            .iter()
            .fold(vec![], |mut tabs, &[_line_no, x, ch, arg_x]| {
                let col = x as i32;
                tabs.push(col);
                tabs.push(col + if ch == 0x28 { 2 } else { 1 });
                if arg_x != 0 {
                    tabs.push(arg_x as i32);
                }
                tabs
            });

    tabs.dedup();
    tabs.sort();

    if dx < 0 {
        tabs.into_iter().rfind(|&i| i < x)
    } else {
        tabs.into_iter().find(|&i| i > x)
    }
    .unwrap_or(x.saturating_add(2 * dx))
}

#[test]
fn test_shift_indent() {
    // 0 1 2 3 4 10
    let tab_stops = [[1, 0, 0x28, 4], [1, 1, 0x28, 2], [1, 2, 0x28, 10]];
    assert_eq!(shift_indent(&tab_stops, 1, 0), 1);
    assert_eq!(shift_indent(&tab_stops, -1, 0), 0);
    assert_eq!(shift_indent(&tab_stops, -1, 20), 10);
}

// ----------------------------------------------
// collection of structs that we can mlua easily
type Cursor = [usize; 2];
type IntegrationParenTrail = [usize; 3];
type IntegrationTabStop = [usize; 4];
struct IntegrationResponse {
    tab_stops: Vec<IntegrationTabStop>,
    /// vector of [line_no, start_x, end_x], for each paren_trail i.e. `closers`
    paren_trails: Vec<IntegrationParenTrail>,
    error: Option<IntegrationError>,
}

impl IntegrationResponse {
    fn new<'a>(value: &types::Answer<'a>) -> Self {
        let tab_stops = value
            .tab_stops
            .iter()
            .map(|t| {
                [
                    t.line_no,
                    t.x,
                    t.ch.bytes().next().unwrap_or(0x28).into(),
                    t.arg_x.unwrap_or(0),
                ]
            })
            .collect();
        Self {
            tab_stops,
            paren_trails: value
                .paren_trails
                .iter()
                .map(|t| [t.line_no, t.start_x, t.end_x])
                .collect(),
            error: value.error.as_ref().map(IntegrationError::new),
        }
    }
}

impl mlua::IntoLua for IntegrationResponse {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let info = lua.create_table()?;
        info.set("tab_stops", self.tab_stops)?;
        info.set("paren_trails", self.paren_trails)?;
        info.set("error", self.error)?;
        Ok(LuaValue::Table(info))
    }
}

struct IntegrationError {
    name: String,
    message: String,
    col: usize,
    row: usize,
}

impl IntegrationError {
    fn new(value: &types::Error) -> Self {
        IntegrationError {
            name: format!("{}", value.name),
            message: value.message.clone(),
            col: value.x,
            row: value.line_no,
        }
    }
}

impl mlua::IntoLua for IntegrationError {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        table.set("name", self.name)?;
        table.set("message", self.message)?;
        table.set("col", self.col)?;
        table.set("row", self.row)?;
        Ok(LuaValue::Table(table))
    }
}

// dialects, helps avoiding constructing huge request and response tables
#[derive(Debug, PartialEq)]
enum Dialect {
    Scheme,
    Lisp,
    Clojure,
    Hy,
    Janet,
    Yuck,
}
impl FromStr for Dialect {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hy" => Ok(Dialect::Hy),
            "janet" => Ok(Dialect::Janet),
            "yuck" => Ok(Dialect::Yuck),
            "racket" | "scheme" | "chicken" | "query" => Ok(Dialect::Scheme),
            s if s.contains("lisp") => Ok(Dialect::Lisp),
            "clojure" | "fennel" | "carp" | "wast" => Ok(Dialect::Clojure),
            _ => Ok(Dialect::Clojure),
        }
    }
}

impl mlua::FromLua for Dialect {
    fn from_lua(value: mlua::Value, _lua: &mlua::Lua) -> mlua::Result<Self> {
        let out = match value {
            mlua::Value::String(lang) => match lang.to_str() {
                Ok(s) => Dialect::from_str(&s).unwrap_or(Dialect::Clojure),
                Err(_) => Dialect::Clojure,
            },
            _ => {
                return Err(conversion_error!("string", "Dialect", None));
            }
        };
        Ok(out)
    }
}

impl Dialect {
    fn options(&self) -> crate::types::Options {
        let mut options = crate::types::Options {
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
        match self {
            Dialect::Hy => options.hy_bracket_strings = true,
            Dialect::Yuck => {
                options.string_delimiters = vec!["\"".to_string(), "'".to_string(), "`".to_string()]
            }
            Dialect::Janet => {
                options.comment_char = '#';
                options.janet_long_strings = true;
            }
            Dialect::Lisp => {
                options.lisp_vline_symbols = true;
                options.lisp_block_comments = true;
            }
            Dialect::Scheme => {
                options.lisp_vline_symbols = true;
                options.lisp_block_comments = true;
                options.scheme_sexp_comments = true;
                options.guile_block_comments = true;
            }
            _ => (),
        }
        options
    }
}

#[test]
fn dialect_from_str() {
    let scheme = vec!["scheme", "racket", "chicken", "query"];
    for lang in scheme {
        assert_eq!(Dialect::from_str(lang), Ok(Dialect::Scheme));
    }
    let langs = vec!["clojure", "fennel", "carp", "wast"];
    for lang in langs {
        assert_eq!(Dialect::from_str(lang), Ok(Dialect::Clojure));
    }
    let langs = vec!["commonlisp", "lisp", "maclisp"];
    for lang in langs {
        assert_eq!(Dialect::from_str(lang), Ok(Dialect::Lisp));
    }
}

#[test]
fn test_run() {
    macro_rules! string_split {
        ($text:expr) => {
            $text.split('\n').map(|s| s.to_string()).collect::<Vec<_>>()
        };
    }
    macro_rules! editor_state {
        ($text:expr, $cursor:expr) => {
            (string_split!($text), $cursor)
        };
    }
    let dialect = Dialect::Clojure;
    let editor_history = [
        // current_state                expected_state
        (editor_state!("(", [1, 1]), editor_state!("()", [1, 1])),
        (
            editor_state!("(\n)", [2, 0]),
            editor_state!("(\n )", [2, 1]),
        ),
        (
            editor_state!("(\n [)", [2, 2]),
            editor_state!("(\n [])", [2, 2]),
        ),
        (
            editor_state!("(\n [{])", [2, 3]),
            editor_state!("(\n [{}])", [2, 3]),
        ),
        (
            editor_state!("(\n [{\n}])", [3, 0]),
            editor_state!("(\n [{\n   }])", [3, 3]),
        ),
    ];
    // previous state is the result of the previous tuple's expected state
    // which should match the result of running over the previous tuple's current_state
    for (i, (current_state, expected_state)) in editor_history.iter().enumerate() {
        let previous_state = editor_history
            .get(i.saturating_sub(1))
            .map(|(_, a)| a.clone());
        let (_, _, new_state) = run(&dialect, current_state.clone(), previous_state);
        assert_eq!(*expected_state, new_state.unwrap());
    }
}
