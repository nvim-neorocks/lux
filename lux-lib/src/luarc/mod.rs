pub fn update_luarc() -> Result<(), ()> {
    let file = generate_luarc();

    std::fs::write(".luarc.json", file).expect("failed to write .luarc.json file");
    Ok(())
}

fn generate_luarc() -> String {
    let mut content = String::new();
    content.push_str("{");
    content.push_str("\n    \"workspace.library\": []\n");
    content.push_str("}");
    content
}

#[cfg(test)]
mod test {

    #[test]
    fn test_generate_luarc() {
        let content = super::generate_luarc();
        assert_eq!(
            content,
            r#"{
    "workspace.library": []
}"#
        );
    }
}
