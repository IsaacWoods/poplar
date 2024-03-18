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

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Subcommand {
    Build,
    Doc,
}

pub struct RunCargo {
    pub artifact_name: String,
    /// The directory that contains the crate to be built. The manifest should be at `{manifest_dir}/Cargo.toml`.
    pub manifest_dir: PathBuf,
    pub subcommand: Subcommand,
    pub workspace: Option<PathBuf>,
    pub target: Target,
    pub release: bool,
    pub features: Vec<String>,
    pub std_components: Vec<String>,
    pub std_features: Vec<String>,
    pub toolchain: Option<String>,
    /// Any extra arguments that should be passed to Cargo.
    pub extra: Vec<String>,
    /// These are passed in the `RUSTFLAGS` environment variable
    pub rustflags: Option<String>,
    /// If `true`, the resulting artifact will be flattened into a flat binary and the path to that
    /// binary returned as the artifact. The artifact will be placed in Cargo's `target` directory
    /// with the same name as the original artifact, but with an extension of `bin`.
    pub flatten_result: bool,
}

impl RunCargo {
    pub fn new<S: Into<String>>(artifact_name: S, manifest_dir: PathBuf) -> RunCargo {
        RunCargo {
            artifact_name: artifact_name.into(),
            manifest_dir,
            subcommand: Subcommand::Build,
            workspace: None,
            target: Target::Host,
            release: false,
            features: vec![],
            std_components: vec![],
            std_features: vec![],
            toolchain: None,
            extra: vec![],
            rustflags: None,
            flatten_result: false,
        }
    }

    pub fn workspace(self, workspace: PathBuf) -> RunCargo {
        RunCargo { workspace: Some(workspace), ..self }
    }

    pub fn subcommand(self, subcommand: Subcommand) -> RunCargo {
        RunCargo { subcommand, ..self }
    }

    pub fn target(self, target: Target) -> RunCargo {
        RunCargo { target, ..self }
    }

    pub fn release(self, release: bool) -> RunCargo {
        RunCargo { release, ..self }
    }

    /// Pass the given crate features to Cargo. This is additive - multiple calls to this method
    /// result in all of the features being passed.
    pub fn features(mut self, mut features: Vec<String>) -> RunCargo {
        self.features.append(&mut features);
        self
    }

    pub fn std_components(self, std_components: Vec<String>) -> RunCargo {
        RunCargo { std_components, ..self }
    }

    pub fn std_features(self, std_features: Vec<String>) -> RunCargo {
        RunCargo { std_features, ..self }
    }

    pub fn toolchain<S: Into<String>>(self, toolchain: S) -> RunCargo {
        RunCargo { toolchain: Some(toolchain.into()), ..self }
    }

    /// Pass the given extra arguments to Cargo. This is additive.
    pub fn extra(mut self, mut extra: Vec<String>) -> RunCargo {
        self.extra.append(&mut extra);
        self
    }

    pub fn rustflags<S: Into<String>>(self, rustflags: S) -> RunCargo {
        RunCargo { rustflags: Some(rustflags.into()), ..self }
    }

    pub fn flatten_result(self, flatten_result: bool) -> RunCargo {
        RunCargo { flatten_result, ..self }
    }

    /// Run the Cargo invocation. Returns the path at which to find the built artifact.
    pub fn run(self) -> Result<PathBuf> {
        /*
         * Lots of people use `std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())` to get the "real"
         * cargo in all cases. However, this doesn't let us specify a toolchain with `+toolchain`, so doesn't
         * really work for us.
         */
        let mut cargo = Command::new("cargo");

        if let Some(ref toolchain) = self.toolchain {
            cargo.arg(format!("+{}", toolchain));
        }

        match self.subcommand {
            Subcommand::Build => cargo.arg("build"),
            Subcommand::Doc => cargo.arg("doc"),
        };

        cargo.arg("--manifest-path").arg(self.manifest_dir.join("Cargo.toml"));

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
        if self.features.len() != 0 {
            cargo.arg("--features");
            cargo.arg(self.features.join(","));
        }
        if self.std_components.len() != 0 {
            cargo.arg(format!("-Zbuild-std={}", self.std_components.join(",")));
        }
        if self.std_features.len() != 0 {
            cargo.arg(format!("-Zbuild-std-features={}", self.std_features.join(",")));
        }
        if self.extra.len() != 0 {
            cargo.args(&self.extra);
        }
        if let Some(ref rustflags) = self.rustflags {
            cargo.env("RUSTFLAGS", rustflags);
        }

        cargo
            .status()
            .wrap_err_with(|| format!("Failed to invoke cargo for crate at {:?}", self.manifest_dir))?
            .success()
            .then_some(())
            .ok_or(eyre!("Cargo invocation for crate {:?} failed", self.manifest_dir))?;

        let target_path = if let Some(workspace) = self.workspace {
            workspace.join("target").join(match self.target {
                Target::Host => todo!(),
                Target::Triple(triple) => triple,
                Target::Custom { triple, spec: _spec } => triple,
            })
        } else {
            self.manifest_dir.join("target").join(match self.target {
                Target::Host => todo!(),
                Target::Triple(triple) => triple,
                Target::Custom { triple, spec: _spec } => triple,
            })
        };

        match self.subcommand {
            Subcommand::Build => {
                let artifact_path =
                    target_path.join(if self.release { "release" } else { "debug" }).join(self.artifact_name);

                if self.flatten_result {
                    let binary_path = artifact_path.with_extension("bin");
                    // TODO: `cargo-binutils` does more complex logic to find this binary from the
                    // `llvm-tools` component. It's in our path for some reason, but that might not be true
                    // for everyone?
                    Command::new("llvm-objcopy")
                        .args(&["-O", "binary"])
                        .arg(&artifact_path)
                        .arg(&binary_path)
                        .status()?;
                    Ok(binary_path)
                } else {
                    Ok(artifact_path)
                }
            }
            Subcommand::Doc => Ok(target_path.join("doc")),
        }
    }
}
