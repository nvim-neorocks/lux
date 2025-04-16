use assert_fs::TempDir;
use lux_lib::{
    config::{ConfigBuilder, LuaVersion},
    lua_installation::get_installed_lua_version,
};
use mlua::{IntoLua, Lua};

#[test]
fn tree_userdata() {
    let temp = TempDir::new().unwrap();

    let lua = Lua::new();

    let lua_version = get_installed_lua_version("lua")
        .ok()
        .and_then(|version| LuaVersion::from_version(version).ok())
        .or(Some(LuaVersion::Lua51));

    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(temp.to_path_buf()))
        .lua_version(lua_version)
        .build()
        .unwrap();
    let t = config.tree(LuaVersion::Lua51).unwrap();
    let tree = t.into_lua(&lua).unwrap();
    lua.globals().set("tree", tree).unwrap();

    lua.load(
        r#"
        print(tree:bin())
    "#,
    )
    .exec()
    .unwrap();
}
