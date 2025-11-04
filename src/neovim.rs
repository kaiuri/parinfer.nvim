#![allow(dead_code)]
use crate::{conversion, dialect, parinfer, types};

macro_rules! split {
    ($text:expr) => {
        $text
            .split('\n')
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
    };
}

type Cursor = [usize; 2];
type Lines = Vec<String>;
type Language = String;
struct TabStop {
    line_no: usize,
    x: usize,
    ch: u8,
    arg_x: Option<usize>,
}
impl From<&types::TabStop<'_>> for TabStop {
    fn from(value: &types::TabStop<'_>) -> Self {
        Self {
            line_no: value.line_no,
            x: value.x,
            ch: value.ch.bytes().next().unwrap_or(0x28),
            arg_x: value.arg_x,
        }
    }
}

pub struct EditorState {
    language: String,
    lines: Lines,
    cursor_line: usize,
    cursor_x: usize,
    tab_stops: Vec<TabStop>,
    paren_trails: Vec<types::ParenTrail>,
    changes: Option<[usize; 4]>,
    error: Option<types::Error>,
}

impl mlua::UserData for EditorState {}

pub fn suggestions(
    lua: &mlua::Lua,
    res: mlua::UserDataRef<EditorState>,
) -> mlua::Result<mlua::Value> {
    if let Some([insert_start, insert_end, range_start, range_end]) = res.changes {
        let lnum = res.cursor_line + 1;
        let bytepos = conversion::charpos_to_bytepos(&res.lines[res.cursor_line], res.cursor_x);
        let lines = &res.lines[range_start..=range_end];
        let t = lua.create_table()?;
        t.set("lines", lines.to_vec())?;
        t.set("cursor", [lnum, bytepos])?;
        t.set("insert_start", insert_start)?;
        t.set("insert_end", insert_end)?;
        lua.pack(t)
    } else {
        lua.pack(&mlua::Nil)
    }
}

pub fn indent(
    lua: &mlua::Lua,
    (res, dedent): (mlua::UserDataRef<EditorState>, Option<bool>),
) -> mlua::Result<mlua::MultiValue> {
    let indent = &res.lines[res.cursor_line]
        .bytes()
        .take_while(|&b| b == b' ')
        .count();
    let mut tabs = res.tab_stops.iter().fold(vec![], |mut tabs, tab_stop| {
        tabs.push(tab_stop.x);
        tabs.push(tab_stop.x + if tab_stop.ch == 0x28 { 2 } else { 1 });
        if let Some(arg_x) = tab_stop.arg_x {
            tabs.push(arg_x);
        }
        tabs
    });
    tabs.dedup();
    tabs.sort_unstable();

    let newindent = if dedent.is_some_and(|d| d) {
        tabs.into_iter()
            .rfind(|i| i < indent)
            .unwrap_or_else(|| indent.saturating_sub(2))
    } else {
        tabs.into_iter()
            .find(|i| i > indent)
            .unwrap_or_else(|| indent.saturating_add(2))
    };
    let mut new_line = String::from(&res.lines[res.cursor_line]);
    new_line.replace_range(0..*indent, &" ".repeat(newindent));
    lua.pack_multi((new_line, [res.cursor_line + 1, newindent]))
}

pub fn decorations(
    lua: &mlua::Lua,
    (res, toprow, botrow): (mlua::UserDataRef<EditorState>, usize, usize),
) -> mlua::Result<mlua::Value> {
    let out = res
        .paren_trails
        .iter()
        .filter(|t| t.line_no >= toprow && t.line_no <= botrow)
        .map(|t| {
            let line = &res.lines[t.line_no];
            let start_x = conversion::charpos_to_bytepos(line, t.start_x);
            let end_x = conversion::charpos_to_bytepos(line, t.end_x);
            [t.line_no, start_x, end_x]
        })
        .collect::<Vec<_>>();
    lua.pack(out)
}

pub fn init(
    lua: &mlua::Lua,
    (language, lines1, [lnum, bytepos]): (Language, Lines, Cursor),
) -> mlua::Result<mlua::Value> {
    let cursor_line = lnum - 1;
    let cursor_x = conversion::bytepos_to_charpos(&lines1[cursor_line], bytepos);
    let mut options = dialect::dialect_options(&language);
    options.cursor_line = Some(cursor_line);
    options.cursor_x = Some(cursor_x);
    let request = types::Request {
        text: lines1.join("\n"),
        options,
        mode: "smart".into(),
    };
    let answer = parinfer::process(&request);
    let lines = split!(answer.text);
    let changes = conversion::diff_slice(&lines1, &lines);
    lua.pack(EditorState {
        language,
        lines,
        changes,
        cursor_x: answer.cursor_x.unwrap(),
        cursor_line: answer.cursor_line.unwrap(),
        tab_stops: answer.tab_stops.iter().map(Into::into).collect(),
        paren_trails: answer.paren_trails,
        error: answer.error,
    })
}

pub fn refresh(
    _lua: &mlua::Lua,
    (mut res, lines1, [lnum, bytepos]): (mlua::UserDataRefMut<EditorState>, Lines, Cursor),
) -> mlua::Result<()> {
    let mut options = dialect::dialect_options(&res.language);
    options.prev_cursor_line = Some(res.cursor_line);
    options.prev_cursor_x = Some(res.cursor_x);
    options.prev_text = Some(res.lines.join("\n"));
    let row = lnum - 1;
    let charpos = conversion::bytepos_to_charpos(&lines1[row], bytepos);
    options.cursor_line = Some(row);
    options.cursor_x = Some(charpos);
    let request = types::Request {
        text: lines1.join("\n"),
        options,
        mode: "smart".into(),
    };
    let answer = parinfer::process(&request);
    let lines = split!(answer.text);
    let changes = conversion::diff_slice(&lines1, &lines);
    res.lines = lines;
    res.changes = changes;
    res.cursor_x = answer.cursor_x.unwrap();
    res.cursor_line = answer.cursor_line.unwrap();
    res.tab_stops = answer.tab_stops.iter().map(Into::into).collect();
    res.paren_trails = answer.paren_trails;
    res.error = answer.error;
    Ok(())
}
