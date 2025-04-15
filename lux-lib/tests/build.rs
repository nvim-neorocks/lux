use std::path::PathBuf;

use assert_fs::prelude::PathCopy;
use lux_lib::{
    build::{Build, BuildBehaviour::Force},
    config::{ConfigBuilder, LuaVersion},
    lua_rockspec::RemoteLuaRockspec,
    progress::{MultiProgress, Progress},
    project::Project,
    tree,
};
use tempdir::TempDir;
use tokio::runtime::Builder;

#[tokio::test]
async fn builtin_build() {
    let dir = TempDir::new("lux-test").unwrap();

    let content =
        String::from_utf8(std::fs::read("resources/test/lua-cjson-2.1.0-1.rockspec").unwrap())
            .unwrap();
    let rockspec = RemoteLuaRockspec::new(&content).unwrap();

    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(dir.into_path()))
        .build()
        .unwrap();

    let progress = MultiProgress::new();
    let bar = progress.new_bar();

    let tree = config.tree(LuaVersion::from(&config).unwrap()).unwrap();

    Build::new(
        &rockspec,
        &tree,
        tree::EntryType::Entrypoint,
        &config,
        &Progress::Progress(bar),
    )
    .behaviour(Force)
    .build()
    .await
    .unwrap();
}

#[tokio::test]
async fn make_build() {
    let dir = TempDir::new("lux-test").unwrap();

    let content = String::from_utf8(
        std::fs::read("resources/test/make-project/make-project-scm-1.rockspec").unwrap(),
    )
    .unwrap();
    let rockspec = RemoteLuaRockspec::new(&content).unwrap();

    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(dir.into_path()))
        .build()
        .unwrap();

    let progress = MultiProgress::new();
    let bar = progress.new_bar();

    let tree = config.tree(LuaVersion::from(&config).unwrap()).unwrap();

    Build::new(
        &rockspec,
        &tree,
        tree::EntryType::Entrypoint,
        &config,
        &Progress::Progress(bar),
    )
    .behaviour(Force)
    .build()
    .await
    .unwrap();
}

#[tokio::test]
async fn cmake_build() {
    test_build_rockspec("resources/test/luv-1.48.0-2.rockspec".into()).await
}

#[tokio::test]
async fn command_build() {
    // The rockspec appears to be broken when using luajit headers on macos
    let config = ConfigBuilder::new().unwrap().build().unwrap();
    if cfg!(target_os = "macos") && config.lua_version() == Some(&LuaVersion::LuaJIT) {
        println!("luaposix is broken on macos/luajit! Skipping...");
        return;
    }
    test_build_rockspec("resources/test/luaposix-35.1-1.rockspec".into()).await
}

async fn test_build_rockspec(rockspec_path: PathBuf) {
    let dir = TempDir::new("lux-test").unwrap();

    let content = String::from_utf8(std::fs::read(rockspec_path).unwrap()).unwrap();
    let rockspec = RemoteLuaRockspec::new(&content).unwrap();

    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(dir.into_path()))
        .build()
        .unwrap();

    let progress = MultiProgress::new();
    let bar = progress.new_bar();

    let tree = config.tree(LuaVersion::from(&config).unwrap()).unwrap();

    Build::new(
        &rockspec,
        &tree,
        tree::EntryType::Entrypoint,
        &config,
        &Progress::Progress(bar),
    )
    .behaviour(Force)
    .build()
    .await
    .unwrap();
}

#[tokio::test]
async fn treesitter_parser_build() {
    let dir = TempDir::new("lux-test").unwrap();

    let content = String::from_utf8(
        std::fs::read("resources/test/tree-sitter-rust-0.0.43.rockspec").unwrap(),
    )
    .unwrap();
    let rockspec = RemoteLuaRockspec::new(&content).unwrap();

    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(dir.into_path()))
        .build()
        .unwrap();

    let progress = MultiProgress::new();
    let bar = progress.new_bar();

    let tree = config.tree(LuaVersion::from(&config).unwrap()).unwrap();

    Build::new(
        &rockspec,
        &tree,
        tree::EntryType::Entrypoint,
        &config,
        &Progress::Progress(bar),
    )
    .behaviour(Force)
    .build()
    .await
    .unwrap();
}

#[tokio::test]
async fn test_build_local_project_no_source() {
    let sample_project: PathBuf = "resources/test/sample-project-no-source/".into();
    let project_root = assert_fs::TempDir::new().unwrap();
    project_root.copy_from(&sample_project, &["**"]).unwrap();

    let project = Project::from(&project_root).unwrap().unwrap();
    let project_toml = project.toml().into_local().unwrap();
    let config = ConfigBuilder::new().unwrap().build().unwrap();
    let tree = project.tree(&config).unwrap();
    let progress = MultiProgress::new();
    let bar = progress.new_bar();

    Build::new(
        &project_toml,
        &tree,
        tree::EntryType::Entrypoint,
        &config,
        &Progress::Progress(bar),
    )
    .behaviour(Force)
    .build()
    .await
    .unwrap();
}

#[tokio::test]
async fn test_build_local_project_only_src() {
    let sample_project: PathBuf = "resources/test/sample-project-only-src/".into();
    let project_root = assert_fs::TempDir::new().unwrap();
    project_root.copy_from(&sample_project, &["**"]).unwrap();

    let project = Project::from(&project_root).unwrap().unwrap();
    let project_toml = project.toml().into_local().unwrap();
    let config = ConfigBuilder::new().unwrap().build().unwrap();
    let tree = project.tree(&config).unwrap();
    let progress = MultiProgress::new();
    let bar = progress.new_bar();

    let pkg = Build::new(
        &project_toml,
        &tree,
        tree::EntryType::Entrypoint,
        &config,
        &Progress::Progress(bar),
    )
    .behaviour(Force)
    .build()
    .await
    .unwrap();

    let layout = tree.installed_rock_layout(&pkg).unwrap();
    assert!(layout.src.is_dir());
    assert!(layout.src.join("main.lua").is_file());
    assert!(layout.src.join("foo.lua").is_file());
}

#[test]
fn test_build_multiple_treesitter_parsers() {
    let dir = TempDir::new("lux-test").unwrap();

    let content = String::from_utf8(
        std::fs::read("resources/test/tree-sitter-rust-0.0.43.rockspec").unwrap(),
    )
    .unwrap();
    let rockspec = RemoteLuaRockspec::new(&content).unwrap();

    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(dir.into_path()))
        .build()
        .unwrap();

    let tree = config.tree(LuaVersion::from(&config).unwrap()).unwrap();

    let runtime = Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut handles = vec![];

    for _ in 0..4 {
        let config = config.clone();
        let tree = tree.clone();
        let rockspec = rockspec.clone();

        handles.push(runtime.spawn(async move {
            Build::new(
                &rockspec,
                &tree,
                tree::EntryType::Entrypoint,
                &config,
                &Progress::NoProgress,
            )
            .behaviour(Force)
            .build()
            .await
            .unwrap()
        }));
    }

    runtime.block_on(futures::future::join_all(handles));
}
