use std::collections::HashSet;

use clap::Args;
use eyre::Result;
use itertools::Itertools;
use lux_lib::{
    config::Config,
    operations::{Exec, Install, PackageInstallSpec},
    progress::MultiProgress,
    project::Project,
    tree,
};
use path_slash::PathBufExt;
use walkdir::WalkDir;

#[derive(Args)]
pub struct Check {
    /// Arguments to pass to the luacheck command.{n}
    /// If you pass arguments to luacheck, Lux will not pass any default arguments.
    check_args: Option<Vec<String>>,
    /// By default, Lux will add top-level ignored files and directories{n}
    /// (like those in .gitignore) to luacheck's exclude files.{n}
    /// This flag disables that behaviour.{n}
    #[arg(long)]
    no_ignore: bool,
}

pub async fn check(check: Check, config: Config) -> Result<()> {
    let project = Project::current_or_err()?;

    let luacheck =
        PackageInstallSpec::new("luacheck".parse()?, tree::EntryType::Entrypoint).build();

    Install::new(&config)
        .package(luacheck)
        .project(&project)?
        .progress(MultiProgress::new_arc())
        .install()
        .await?;

    let check_args: Vec<String> = match check.check_args {
        Some(args) => args,
        None if check.no_ignore => Vec::new(),
        None => {
            let top_level_project_files = ignore::WalkBuilder::new(project.root())
                .max_depth(Some(1))
                .build()
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let file = entry.into_path();
                    if file.is_dir() || file.extension().is_some_and(|ext| ext == "lua") {
                        Some(file)
                    } else {
                        None
                    }
                })
                .collect::<HashSet<_>>();

            let top_level_files = WalkDir::new(project.root())
                .max_depth(1)
                .into_iter()
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let file = entry.into_path();
                    if file.is_dir() || file.extension().is_some_and(|ext| ext == "lua") {
                        Some(file)
                    } else {
                        None
                    }
                })
                .collect::<HashSet<_>>();

            let ignored_files = top_level_files
                .difference(&top_level_project_files)
                .map(|file| file.to_slash_lossy().to_string());

            std::iter::once("--exclude-files".into())
                .chain(ignored_files)
                .collect_vec()
        }
    };

    Exec::new("luacheck", Some(&project), &config)
        .arg(project.root().to_slash_lossy())
        .args(check_args)
        .exec()
        .await?;

    Ok(())
}
