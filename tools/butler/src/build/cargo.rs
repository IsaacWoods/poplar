use super::BuildStep;
use async_trait::async_trait;
use eyre::{eyre, Result, WrapErr};
use std::{path::PathBuf, string::ToString};
use tokio::process::Command;

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
    async fn build(self) -> Result<()> {
        // TODO: the rpi4 kernel passes `RUSTFLAGS="-Ctarget-cpu=cortex-a72". I'd like to think there's a better
        // way to do this than setting an environment variable, but we might want to add that as a capability if
        // not.
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

        Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
            .arg("build")
            .arg("--manifest-path")
            .arg(self.manifest_path.clone())
            .args(args)
            .status()
            .await
            .wrap_err_with(|| format!("Failed to invoke cargo for crate at {:?}", self.manifest_path))?
            .success()
            .then_some(())
            .ok_or(eyre!("Cargo invocation for crate {:?} failed", self.manifest_path))?;

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
            tokio::fs::copy(cargo_result_path.clone(), artifact_path.clone()).await.wrap_err_with(|| {
                format!("Failed to copy Cargo artifact from {:?} to {:?}", cargo_result_path, artifact_path)
            })?;
        }

        Ok(())
    }
}
