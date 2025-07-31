use std::{collections::HashSet, path::PathBuf};

use itertools::Itertools;
use lux_lib::project::Project;
use walkdir::WalkDir;

pub fn top_level_ignored_files(project: &Project) -> Vec<PathBuf> {
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

    top_level_files
        .difference(&top_level_project_files)
        .cloned()
        .collect_vec()
}
