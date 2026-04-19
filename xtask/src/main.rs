use eyre::{Result, eyre};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Command};

xflags::xflags! {
    cmd app {
        cmd build {}
        cmd run {}
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QemuConfig {
    ovmf: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    qemu: QemuConfig,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let flags = match App::from_env() {
        Ok(flags) => flags,
        Err(err) => {
            return Err(eyre!("Invalid configuration: {}", err));
        }
    };
    let config: Config = toml::from_str(&std::fs::read_to_string("Poplar.toml")?)?;

    match flags.subcommand {
        AppCmd::Build(_build) => build()?,

        AppCmd::Run(_run) => {
            build()?;

            let mut qemu = Command::new("qemu-system-x86_64");
            qemu.arg("-enable-kvm");
            qemu.args(&["-machine", "q35"]);
            qemu.args(&["-cpu", "max,vmware-cpuid-freq,invtsc"]);
            qemu.args(&["-debugcon", "stdio"]);
            qemu.arg("--no-reboot");
            qemu.arg("--no-shutdown");

            // Firmware
            let ovmf_code = config.qemu.ovmf.join("code.fd");
            let ovmf_vars = config.qemu.ovmf.join("vars.fd");
            qemu.args(&[
                "-drive",
                &format!(
                    "if=pflash,format=raw,readonly=on,file={}",
                    ovmf_code.display()
                ),
            ]);
            qemu.args(&[
                "-drive",
                &format!(
                    "if=pflash,format=raw,readonly=on,file={}",
                    ovmf_vars.display()
                ),
            ]);

            // Emulate `build` as a FAT filesystem
            qemu.args(&["-drive", "format=raw,file=fat:rw:build"]);
            qemu.status()?;
        }
    }

    Ok(())
}

fn build() -> Result<()> {
    let loader = BuildComponent {
        artifact_name: "loader.efi".into(),
        manifest_path: PathBuf::from("kernel/loader/"),
        workspace: Some(PathBuf::from("kernel/")),
        target: "x86_64-unknown-uefi".into(),
        release: true,
        build_std_components: true,
        ..Default::default()
    }
    .build()?;
    let kernel = BuildComponent {
        artifact_name: "kernel".into(),
        manifest_path: PathBuf::from("kernel/"),
        workspace: Some(PathBuf::from("kernel/")),
        target: "x86_64-unknown-none".into(),
        release: true,
        rustflags: Some("-Crelocation-model=static".into()),
        build_std_components: true,
        ..Default::default()
    }
    .build()?;

    std::fs::create_dir_all("build/efi/boot/")?;
    std::fs::copy(loader, "build/efi/boot/bootx64.efi")?;
    std::fs::copy(kernel, "build/kernel.elf")?;

    Ok(())
}

#[derive(Clone, Default, Debug)]
struct BuildComponent {
    artifact_name: String,
    manifest_path: PathBuf,
    workspace: Option<PathBuf>,
    target: String,
    release: bool,
    rustflags: Option<String>,
    features: Vec<String>,
    build_std_components: bool,
}

impl BuildComponent {
    /// Build a component, returning the path at which the artifact can be found
    fn build(self) -> Result<PathBuf> {
        let mut cargo = Command::new("cargo");
        cargo.arg("build");

        cargo
            .arg("--manifest-path")
            .arg(self.manifest_path.join("Cargo.toml"));
        cargo.arg("--target").arg(self.target.clone());
        if self.release {
            cargo.arg("--release");
        }
        if let Some(ref rustflags) = self.rustflags {
            cargo.env("RUSTFLAGS", rustflags);
        }
        if self.features.len() > 0 {
            cargo.arg("--features");
            cargo.arg(self.features.join(","));
        }
        if self.build_std_components {
            cargo.arg("-Zbuild-std=core,alloc");
            cargo.arg("-Zbuild-std-features=compiler-builtins-mem");
        }

        cargo
            .status()?
            .success()
            .then_some(())
            .ok_or(eyre!("Failed to build component: {:?}", self.manifest_path))?;

        // TODO: this will not work for things built with the host target
        let artifact_path = if let Some(workspace) = self.workspace {
            workspace
                .join("target")
                .join(self.target)
                .join(if self.release { "release" } else { "debug" })
                .join(self.artifact_name)
        } else {
            self.manifest_path
                .join("target")
                .join(self.target)
                .join(if self.release { "release" } else { "debug" })
                .join(self.artifact_name)
        };

        Ok(artifact_path)
    }
}
