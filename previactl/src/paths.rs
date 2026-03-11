use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct PreviaPaths {
    pub home: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StackPaths {
    pub name: String,
    pub runtime_file: PathBuf,
}

impl PreviaPaths {
    pub fn discover() -> Result<Self> {
        let home = match env::var("PREVIA_HOME") {
            Ok(value) => absolutize(PathBuf::from(value))?,
            Err(_) => {
                let user_home = env::var("HOME").context("HOME is not set and PREVIA_HOME is unset")?;
                absolutize(PathBuf::from(user_home).join(".previa"))?
            }
        };

        Ok(Self {
            home,
        })
    }

    pub fn stack(&self, name: &str) -> StackPaths {
        let root = self.home.join("stacks").join(name);
        let run_dir = root.join("run");
        StackPaths {
            name: name.to_owned(),
            runtime_file: run_dir.join("state.json"),
        }
    }

    pub fn stack_roots(&self) -> Result<Vec<StackPaths>> {
        let stacks_dir = self.home.join("stacks");
        if !stacks_dir.exists() {
            return Ok(Vec::new());
        }
        let mut stacks = Vec::new();
        for entry in std::fs::read_dir(&stacks_dir)
            .with_context(|| format!("failed to read stacks directory '{}'", stacks_dir.display()))?
        {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().to_string_lossy().into_owned();
                stacks.push(self.stack(&name));
            }
        }
        Ok(stacks)
    }
}

fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(env::current_dir()
        .context("failed to read current directory")?
        .join(path))
}
