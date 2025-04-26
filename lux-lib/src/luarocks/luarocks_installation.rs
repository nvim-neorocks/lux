use futures::future::join_all;
use itertools::Itertools;
use path_slash::{PathBufExt, PathExt};
use ssri::Integrity;
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::Arc,
};
use tempdir::TempDir;
use thiserror::Error;

use crate::{
    build::{Build, BuildError},
    config::{Config, LuaVersion, LuaVersionUnset},
    lockfile::{LocalPackage, LocalPackageId},
    lua_installation::LuaInstallation,
    lua_rockspec::RockspecFormat,
    operations::{get_all_dependencies, PackageInstallSpec, SearchAndDownloadError, UnpackError},
    path::{Paths, PathsError},
    progress::{MultiProgress, Progress, ProgressBar},
    remote_package_db::{RemotePackageDB, RemotePackageDBError},
    rockspec::Rockspec,
    tree::{self, Tree, TreeError},
};

#[cfg(target_family = "unix")]
const LUAROCKS_EXE: &str = "luarocks";
#[cfg(target_family = "windows")]
const LUAROCKS_EXE: &str = "luarocks.exe";

pub(crate) const LUAROCKS_VERSION: &str = "3.11.1-1";

#[cfg(target_family = "unix")]
const LUAROCKS_ROCKSPEC: &str = "
rockspec_format = '3.0'
package = 'luarocks'
version = '3.11.1-1'
source = {
    url = 'git+https://github.com/luarocks/luarocks',
    tag = 'v3.11.1',
}
build = {
    type = 'builtin',
}
";

#[derive(Error, Debug)]
pub enum LuaRocksError {
    #[error(transparent)]
    LuaVersionUnset(#[from] LuaVersionUnset),
    // #[error(transparent)]
    // Io(#[from] io::Error),
    #[error(transparent)]
    Tree(#[from] TreeError),
}

#[derive(Error, Debug)]
pub enum LuaRocksInstallError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Tree(#[from] TreeError),
    #[error(transparent)]
    BuildError(#[from] BuildError),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    UnpackError(#[from] UnpackError),
    #[error("luarocks integrity mismatch.\nExpected: {expected}\nBut got: {got}")]
    IntegrityMismatch { expected: Integrity, got: Integrity },
}

#[derive(Error, Debug)]
pub enum InstallBuildDependenciesError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Tree(#[from] TreeError),
    #[error(transparent)]
    RemotePackageDBError(#[from] RemotePackageDBError),
    #[error(transparent)]
    SearchAndDownloadError(#[from] SearchAndDownloadError),
    #[error(transparent)]
    BuildError(#[from] BuildError),
}

#[derive(Error, Debug)]
pub enum ExecLuaRocksError {
    #[error(transparent)]
    LuaVersionUnset(#[from] LuaVersionUnset),
    #[error("could not write luarocks config: {0}")]
    WriteLuarocksConfigError(io::Error),
    #[error("failed to run luarocks: {0}")]
    Io(#[from] io::Error),
    #[error("error setting up luarocks paths: {0}")]
    Paths(#[from] PathsError),
    #[error("luarocks binary not found at {0}")]
    LuarocksBinNotFound(PathBuf),
    #[error("executing luarocks compatibility layer failed.\nstatus: {status}\nstdout: {stdout}\nstderr: {stderr}")]
    CommandFailure {
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
}

pub struct LuaRocksInstallation {
    tree: Tree,
    config: Config,
}

impl LuaRocksInstallation {
    pub fn new(config: &Config) -> Result<Self, LuaRocksError> {
        let config = config.clone().with_tree(config.luarocks_tree().clone());
        let luarocks_installation = Self {
            tree: config.tree(LuaVersion::from(&config)?)?,
            config,
        };
        Ok(luarocks_installation)
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    #[cfg(target_family = "unix")]
    pub async fn ensure_installed(
        &self,
        progress: &Progress<ProgressBar>,
    ) -> Result<(), LuaRocksInstallError> {
        use crate::{lua_rockspec::RemoteLuaRockspec, package::PackageReq};

        let mut lockfile = self.tree.lockfile()?.write_guard();

        let luarocks_req =
            PackageReq::new("luarocks".into(), Some(LUAROCKS_VERSION.into())).unwrap();

        if !self.tree.match_rocks(&luarocks_req)?.is_found() {
            let rockspec = RemoteLuaRockspec::new(LUAROCKS_ROCKSPEC).unwrap();
            let pkg = Build::new(
                &rockspec,
                &self.tree,
                tree::EntryType::Entrypoint,
                &self.config,
                progress,
            )
            .constraint(luarocks_req.version_req().clone().into())
            .build()
            .await?;
            lockfile.add_entrypoint(&pkg);
        }
        Ok(())
    }

    #[cfg(target_family = "windows")]
    pub async fn ensure_installed(
        &self,
        progress: &Progress<ProgressBar>,
    ) -> Result<(), LuaRocksInstallError> {
        use crate::{hash::HasIntegrity, operations};
        use std::io::Cursor;
        let url = "https://luarocks.github.io/luarocks/releases/luarocks-3.11.1-windows-64.zip";
        let response = reqwest::get(url.to_owned()).await?.bytes().await?;
        let hash = response.hash()?;
        let expected_hash: Integrity = "sha256-xx26PQPhIwXpzNAixiHIhpq6PRJNkkniFK7VwW82gqM="
            .parse()
            .unwrap();
        if expected_hash.matches(&hash).is_none() {
            return Err(LuaRocksInstallError::IntegrityMismatch {
                expected: expected_hash,
                got: hash,
            });
        }
        let cursor = Cursor::new(response);
        let mime_type = infer::get(cursor.get_ref()).map(|file_type| file_type.mime_type());
        let unpack_dir = TempDir::new("luarocks-exe")?.into_path();
        operations::unpack(
            mime_type,
            cursor,
            false,
            "luarocks-3.11.1-windows-64.zip".into(),
            &unpack_dir,
            progress,
        )
        .await?;
        let luarocks_exe = unpack_dir
            .join("luarocks-3.11.1-windows-64")
            .join(LUAROCKS_EXE);
        tokio::fs::copy(luarocks_exe, &self.tree().bin().join(LUAROCKS_EXE)).await?;

        Ok(())
    }

    pub async fn install_build_dependencies<R: Rockspec>(
        &self,
        build_backend: &str,
        rocks: &R,
        progress_arc: Arc<Progress<MultiProgress>>,
    ) -> Result<(), InstallBuildDependenciesError> {
        let progress = Arc::clone(&progress_arc);
        let mut lockfile = self.tree.lockfile()?.write_guard();
        let bar = progress.map(|p| p.new_bar());
        let package_db = RemotePackageDB::from_config(&self.config, &bar).await?;
        bar.map(|b| b.finish_and_clear());
        let build_dependencies = match rocks.format() {
            Some(RockspecFormat::_1_0 | RockspecFormat::_2_0) => {
                // XXX: rockspec formats < 3.0 don't support `build_dependencies`,
                // so we have to fetch the build backend from the dependencies.
                rocks
                    .dependencies()
                    .current_platform()
                    .iter()
                    .filter(|dep| dep.name().to_string().contains(build_backend))
                    .cloned()
                    .collect_vec()
            }
            _ => rocks.build_dependencies().current_platform().to_vec(),
        }
        .into_iter()
        .map(|dep| PackageInstallSpec::new(dep.package_req, tree::EntryType::Entrypoint).build())
        .collect_vec();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        get_all_dependencies(
            tx,
            build_dependencies,
            Arc::new(package_db),
            Arc::new(lockfile.clone()),
            &self.config,
            progress_arc,
        )
        .await?;

        let mut all_packages = HashMap::with_capacity(rx.len());
        while let Some(dep) = rx.recv().await {
            all_packages.insert(dep.spec.id(), dep);
        }

        let installed_packages = join_all(all_packages.clone().into_values().map(|install_spec| {
            let bar = progress.map(|p| {
                p.add(ProgressBar::from(format!(
                    "💻 Installing build dependency: {}",
                    install_spec.downloaded_rock.rockspec().package(),
                )))
            });
            let config = self.config.clone();
            let tree = self.tree.clone();
            tokio::spawn(async move {
                let rockspec = install_spec.downloaded_rock.rockspec();
                let pkg = Build::new(rockspec, &tree, tree::EntryType::Entrypoint, &config, &bar)
                    .constraint(install_spec.spec.constraint())
                    .behaviour(install_spec.build_behaviour)
                    .build()
                    .await?;

                bar.map(|b| b.finish_and_clear());

                Ok::<_, InstallBuildDependenciesError>((pkg.id(), (pkg, install_spec.entry_type)))
            })
        }))
        .await
        .into_iter()
        .flatten()
        .try_collect::<_, HashMap<LocalPackageId, (LocalPackage, tree::EntryType)>, _>()?;

        installed_packages
            .iter()
            .for_each(|(id, (pkg, entry_type))| {
                if *entry_type == tree::EntryType::Entrypoint {
                    lockfile.add_entrypoint(pkg);
                }

                all_packages
                    .get(id)
                    .map(|pkg| pkg.spec.dependencies())
                    .unwrap_or_default()
                    .into_iter()
                    .for_each(|dependency_id| {
                        lockfile.add_dependency(
                            pkg,
                            &installed_packages
                                .get(dependency_id)
                                .expect("required dependency not found")
                                .0,
                        );
                    });
            });

        Ok(())
    }

    pub fn make(
        self,
        rockspec_path: &Path,
        build_dir: &Path,
        dest_dir: &Path,
        lua: &LuaInstallation,
    ) -> Result<(), ExecLuaRocksError> {
        std::fs::create_dir_all(dest_dir)?;
        let dest_dir_str = dest_dir.to_slash_lossy().to_string();
        let rockspec_path_str = rockspec_path.to_slash_lossy().to_string();
        let args = vec![
            "make",
            "--deps-mode",
            "none",
            "--tree",
            &dest_dir_str,
            &rockspec_path_str,
        ];
        self.exec(args, build_dir, lua)
    }

    fn exec(
        self,
        args: Vec<&str>,
        cwd: &Path,
        lua: &LuaInstallation,
    ) -> Result<(), ExecLuaRocksError> {
        let luarocks_paths = Paths::new(&self.tree)?;
        // Ensure a pure environment so we can do parallel builds
        let temp_dir = TempDir::new("lux-run-luarocks").unwrap();
        let lua_version_str = match lua.version {
            LuaVersion::Lua51 | LuaVersion::LuaJIT => "5.1",
            LuaVersion::Lua52 | LuaVersion::LuaJIT52 => "5.2",
            LuaVersion::Lua53 => "5.3",
            LuaVersion::Lua54 => "5.4",
        };
        let luarocks_config_content = format!(
            r#"
lua_version = "{0}"
variables = {{
    LUA_LIBDIR = "{1}",
    LUA_INCDIR = "{2}",
    LUA_VERSION = "{3}",
    MAKE = "{4}",
}}
"#,
            lua_version_str,
            lua.lib_dir.to_slash_lossy(),
            lua.include_dir.to_slash_lossy(),
            LuaVersion::from(&self.config)?,
            self.config.make_cmd(),
        );
        let luarocks_config = temp_dir.path().join("luarocks-config.lua");
        std::fs::write(luarocks_config.clone(), luarocks_config_content)
            .map_err(ExecLuaRocksError::WriteLuarocksConfigError)?;
        let luarocks_bin = self.tree().bin().join(LUAROCKS_EXE);
        if !luarocks_bin.is_file() {
            return Err(ExecLuaRocksError::LuarocksBinNotFound(luarocks_bin));
        }
        let output = Command::new(luarocks_bin)
            .current_dir(cwd)
            .args(args)
            .env("PATH", luarocks_paths.path_prepended().joined())
            .env("LUA_PATH", luarocks_paths.package_path().joined())
            .env("LUA_CPATH", luarocks_paths.package_cpath().joined())
            .env("HOME", temp_dir.into_path().to_slash_lossy().to_string())
            .env(
                "LUAROCKS_CONFIG",
                luarocks_config.to_slash_lossy().to_string(),
            )
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ExecLuaRocksError::CommandFailure {
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into(),
                stderr: String::from_utf8_lossy(&output.stderr).into(),
            })
        }
    }
}
