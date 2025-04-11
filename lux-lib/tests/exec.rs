#[cfg(not(target_env = "msvc"))]
use lux_lib::{
    config::ConfigBuilder,
    operations::{install_command, Exec},
};
#[cfg(not(target_env = "msvc"))]
use tempdir::TempDir;

#[cfg(not(target_env = "msvc"))]
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
