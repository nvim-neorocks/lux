#[cfg(not(target_env = "msvc"))]
use lux_lib::{
    config::ConfigBuilder,
    operations::{install_command, Exec},
};
#[cfg(not(target_env = "msvc"))]
use tempdir::TempDir;

#[cfg(not(target_env = "msvc"))]
#[tokio::test]
async fn run_nlua() {
    use lux_lib::{config::LuaVersion, lua_installation::get_installed_lua_version};

    let dir = TempDir::new("lux-test").unwrap();

    let lua_version = get_installed_lua_version("lua")
        .ok()
        .and_then(|version| LuaVersion::from_version(version).ok())
        .or(Some(LuaVersion::Lua51));

    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(dir.into_path()))
        .lua_version(lua_version)
        .build()
        .unwrap();
    install_command("nlua", &config).await.unwrap();
    Exec::new("nlua", None, &config)
        .arg("-v")
        .exec()
        .await
        .unwrap();
}
