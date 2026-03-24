use std::ops::Range;

use crate::conversion;
use crate::parinfer;
use crate::types;

macro_rules! runtime_error {
    ($msg:expr) => {
        Err(mlua::Error::RuntimeError($msg.into()))
    };
}

pub fn parinfer_shift_indent(
    lua: &mlua::Lua,
    (tab_stops, dx, x): (Vec<IntegrationTabStop>, Direction, usize),
) -> mlua::Result<mlua::Value> {
    lua.pack(shift_indent(&tab_stops, &dx, x))
}

// Input is a table with the following keys:
//   [1] string (dialect)
//   [2..3] (string[], [number, number]) (current state)
//   [4..5] (string[], [number, number])? (previous state)
// Will shift 2. and 3. to 4. and 5. respectively and set 2. and 3. to a new
// state IF there's one, else, keeps it unchanged it avoids needlessly creating
// new tables this function does not check if cursors are valid given their lines.
// Returns { tab_stops: [number;4], paren_trails: [number;3], error: { name: string, message: string, row: number, col: number } }
// and, if there's any difference between the new state and the previous state, a 4-element array describing their difference (See `diff_slice` for what that means).
pub fn parinfer_run(lua: &mlua::Lua, value: mlua::Value) -> mlua::Result<mlua::MultiValue> {
    let Some(table) = value.as_table() else {
        return runtime_error!("expected table");
    };
    let convert_state = |(lines, [lnum, bytepos]): State| {
        let row = lnum - 1;
        let charpos = conversion::bytepos_to_charpos(&lines[row], bytepos);
        (lines, [row, charpos])
    };
    let dialect: String = table.get(1)?;
    // it's an error to provide one and not the other
    let current_state = table
        .get::<Vec<String>>(2)
        .ok()
        .zip(table.get::<Cursor>(3).ok())
        .map(convert_state)
        .map(Ok)
        .unwrap_or(runtime_error!(
            "invalid state, current_lines (2) or current_cursor (3) is missing"
        ))?;
    // previous lines and cursor, must be both present or both absent
    // anything else is ignored since it's invalid
    let previous_state = table
        .get::<Vec<String>>(4)
        .ok()
        .zip(table.get::<Cursor>(5).ok())
        .map(convert_state);
    let (response, (new_lines, [new_row, new_charpos])) =
        run(&dialect, &current_state, &previous_state);
    let changes = conversion::diff_ranges(&current_state.0, &new_lines).map(
        |(
            Range { start, end },
            Range { start: start2, end: end2 },
        )| {
            [ start, end.saturating_sub(1), start2, end2.saturating_sub(1) ]
        },
    );

    // current_state is now also the previous_state
    table.set(4, table.get::<mlua::Value>(2)?)?;
    table.set(5, table.get::<mlua::Value>(3)?)?;
    let new_bytepos = conversion::charpos_to_bytepos(&new_lines[new_row], new_charpos);
    table.set(3, [new_row + 1, new_bytepos])?;
    table.set(2, new_lines)?;
    lua.pack_multi((response, changes))
}

// 0: lines, 1: cursor
type State = (Vec<String>, Cursor);

fn run(
    dialect: &str,
    (lines, [cursor_line, cursor_x]): &State,
    previous_state: &Option<State>,
) -> (IntegrationResponse, State) {
    let mut options = crate::dialect::dialect_options(dialect);
    options.cursor_line = Some(*cursor_line);
    options.cursor_x = Some(*cursor_x);

    if let Some((prev_lines, [prev_cursor_line, prev_cursor_x])) = previous_state {
        options.prev_text = Some(prev_lines.join("\n"));
        options.prev_cursor_line = Some(*prev_cursor_line);
        options.prev_cursor_x = Some(*prev_cursor_x);
    }
    let request = types::Request {
        mode: "smart".into(),
        text: lines.join("\n"),
        options,
    };
    let answer = parinfer::process(&request);
    let new_lines = answer
        .text
        .split('\n')
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let cursor = answer.cursor_line.zip(answer.cursor_x).unwrap().into();
    let response = IntegrationResponse::new(&answer);
    (response, (new_lines, cursor))
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
fn shift_indent(tab_stops: &[IntegrationTabStop], dx: &Direction, x: usize) -> usize {
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

// ----------------------------------------------
// collection of structs that we can mlua easily
type Cursor = [usize; 2];
type IntegrationParenTrail = [usize; 3];
type IntegrationTabStop = [usize; 4];
struct IntegrationResponse {
    tab_stops: Vec<IntegrationTabStop>,
    // vector of [line_no, start_x, end_x], for each paren_trail i.e. `closers`
    paren_trails: Vec<IntegrationParenTrail>,
    error: Option<IntegrationError>,
}

impl IntegrationResponse {
    fn new(value: &types::Answer<'_>) -> Self {
        Self {
            tab_stops: value
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
                .collect(),
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
    fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        let info = lua.create_table()?;
        info.set("tab_stops", self.tab_stops)?;
        info.set("paren_trails", self.paren_trails)?;
        info.set("error", self.error)?;
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
            (string_split!($text), $cursor)
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
