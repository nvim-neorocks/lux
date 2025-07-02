use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
};

use crate::{
    build::external_dependency::ExternalDependencyInfo,
    config::Config,
    lua_installation::LuaInstallation,
    progress::{Progress, ProgressBar},
    tree::{RockLayout, Tree},
};

pub trait BuildBackend {
    type Err: std::error::Error;

    #[allow(clippy::too_many_arguments)]
    fn run(
        self,
        output_paths: &RockLayout,
        no_install: bool,
        lua: &LuaInstallation,
        external_dependencies: &HashMap<String, ExternalDependencyInfo>,
        config: &Config,
        tree: &Tree,
        build_dir: &Path,
        progress: &Progress<ProgressBar>,
    ) -> impl Future<Output = Result<BuildInfo, Self::Err>> + Send;
}

#[derive(Default)]
pub struct BuildInfo {
    pub binaries: Vec<PathBuf>,
}
