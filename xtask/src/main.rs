use eyre::{Result, eyre};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

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
    let loader = build_component(
        "loader.efi",
        Path::new("kernel/loader/"),
        None,
        "x86_64-unknown-uefi",
        true,
    )?;

    std::fs::create_dir_all("build/efi/boot/")?;
    std::fs::copy(loader, "build/efi/boot/bootx64.efi")?;

    Ok(())
}

/// Build a component, returning the path at which the artifact can be found
fn build_component(
    artifact_name: &str,
    manifest_path: &Path,
    workspace: Option<&Path>,
    target: &str,
    release: bool,
) -> Result<PathBuf> {
    let mut cargo = Command::new("cargo");
    cargo.arg("build");
    cargo
        .arg("--manifest-path")
        .arg(manifest_path.join("Cargo.toml"));
    cargo.arg("--target").arg(target);
    if release {
        cargo.arg("--release");
    }

    cargo
        .status()?
        .success()
        .then_some(())
        .ok_or(eyre!("Failed to build component: {:?}", manifest_path))?;

    // TODO: this will not work for things built with the host target
    let artifact_path = if let Some(workspace) = workspace {
        workspace
            .join("target")
            .join(target)
            .join(if release { "release" } else { "debug" })
            .join(artifact_name)
    } else {
        manifest_path
            .join("target")
            .join(target)
            .join(if release { "release" } else { "debug" })
            .join(artifact_name)
    };

    Ok(artifact_path)
}
