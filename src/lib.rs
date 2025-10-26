mod changes;
mod conversion;
mod dialect;
mod integration;
mod parinfer;
mod types;

#[mlua::lua_module]
fn parinfer_lib(lua: &mlua::Lua) -> mlua::Result<mlua::Table> {
    let exports = lua.create_table()?;
    exports.set("run", lua.create_function(integration::parinfer_run)?)?;
    exports.set(
        "shift_indent",
        lua.create_function(integration::parinfer_shift_indent)?,
    )?;
    exports.set(
        "to_charpos",
        lua.create_function(|lua, (line, bytepos): (String, usize)| {
            lua.pack(conversion::bytepos_to_charpos(&line, bytepos))
        })?,
    )?;
    exports.set(
        "to_bytepos",
        lua.create_function(|lua, (line, bytepos): (String, usize)| {
            lua.pack(conversion::charpos_to_bytepos(&line, bytepos))
        })?,
    )?;
    Ok(exports)
}
