use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
};

use bon::Builder;

use crate::{
    build::external_dependency::ExternalDependencyInfo,
    config::Config,
    lua_installation::LuaInstallation,
    progress::{Progress, ProgressBar},
    tree::{RockLayout, Tree},
};

#[derive(Builder)]
#[builder(start_fn(name = "new"))]
pub struct RunBuildArgs<'a> {
    pub(crate) output_paths: &'a RockLayout,
    pub(crate) no_install: bool,
    pub(crate) lua: &'a LuaInstallation,
    pub(crate) external_dependencies: &'a HashMap<String, ExternalDependencyInfo>,
    pub(crate) config: &'a Config,
    pub(crate) tree: &'a Tree,
    pub(crate) build_dir: &'a Path,
    pub(crate) progress: &'a Progress<ProgressBar>,
}

pub trait BuildBackend {
    type Err: std::error::Error;

    fn run(
        self,
        args: RunBuildArgs<'_>,
    ) -> impl Future<Output = Result<BuildInfo, Self::Err>> + Send;
}

#[derive(Default)]
pub struct BuildInfo {
    pub binaries: Vec<PathBuf>,
}
