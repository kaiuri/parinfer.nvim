mod changes;
mod position;
mod integration;
mod parinfer;
mod types;

#[mlua::lua_module(name = "parinfer")]
fn parinfer_lib(lua: &mlua::Lua) -> mlua::Result<mlua::Table> {
    let exports = lua.create_table()?;
    exports.set(
        "run",
        lua.create_function(
            |lua: &mlua::Lua,
             (dialect, current_state, previous_state): (
                String,
                integration::EditorState,
                Option<integration::EditorState>,
            )| {
                lua.pack_multi(integration::run(&dialect, &current_state, &previous_state))
            },
        )?,
    )?;
    exports.set(
        "shift_indent",
        lua.create_function(
            |lua: &mlua::Lua,
             (tab_stops, dx, x): (Vec<[usize; 4]>, integration::Direction, usize)| {
                lua.pack(integration::shift_indent(&tab_stops, &dx, x))
            },
        )?,
    )?;
    exports.set(
        "to_charpos",
        lua.create_function(|lua, (line, bytepos): (String, usize)| {
            lua.pack(position::bytepos_to_charpos(&line, bytepos))
        })?,
    )?;
    exports.set(
        "to_bytepos",
        lua.create_function(|lua, (line, bytepos): (String, usize)| {
            lua.pack(position::charpos_to_bytepos(&line, bytepos))
        })?,
    )?;
    Ok(exports)
}
