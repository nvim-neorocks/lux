use eyre::{OptionExt, Result};
use lux_lib::{
    config::Config,
    package::PackageReq,
    progress::{MultiProgress, Progress, ProgressBar},
    project::Project,
    remote_package_db::RemotePackageDB,
    rockspec::lua_dependency::{self},
};

use crate::utils::project::{
    sync_build_dependencies_if_locked, sync_dependencies_if_locked,
    sync_test_dependencies_if_locked,
};

#[derive(clap::Args)]
pub struct Add {
    /// Package or list of packages to install.
    package_req: Vec<PackageReq>,

    /// Reinstall without prompt if a package is already installed.
    #[arg(long)]
    force: bool,

    /// Install the package as a development dependency.
    /// Also called `dev`.
    #[arg(short, long, alias = "dev", visible_short_aliases = ['d', 'b'])]
    build: Option<Vec<PackageReq>>,

    /// Install the package as a test dependency.
    #[arg(short, long)]
    test: Option<Vec<PackageReq>>,
}

pub async fn add(data: Add, config: Config) -> Result<()> {
    let mut project = Project::current()?.ok_or_eyre("No project found")?;

    let db = RemotePackageDB::from_config(&config, &Progress::Progress(ProgressBar::new())).await?;

    let progress = MultiProgress::new_arc();

    if !data.package_req.is_empty() {
        project
            .add(
                lua_dependency::DependencyType::Regular(data.package_req),
                &db,
            )
            .await?;
        sync_dependencies_if_locked(&project, progress.clone(), &config).await?;
    }

    let build_packages = data.build.unwrap_or_default();
    if !build_packages.is_empty() {
        project
            .add(lua_dependency::DependencyType::Build(build_packages), &db)
            .await?;
        sync_build_dependencies_if_locked(&project, progress.clone(), &config).await?;
    }

    let test_packages = data.test.unwrap_or_default();
    if !test_packages.is_empty() {
        project
            .add(lua_dependency::DependencyType::Test(test_packages), &db)
            .await?;
        sync_test_dependencies_if_locked(&project, progress.clone(), &config).await?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use assert_fs::{prelude::PathCopy, TempDir};
    use lux_lib::config::ConfigBuilder;
    use serial_test::serial;

    use super::*;
    use std::path::PathBuf;

    #[serial]
    #[tokio::test]
    async fn test_add_regular_dependencies() {
        if std::env::var("LUX_SKIP_IMPURE_TESTS").unwrap_or("0".into()) == "1" {
            println!("Skipping impure test");
            return;
        }
        let sample_project: PathBuf = "resources/test/sample-project-init/".into();
        let project_root = TempDir::new().unwrap();
        project_root.copy_from(&sample_project, &["**"]).unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_root).unwrap();
        let config = ConfigBuilder::new().unwrap().build().unwrap();
        let args = Add {
            package_req: vec!["penlight@1.5".parse().unwrap()],
            force: false,
            build: Option::None,
            test: Option::None,
        };
        add(args, config.clone()).await.unwrap();
        let lockfile_path = project_root.join("lux.lock");
        let lockfile_content =
            String::from_utf8(tokio::fs::read(&lockfile_path).await.unwrap()).unwrap();
        assert!(lockfile_content.contains("penlight"));
        assert!(lockfile_content.contains("luafilesystem")); // dependency

        let args = Add {
            package_req: vec!["md5".parse().unwrap()],
            force: false,
            build: Option::None,
            test: Option::None,
        };
        add(args, config.clone()).await.unwrap();
        let lockfile_path = project_root.join("lux.lock");
        let lockfile_content =
            String::from_utf8(tokio::fs::read(&lockfile_path).await.unwrap()).unwrap();
        assert!(lockfile_content.contains("penlight"));
        assert!(lockfile_content.contains("luafilesystem"));
        assert!(lockfile_content.contains("md5"));

        std::env::set_current_dir(&cwd).unwrap();
    }

    #[serial]
    #[tokio::test]
    async fn test_add_build_dependencies() {
        if std::env::var("LUX_SKIP_IMPURE_TESTS").unwrap_or("0".into()) == "1" {
            println!("Skipping impure test");
            return;
        }
        let sample_project: PathBuf = "resources/test/sample-project-init/".into();
        let project_root = TempDir::new().unwrap();
        project_root.copy_from(&sample_project, &["**"]).unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_root).unwrap();
        let config = ConfigBuilder::new().unwrap().build().unwrap();
        let args = Add {
            package_req: Vec::new(),
            force: false,
            build: Option::Some(vec!["penlight@1.5".parse().unwrap()]),
            test: Option::None,
        };
        add(args, config.clone()).await.unwrap();
        let lockfile_path = project_root.join("lux.lock");
        let lockfile_content =
            String::from_utf8(tokio::fs::read(&lockfile_path).await.unwrap()).unwrap();
        assert!(lockfile_content.contains("penlight"));
        assert!(lockfile_content.contains("luafilesystem")); // dependency

        let args = Add {
            package_req: Vec::new(),
            force: false,
            build: Option::Some(vec!["md5".parse().unwrap()]),
            test: Option::None,
        };
        add(args, config.clone()).await.unwrap();
        let lockfile_path = project_root.join("lux.lock");
        let lockfile_content =
            String::from_utf8(tokio::fs::read(&lockfile_path).await.unwrap()).unwrap();
        assert!(lockfile_content.contains("penlight"));
        assert!(lockfile_content.contains("luafilesystem"));
        assert!(lockfile_content.contains("md5"));

        std::env::set_current_dir(&cwd).unwrap();
    }

    #[serial]
    #[tokio::test]
    async fn test_add_test_dependencies() {
        if std::env::var("LUX_SKIP_IMPURE_TESTS").unwrap_or("0".into()) == "1" {
            println!("Skipping impure test");
            return;
        }
        let sample_project: PathBuf = "resources/test/sample-project-init/".into();
        let project_root = TempDir::new().unwrap();
        project_root.copy_from(&sample_project, &["**"]).unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_root).unwrap();
        let config = ConfigBuilder::new().unwrap().build().unwrap();
        let args = Add {
            package_req: Vec::new(),
            force: false,
            build: Option::None,
            test: Option::Some(vec!["penlight@1.5".parse().unwrap()]),
        };
        add(args, config.clone()).await.unwrap();
        let lockfile_path = project_root.join("lux.lock");
        let lockfile_content =
            String::from_utf8(tokio::fs::read(&lockfile_path).await.unwrap()).unwrap();
        assert!(lockfile_content.contains("penlight"));
        assert!(lockfile_content.contains("luafilesystem")); // dependency

        let args = Add {
            package_req: Vec::new(),
            force: false,
            build: Option::None,
            test: Option::Some(vec!["md5".parse().unwrap()]),
        };
        add(args, config.clone()).await.unwrap();
        let lockfile_path = project_root.join("lux.lock");
        let lockfile_content =
            String::from_utf8(tokio::fs::read(&lockfile_path).await.unwrap()).unwrap();
        assert!(lockfile_content.contains("penlight"));
        assert!(lockfile_content.contains("luafilesystem"));
        assert!(lockfile_content.contains("md5"));

        std::env::set_current_dir(&cwd).unwrap();
    }
}
