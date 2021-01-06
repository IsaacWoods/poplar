pub mod cargo;
pub mod image;

use eyre::{Result, WrapErr};
use std::path::PathBuf;

pub trait BuildStep {
    fn build(self) -> Result<()>;
}

pub struct MakeDirectories(pub PathBuf);

impl BuildStep for MakeDirectories {
    fn build(self) -> Result<()> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(self.0.clone())
            .wrap_err_with(|| format!("Failed to make directory tree at {:?}", self.0))
    }
}
