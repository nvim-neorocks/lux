use assert_fs::TempDir;
use lux_lib::{
    config::{ConfigBuilder, LuaVersion},
    lua_installation::get_installed_lua_version,
    lua_rockspec::{GitSource, RockSourceSpec},
    operations::{Install, PackageInstallSpec},
    tree::EntryType,
};

#[tokio::test]
async fn install_git_package() {
    let dir = TempDir::new().unwrap();
    let install_spec =
        PackageInstallSpec::new("rustaceanvim@6.0.3".parse().unwrap(), EntryType::Entrypoint)
            .source(RockSourceSpec::Git(GitSource {
                url: "https://github.com/mrcjkb/rustaceanvim.git"
                    .parse()
                    .unwrap(),
                checkout_ref: Some("v6.0.3".into()),
            }))
            .build();

    let lua_version = get_installed_lua_version("lua")
        .ok()
        .and_then(|version| LuaVersion::from_version(version).ok())
        .or(Some(LuaVersion::Lua51));

    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(dir.to_path_buf()))
        .lua_version(lua_version)
        .build()
        .unwrap();

    let tree = config.tree(LuaVersion::from(&config).unwrap()).unwrap();
    let installed = Install::new(&tree, &config)
        .package(install_spec)
        .install()
        .await
        .unwrap();
    assert_eq!(installed.len(), 1);
}
