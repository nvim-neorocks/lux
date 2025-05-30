use std::path::PathBuf;

use assert_fs::prelude::PathCopy;
use lux_lib::{
    config::{ConfigBuilder, LuaVersion},
    lua_installation::detect_installed_lua_version,
    operations::Test,
    project::Project,
};

#[tokio::test]
async fn run_busted_test() {
    let project_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test/sample-project-busted");
    let temp_dir = assert_fs::TempDir::new().unwrap();
    temp_dir.copy_from(&project_root, &["**"]).unwrap();
    let project_root = temp_dir.path();
    let project: Project = Project::from(project_root).unwrap().unwrap();
    let tree_root = project.root().to_path_buf().join(".lux");
    let _ = std::fs::remove_dir_all(&tree_root);

    let lua_version = detect_installed_lua_version().or(Some(LuaVersion::Lua51));

    let config = ConfigBuilder::new()
        .unwrap()
        .user_tree(Some(tree_root))
        .lua_version(lua_version)
        .build()
        .unwrap();

    Test::new(project, &config).run().await.unwrap();
}

#[tokio::test]
async fn run_busted_test_no_lock() {
    let project_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test/sample-project-busted");
    let temp_dir = assert_fs::TempDir::new().unwrap();
    temp_dir.copy_from(&project_root, &["**"]).unwrap();
    let project_root = temp_dir.path();
    let project: Project = Project::from(project_root).unwrap().unwrap();
    let tree_root = project.root().to_path_buf().join(".lux");
    let _ = std::fs::remove_dir_all(&tree_root);

    let lua_version = detect_installed_lua_version().or(Some(LuaVersion::Lua51));

    let config = ConfigBuilder::new()
        .unwrap()
        .user_tree(Some(tree_root))
        .lua_version(lua_version)
        .build()
        .unwrap();

    Test::new(project, &config)
        .no_lock(true)
        .run()
        .await
        .unwrap();
}
