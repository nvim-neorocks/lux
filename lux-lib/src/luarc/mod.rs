use crate::project::Project;
use crate::project::ProjectRoot;

pub fn update_luarc() -> Result<(), ()> {
    let project = Project::current_or_err().expect("failed to get current project");
    let luarc_path = project.luarc_path();
    let file = generate_luarc(project.root());

    std::fs::write(&luarc_path, file)
        .expect(format!("failed to write {} file", luarc_path.display()).as_str());
    Ok(())
}

fn generate_luarc(project_root: &ProjectRoot) -> String {
    let mut content = String::new();
    content.push_str("{");
    content.push_str("\n    \"workspace.library\": []\n");
    content.push_str("}");
    content
}

#[cfg(test)]
mod test {

    use crate::project::ProjectRoot;

    #[test]
    fn test_generate_luarc() {
        let content = super::generate_luarc(&ProjectRoot::new());
        assert_eq!(
            content,
            r#"{
    "workspace.library": []
}"#
        );
    }
}
