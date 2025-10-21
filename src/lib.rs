mod changes;
mod integration;
mod parinfer;
mod types;
use integration::*;

#[mlua::lua_module]
fn parinfer_lib(lua: &mlua::Lua) -> mlua::Result<mlua::Table> {
    let exports = lua.create_table()?;
    exports.set("run", lua.create_function(parinfer_run)?)?;
    exports.set("to_charpos", lua.create_function(parinfer_to_charpos)?)?;
    exports.set("to_bytepos", lua.create_function(parinfer_to_bytepos)?)?;
    exports.set("shift_indent", lua.create_function(parinfer_shift_indent)?)?;
    Ok(exports)
}
