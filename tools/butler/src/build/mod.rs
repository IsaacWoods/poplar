pub mod cargo;
// pub mod image;

use async_trait::async_trait;
use eyre::{Result, WrapErr};
use std::path::PathBuf;

pub type BuildFuture = futures::future::BoxFuture<'static, Result<()>>;

#[async_trait]
pub trait BuildStep {
    async fn build(self) -> Result<()>;
}

pub struct MakeDirectories(pub PathBuf);

#[async_trait]
impl BuildStep for MakeDirectories {
    async fn build(self) -> Result<()> {
        tokio::fs::DirBuilder::new()
            .recursive(true)
            .create(self.0.clone())
            .await
            .wrap_err_with(|| format!("Failed to make directory tree at {:?}", self.0))
    }
}
