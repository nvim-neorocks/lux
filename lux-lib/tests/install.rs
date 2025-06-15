use assert_fs::TempDir;
use lux_lib::{
    config::{ConfigBuilder, LuaVersion},
    git::GitSource,
    lua_installation::detect_installed_lua_version,
    lua_rockspec::RockSourceSpec,
    operations::{Install, PackageInstallSpec},
    tree::EntryType,
};

#[tokio::test]
async fn install_git_package() {
    let install_spec =
        PackageInstallSpec::new("rustaceanvim@6.0.3".parse().unwrap(), EntryType::Entrypoint)
            .source(RockSourceSpec::Git(GitSource {
                url: "https://github.com/mrcjkb/rustaceanvim.git"
                    .parse()
                    .unwrap(),
                checkout_ref: Some("v6.0.3".into()),
            }))
            .build();
    test_install(install_spec).await
}

// http 0.4 has an http-0.4-0.all.rock packed rock on luarocks.org
#[tokio::test]
#[cfg(not(target_env = "msvc"))] // http has dependencies that are not supported on Windows
async fn install_http_package() {
    let install_spec =
        PackageInstallSpec::new("http@0.4-0".parse().unwrap(), EntryType::Entrypoint).build();
    test_install(install_spec).await
}

async fn test_install(install_spec: PackageInstallSpec) {
    let dir = TempDir::new().unwrap();
    let lua_version = detect_installed_lua_version().or(Some(LuaVersion::Lua51));

    let config = ConfigBuilder::new()
        .unwrap()
        .user_tree(Some(dir.to_path_buf()))
        .lua_version(lua_version)
        .build()
        .unwrap();

    let tree = config
        .user_tree(LuaVersion::from_config(&config).unwrap().clone())
        .unwrap();
    let installed = Install::new(&config)
        .package(install_spec)
        .tree(tree)
        .install()
        .await
        .unwrap();
    assert!(!installed.is_empty());
}
