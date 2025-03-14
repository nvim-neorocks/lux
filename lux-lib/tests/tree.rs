use lux_lib::config::{ConfigBuilder, LuaVersion};
use mlua::{IntoLua, Lua};
use tempdir::TempDir;

#[test]
fn tree_userdata() {
    let temp = TempDir::new("tree-userdata").unwrap();

    let lua = Lua::new();
    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(temp.into_path()))
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
