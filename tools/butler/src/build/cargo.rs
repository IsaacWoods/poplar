use super::BuildStep;
use eyre::{eyre, Result, WrapErr};
use std::{path::PathBuf, process::Command};

#[derive(Clone, Debug)]
pub enum Target {
    #[allow(dead_code)]
    Host,
    Triple(String),
    Custom {
        triple: String,
        spec: PathBuf,
    },
}

pub struct RunCargo {
    pub toolchain: Option<String>,
    pub manifest_path: PathBuf,
    // TODO: can we work this out ourselves?
    pub workspace: PathBuf,
    pub target: Target,
    pub release: bool,
    pub std_components: Vec<String>,
    pub std_features: Vec<String>,
    pub artifact_name: String,
    /// If this is not `None`, the result of the Cargo invocation will be copied to the given path.
    pub artifact_path: Option<PathBuf>,
}

impl BuildStep for RunCargo {
    fn build(self) -> Result<()> {
        // TODO: the rpi4 kernel passes `RUSTFLAGS="-Ctarget-cpu=cortex-a72". I'd like to think there's a better
        // way to do this than setting an environment variable, but we might want to add that as a capability if
        // not.

        /*
         * Lots of people use `std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())` to get the "real"
         * cargo in all cases. However, this doesn't let us specify a toolchain with `+toolchain`, so doesn't
         * really work for us.
         */
        let mut cargo = Command::new("cargo");

        if let Some(ref toolchain) = self.toolchain {
            cargo.arg(format!("+{}", toolchain));
        }

        cargo.arg("build");
        cargo.arg("--manifest-path").arg(&self.manifest_path);

        if self.release {
            cargo.arg("--release");
        }
        match self.target.clone() {
            Target::Host => (),
            Target::Triple(triple) => {
                cargo.arg("--target");
                cargo.arg(triple);
            }
            Target::Custom { triple: _triple, spec } => {
                cargo.arg("--target");
                // XXX: this assumes paths on the build platform are valid UTF-8
                cargo.arg(spec.to_str().unwrap());
            }
        }
        if self.std_components.len() != 0 {
            cargo.arg(format!("-Zbuild-std={}", self.std_components.join(",")));
        }
        if self.std_features.len() != 0 {
            cargo.arg(format!("-Zbuild-std-features={}", self.std_features.join(",")));
        }

        cargo
            .status()
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
            std::fs::copy(cargo_result_path.clone(), artifact_path.clone()).wrap_err_with(|| {
                format!("Failed to copy Cargo artifact from {:?} to {:?}", cargo_result_path, artifact_path)
            })?;
        }

        Ok(())
    }
}
