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
    // TODO: can we work this out ourselves?
    pub workspace: PathBuf,
    pub target: Target,
    pub release: bool,
    pub std_components: Vec<String>,
    pub artifact_name: String,
    /// If this is not `None`, the result of the Cargo invocation will be copied to the given path.
    pub artifact_path: Option<PathBuf>,
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
                true => (),
                false => return Err(BuildError::BuildFailed),
            },
            Err(err) => return Err(BuildError::Io(err)),
        }

        if let Some(artifact_path) = self.artifact_path {
            let cargo_result_path = self
                .workspace
                .join("target")
                .join(match self.target {
                    Target::Host => todo!(),
                    Target::Triple(triple) => triple,
                    Target::Custom { triple, spec: _spec } => triple,
                })
                .join(if self.release { "release" } else { "debug" })
                .join(self.artifact_name);
            println!("Copying artifact from {:?} to {:?}", cargo_result_path, artifact_path);
            match tokio::fs::copy(cargo_result_path, artifact_path).await {
                Ok(_) => (),
                Err(err) => return Err(BuildError::Io(err)),
            }
        }

        Ok(())
    }
}

pub struct MakeDirectories(pub PathBuf);

#[async_trait]
impl BuildStep for MakeDirectories {
    async fn build(self) -> Result<(), BuildError> {
        tokio::fs::DirBuilder::new().recursive(true).create(self.0).await.map_err(|err| BuildError::Io(err))
    }
}
