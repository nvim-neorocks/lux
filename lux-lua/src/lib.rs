use std::path::PathBuf;

use lux_lib::{config::ConfigBuilder, project::Project};
use mlua::prelude::*;

mod loader;

fn config(lua: &Lua) -> mlua::Result<LuaTable> {
    let table = lua.create_table()?;

    table.set(
        "default",
        lua.create_function(|_, ()| ConfigBuilder::default().build().into_lua_err())?,
    )?;

    Ok(table)
}

fn project(lua: &Lua) -> mlua::Result<LuaTable> {
    let table = lua.create_table()?;

    table.set(
        "current",
        lua.create_function(|_, ()| Project::current().into_lua_err())?,
    )?;

    table.set(
        "new",
        lua.create_function(|_, path: PathBuf| Project::from(path).into_lua_err())?,
    )?;

    Ok(table)
}

#[mlua::lua_module]
fn lux(lua: &Lua) -> LuaResult<LuaTable> {
    #[cfg(not(any(
        feature = "lua51",
        feature = "lua52",
        feature = "lua53",
        feature = "lua54"
    )))]
    compile_error!(
        "
        At least one Lua version feature must be enabled. \
        Please enable one of the following features: \
        lua51, lua52, lua53, lua54."
    );

    let exports = lua.create_table()?;

    exports.set(
        "loader",
        lua.create_function(|lua, ()| loader::load_loader(lua))?,
    )?;
    exports.set("config", config(lua)?)?;
    exports.set("project", project(lua)?)?;

    Ok(exports)
}
