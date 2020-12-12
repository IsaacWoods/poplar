use std::{future::Future, io, path::PathBuf, process::ExitStatus, string::ToString};
use tokio::process::Command;

pub struct RunCargo {
    pub manifest_path: PathBuf,
    pub target: Option<String>,
    pub release: bool,
    pub std_components: Vec<String>,
}

impl RunCargo {
    pub fn build(self) -> impl Future<Output = io::Result<ExitStatus>> {
        let mut args = Vec::new();
        if self.release {
            args.push("--release".to_string());
        }
        if let Some(target) = self.target {
            args.push("--target".to_string());
            args.push(target);
        }
        if self.std_components.len() != 0 {
            args.push(format!("-Zbuild-std={}", self.std_components.join(",")));
        }

        Command::new("cargo").arg("build").arg("--manifest-path").arg(self.manifest_path).args(args).status()
    }
}
