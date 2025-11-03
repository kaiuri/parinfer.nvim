mod changes;
mod conversion;
mod dialect;
mod neovim;
mod parinfer;
mod types;

#[mlua::lua_module(name = "parinfer")]
fn parinfer_lib(lua: &mlua::Lua) -> mlua::Result<mlua::Table> {
    let exports = lua.create_table()?;
    exports.set("refresh", lua.create_function(neovim::refresh)?)?;
    exports.set("init", lua.create_function(neovim::init)?)?;
    exports.set("indent", lua.create_function(neovim::indent)?)?;
    exports.set("decorations", lua.create_function(neovim::decorations)?)?;
    exports.set("suggestions", lua.create_function(neovim::suggestions)?)?;
    Ok(exports)
}
