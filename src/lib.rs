use ltrait::{
    Action, Source,
    color_eyre::eyre::Context as _,
    tokio_stream::{self, StreamExt as _},
};
use serde::Deserialize;
use std::{
    os::unix::process::CommandExt as _,
    path::PathBuf,
    process::{Command, Stdio},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Could not find config directory")]
    ConfigDir,
    #[error("failed to read task config file: {0}")]
    ReadTaskFile(#[source] std::io::Error),
    #[error("failed to parse the toml: {0}")]
    Toml(#[source] toml::de::Error),
}

#[derive(Debug, Clone)]
pub struct TaskConfig {
    /// The list of paths to load tasks
    pub path: Vec<PathBuf>,
}

/// The default path is ~/.config/yurf/task.toml
pub fn default_path() -> Result<PathBuf, TaskError> {
    Ok(dirs::config_dir()
        .ok_or(TaskError::ConfigDir)?
        .join("yurf")
        .join("task.toml"))
}

#[derive(Debug, Clone)]
pub struct Task {
    config: TaskConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TaskItem {
    pub name: String,
    /// show only if this command returns 0(exit code).
    /// this command will be executed  when source is evaled.
    ///
    /// the command will be executed as shell command("sh -c")
    pub show_if: Option<String>,
    /// the action will execute this command.
    ///
    /// the command will be executed as shell command("sh -c")
    pub command: String,
}

#[derive(Deserialize, Debug)]
struct TaskFile {
    task: Vec<TaskItem>,
}

impl Task {
    pub fn new(config: TaskConfig) -> Self {
        Self { config }
    }

    pub fn create_source(&self) -> Result<Source<TaskItem>, TaskError> {
        let tasks = self
            .config
            .path
            .clone()
            .into_iter()
            .map(|p| Ok(p))
            .map(|p| p.and_then(|p| std::fs::read_to_string(p).map_err(TaskError::ReadTaskFile)))
            .map(|p| p.and_then(|p| toml::from_str::<TaskFile>(&p).map_err(TaskError::Toml)))
            .map(|p| p.map(|t| t.task))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten();

        let aiter = tokio_stream::iter(tasks).filter(|c| {
            let cmd = c.show_if.as_ref();
            if let Some(cmd) = cmd {
                Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .process_group(0)
                    .output()
                    .map(|o| o.status.code() == Some(0))
                    .unwrap_or(false)
            } else {
                true
            }
        });

        Ok(Box::pin(aiter))
    }
}

impl Action for Task {
    type Context = TaskItem;

    fn act(&self, ctx: &Self::Context) -> ltrait::color_eyre::eyre::Result<()> {
        Command::new("sh")
            .arg("-c")
            .arg(&ctx.command)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .wrap_err("failed to start the selected app")?;

        Ok(())
    }
}
