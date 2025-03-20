use lux_lib::{
    config::ConfigBuilder,
    operations::{install_command, Exec},
};
use tempdir::TempDir;

#[tokio::test]
async fn run_nlua() {
    let dir = TempDir::new("lux-test").unwrap();
    let config = ConfigBuilder::new()
        .unwrap()
        .tree(Some(dir.into_path()))
        .build()
        .unwrap();
    install_command("nlua", &config).await.unwrap();
    Exec::new("nlua", None, &config)
        .arg("-v")
        .exec()
        .await
        .unwrap();
}
