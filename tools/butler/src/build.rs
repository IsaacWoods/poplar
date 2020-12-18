use async_trait::async_trait;
use std::{io, path::PathBuf, string::ToString};
use tokio::process::Command;

pub type BuildFuture = futures::future::BoxFuture<'static, Result<(), BuildError>>;

#[derive(Debug)]
pub enum BuildError {
    BuildFailed,
    Io(io::Error),
}

#[async_trait]
pub trait BuildStep {
    async fn build(self) -> Result<(), BuildError>;
}

#[derive(Clone, Debug)]
pub enum Target {
    Host,
    Triple(String),
    Custom { triple: String, spec: PathBuf },
}

pub struct RunCargo {
    pub manifest_path: PathBuf,
    pub target: Target,
    pub release: bool,
    pub std_components: Vec<String>,
}

#[async_trait]
impl BuildStep for RunCargo {
    async fn build(self) -> Result<(), BuildError> {
        let mut args = Vec::new();
        if self.release {
            args.push("--release".to_string());
        }
        match self.target.clone() {
            Target::Host => (),
            Target::Triple(triple) => {
                args.push("--target".to_string());
                args.push(triple);
            }
            Target::Custom { triple: _triple, spec } => {
                args.push("--target".to_string());
                // XXX: this assumes paths on the build platform are valid UTF-8
                args.push(spec.to_str().unwrap().to_string());
            }
        }
        if self.std_components.len() != 0 {
            args.push(format!("-Zbuild-std={}", self.std_components.join(",")));
        }

        match Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
            .arg("build")
            .arg("--manifest-path")
            .arg(self.manifest_path)
            .args(args)
            .status()
            .await
        {
            Ok(exit_status) => match exit_status.success() {
                true => Ok(()),
                false => Err(BuildError::BuildFailed),
            },
            Err(err) => Err(BuildError::Io(err)),
        }
    }
}
