use std::collections::HashMap;

use mlua::UserData;

use crate::merge::Merge;

#[derive(Debug, PartialEq, Clone)]
pub struct CMakeBuildSpec {
    pub cmake_lists_content: Option<String>,
    /// Whether to perform a build pass.
    /// Default is true.
    pub build_pass: bool,
    /// Whether to perform an install pass.
    /// Default is true.
    pub install_pass: bool,
    pub variables: HashMap<String, String>,
}

impl Merge for CMakeBuildSpec {
    fn merge(self, other: Self) -> Self {
        Self {
            cmake_lists_content: other.cmake_lists_content.or(self.cmake_lists_content),
            build_pass: other.build_pass,
            install_pass: other.install_pass,
            variables: self.variables.into_iter().chain(other.variables).collect(),
        }
    }
}

impl Default for CMakeBuildSpec {
    fn default() -> Self {
        Self {
            cmake_lists_content: Default::default(),
            build_pass: default_pass(),
            install_pass: default_pass(),
            variables: Default::default(),
        }
    }
}

impl UserData for CMakeBuildSpec {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("cmake_lists_content", |_, this, _: ()| {
            Ok(this.cmake_lists_content.clone())
        });
        methods.add_method("build_pass", |_, this, _: ()| Ok(this.build_pass));
        methods.add_method("install_pass", |_, this, _: ()| Ok(this.install_pass));
        methods.add_method("variables", |_, this, _: ()| Ok(this.variables.clone()));
    }
}

fn default_pass() -> bool {
    true
}
