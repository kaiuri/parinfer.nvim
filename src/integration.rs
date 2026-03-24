use std::ops::Range;

use crate::position;
use crate::parinfer;
use crate::types;

macro_rules! runtime_error {
    ($msg:expr) => {
        Err(mlua::Error::RuntimeError($msg.into()))
    };
}

#[derive(Debug, PartialEq, Clone)]
pub struct EditorState {
    lines: Vec<String>,
    cursor: [usize; 2],
}

impl mlua::FromLua for EditorState {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        let Some(table) = value.as_table() else {
            return runtime_error!("expected table");
        };
        let lines = table.get::<Vec<String>>(1)?;
        let [lnum, bytepos] = table.get::<[usize; 2]>(2)?;
        let row = lnum - 1;
        let charpos = position::bytepos_to_charpos(&lines[row], bytepos);
        Ok(Self {
            lines,
            cursor: [row, charpos],
        })
    }
}
impl mlua::IntoLua for EditorState {
    fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        let table = lua.create_table()?;
        let lines = self.lines;
        let [row, charpos] = self.cursor;
        let lnum = row + 1;
        let bytepos = position::charpos_to_bytepos(&lines[row], charpos);
        table.set(1, lines)?;
        table.set(2, [lnum, bytepos])?;
        Ok(mlua::Value::Table(table))
    }
}

pub fn run(
    dialect: &str,
    current_state: &EditorState,
    previous_state: &Option<EditorState>,
) -> (IntegrationResponse, EditorState) {
    let mut options = dialect_options(dialect);
    options.cursor_line = Some(current_state.cursor[0]);
    options.cursor_x = Some(current_state.cursor[1]);

    if let Some(EditorState {
        lines,
        cursor: [prev_cursor_line, prev_cursor_x],
    }) = previous_state
    {
        options.prev_text = Some(lines.join("\n"));
        options.prev_cursor_line = Some(*prev_cursor_line);
        options.prev_cursor_x = Some(*prev_cursor_x);
    }
    let request = types::Request {
        mode: "smart".into(),
        text: current_state.lines.join("\n"),
        options,
    };
    let answer = parinfer::process(&request);

    let lines = answer
        .text
        .split('\n')
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let cursor = answer
        .cursor_line
        .zip(answer.cursor_x)
        .map(Into::into)
        .unwrap();
    let response = IntegrationResponse {
        changes: position::diff_ranges(&current_state.lines, &lines).map(
            |(
                Range { start, end },
                Range {
                    start: start2,
                    end: end2,
                },
            )| { [start, end.saturating_sub(1), start2, end2.saturating_sub(1)] },
        ),
        tab_stops: answer.tab_stops.iter().map(Into::into).collect(),
        paren_trails: answer.paren_trails.iter().map(Into::into).collect(),
        error: answer.error.as_ref().map(IntegrationError::new),
    };
    (response, EditorState { lines, cursor })
}

pub enum Direction {
    Forward,
    Backward,
}
impl mlua::FromLua for Direction {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        match value.as_i32() {
            Some(i) if i < 0 => Ok(Self::Backward),
            _ => Ok(Self::Forward),
        }
    }
}

pub fn shift_indent(tab_stops: &[[usize; 4]], dx: &Direction, x: usize) -> usize {
    let mut tabs: Vec<usize> =
        tab_stops
            .iter()
            .fold(vec![], |mut tabs, &[_line_no, x, ch, arg_x]| {
                tabs.push(x);
                tabs.push(x + if ch == 0x28 { 2 } else { 1 });
                if arg_x != 0 {
                    tabs.push(arg_x);
                }
                tabs
            });

    tabs.sort_unstable();
    tabs.dedup();

    match dx {
        Direction::Forward => tabs
            .into_iter()
            .find(|&i| i > x)
            .unwrap_or_else(|| x.saturating_add(2)),
        Direction::Backward => tabs
            .into_iter()
            .rfind(|&i| i < x)
            .unwrap_or_else(|| x.saturating_sub(2)),
    }
}

#[test]
fn test_shift_indent() {
    // 0 1 2 3 4 10
    let tab_stops = [[1, 0, 0x28, 4], [1, 1, 0x28, 2], [1, 2, 0x28, 10]];
    assert_eq!(shift_indent(&tab_stops, &Direction::Forward, 0), 1);
    assert_eq!(shift_indent(&tab_stops, &Direction::Backward, 0), 0);
    assert_eq!(shift_indent(&tab_stops, &Direction::Backward, 20), 10);
}

pub struct IntegrationResponse {
    tab_stops: Vec<[usize; 4]>,
    paren_trails: Vec<[usize; 3]>,
    error: Option<IntegrationError>,
    changes: Option<[usize; 4]>,
}

impl From<&types::TabStop<'_>> for [usize; 4] {
    fn from(val: &types::TabStop<'_>) -> Self {
        [
            val.line_no,
            val.x,
            val.ch.bytes().next().unwrap_or(0x28).into(),
            val.arg_x.unwrap_or(0),
        ]
    }
}
impl From<&types::ParenTrail> for [usize; 3] {
    fn from(val: &types::ParenTrail) -> Self {
        [val.line_no, val.start_x, val.end_x]
    }
}

impl mlua::IntoLua for IntegrationResponse {
    fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        let info = lua.create_table()?;
        info.set("tab_stops", self.tab_stops)?;
        info.set("paren_trails", self.paren_trails)?;
        info.set("error", self.error)?;
        info.set("changes", self.changes)?;
        Ok(mlua::Value::Table(info))
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
        Self {
            name: format!("{}", value.name),
            message: value.message.clone(),
            col: value.x,
            row: value.line_no,
        }
    }
}

impl mlua::IntoLua for IntegrationError {
    fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        let table = lua.create_table()?;
        table.set("name", self.name)?;
        table.set("message", self.message)?;
        table.set("col", self.col)?;
        table.set("row", self.row)?;
        Ok(mlua::Value::Table(table))
    }
}

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

#[test]
fn test_run() {
    macro_rules! string_split {
        ($text:expr) => {
            $text
                .split('\n')
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
        };
    }
    macro_rules! editor_state {
        ($text:expr, $cursor:expr) => {
            EditorState {
                lines: string_split!($text),
                cursor: $cursor,
            }
        };
    }
    let dialect = "clojure";
    let editor_history = [
        // current_state                expected_state
        (editor_state!("(", [0, 1]), editor_state!("()", [0, 1])),
        (
            editor_state!("(\n)", [1, 0]),
            editor_state!("(\n )", [1, 1]),
        ),
        (
            editor_state!("(\n [)", [1, 2]),
            editor_state!("(\n [])", [1, 2]),
        ),
        (
            editor_state!("(\n [{])", [1, 3]),
            editor_state!("(\n [{}])", [1, 3]),
        ),
        (
            editor_state!("(\n [{\n}])", [2, 0]),
            editor_state!("(\n [{\n   }])", [2, 3]),
        ),
    ];
    // previous state is the result of the previous tuple's expected state
    // which should match the result of running over the previous tuple's current_state
    for (i, (current_state, expected_state)) in editor_history.iter().enumerate() {
        let previous_state = editor_history
            .get(i.saturating_sub(1))
            .map(|(_, a)| a.clone());
        let (_, new_state) = run(dialect, current_state, &previous_state);
        assert_eq!(*expected_state, new_state);
    }
}
