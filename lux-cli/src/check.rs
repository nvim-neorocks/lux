use clap::{Args, ValueEnum};
use emmylua_check::OutputDestination;
use eyre::{eyre, Result};
use itertools::Itertools;
use lux_lib::{config::Config, project::Project};

use crate::project::top_level_ignored_files;

#[derive(Args)]
pub struct Check {
    /// Comma-separated list of ignore patterns.
    /// Patterns must follow glob syntax.
    /// Lux will automatically add top-level ignored project files.
    #[arg(short, long, value_delimiter = ',')]
    ignore: Option<Vec<String>>,

    /// The output format.
    #[arg(long, default_value = "text", value_enum, ignore_case = true)]
    output_format: OutputFormat,

    /// Output destination.{n}
    /// (stdout or a file path, only used when the output format is json).
    #[arg(long, default_value = "stdout")]
    output: OutputDestination,

    /// Treat warnings as errors.
    #[arg(long)]
    warnings_as_errors: bool,
}

#[derive(Debug, Clone, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
    Text,
}

impl From<OutputFormat> for emmylua_check::OutputFormat {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Json => emmylua_check::OutputFormat::Json,
            OutputFormat::Text => emmylua_check::OutputFormat::Text,
        }
    }
}

pub async fn check(args: Check, config: Config) -> Result<()> {
    let project = Project::current_or_err()?;
    let workspace = project.root().to_path_buf();
    let ignore = Some(
        args.ignore
            .unwrap_or_default()
            .into_iter()
            .chain(
                top_level_ignored_files(&project)
                    .iter()
                    .filter_map(|file| file.file_name().map(|file_name| (file_name, file)))
                    .map(|(file_name, file)| {
                        if file.is_dir() {
                            format!("{}/**/*", file_name.display())
                        } else {
                            file_name.to_string_lossy().to_string()
                        }
                    }),
            )
            .collect_vec(),
    );
    let emmylua_check_args = emmylua_check::CmdArgs {
        config: None,
        workspace,
        ignore,
        output_format: args.output_format.into(),
        output: args.output,
        warnings_as_errors: args.warnings_as_errors,
        verbose: config.verbose(),
    };
    emmylua_check::run_check(emmylua_check_args)
        .await
        .map_err(|err| eyre!(err.to_string()))?;
    Ok(())
}
