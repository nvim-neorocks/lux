use eyre::Result;
use inquire::Confirm;
use lux_lib::{
    config::{Config, LuaVersion},
    progress::{MultiProgress, ProgressBar},
};

/// Purge the user tree
pub async fn purge(config: Config) -> Result<()> {
    let tree = config.user_tree(LuaVersion::from(&config)?.clone())?;

    let len = tree.list()?.len();

    if Confirm::new(&format!("Are you sure you want to purge all {len} rocks?"))
        .with_default(false)
        .prompt()?
    {
        let root_dir = tree.root();

        let _spinner = MultiProgress::new().add(ProgressBar::from(format!(
            "🗑️ Purging {}",
            root_dir.display()
        )));
        std::fs::remove_dir_all(tree.root())?;
    }

    Ok(())
}
