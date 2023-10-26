/*
 * Seeing warnings from the `xtask` is annoying, and we have some stuff hanging around that
 * might be useful in the future.
 */
#![allow(dead_code)]

mod cargo;
mod config;
mod flags;
mod image;
mod riscv;
mod x64;

use cargo::Target;
use colored::Colorize;
use config::{Config, Platform};
use eyre::{eyre, Result, WrapErr};
use flags::{DistOptions, TaskCmd};
use riscv::qemu::RunQemuRiscV;
use serde::Serialize;
use std::{
    env,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use x64::qemu::RunQemuX64;
use xshell::pushd;

fn main() -> Result<()> {
    color_eyre::install()?;
    let _root = pushd(project_root())?;

    let flags = flags::Task::from_env()?;
    match flags.subcommand {
        TaskCmd::Help(_) => {
            println!("{}", flags::Task::HELP);
            Ok(())
        }

        TaskCmd::Dist(flags) => {
            let config = config::Config::new(&DistOptions::from(&flags));
            dist(&config)?;
            Ok(())
        }

        TaskCmd::Qemu(flags) => {
            let config = config::Config::new(&DistOptions::from(&flags));
            let dist_result = dist(&config)?;
            match config.platform {
                Platform::X64 => RunQemuX64::new(dist_result.disk_image.unwrap())
                    .open_display(flags.display)
                    .debug_int_firehose(flags.debug_int_firehose)
                    .debug_mmu_firehose(flags.debug_mmu_firehose)
                    .debug_cpu_firehose(flags.debug_cpu_firehose)
                    .run(),
                Platform::RiscV => {
                    RunQemuRiscV::new(dist_result.bootloader_path, dist_result.kernel_path, dist_result.disk_image)
                        .open_display(flags.display)
                        .debug_int_firehose(flags.debug_int_firehose)
                        .run()
                }
            }
        }

        TaskCmd::Opensbi(flags) => {
            match flags.platform.unwrap_or(Platform::default()) {
                Platform::RiscV => {
                    build_opensbi("generic")
                }
                _ => Err(eyre!("OpenSBI is only needed for RISC-V platforms!")),
            }
        }

        TaskCmd::Clean(_) => {
            clean(PathBuf::from("seed/"))?;
            clean(PathBuf::from("kernel"))?;
            clean(PathBuf::from("user"))?;
            clean(PathBuf::from("lib/acpi"))?;
            clean(PathBuf::from("lib/gfxconsole"))?;
            clean(PathBuf::from("lib/poplar"))?;
            clean(PathBuf::from("lib/mer"))?;
            clean(PathBuf::from("lib/pci_types"))?;
            clean(PathBuf::from("lib/poplar_util"))?;
            clean(PathBuf::from("lib/ptah"))?;
            clean(PathBuf::from("lib/virtio"))?;
            Ok(())
        }
    }
}

fn dist(config: &Config) -> Result<DistResult> {
    let dist = Dist {
        release: config.release,
        kernel_features: config.kernel_features.clone(),
        user_tasks: config.user_tasks.clone(),
    };

    match config.platform {
        Platform::X64 => dist.build_x64(),
        Platform::RiscV => dist.build_riscv(),
    }
}

struct Dist {
    release: bool,
    kernel_features: Vec<String>,
    user_tasks: Vec<config::UserTask>,
}

struct DistResult {
    bootloader_path: PathBuf,
    kernel_path: PathBuf,
    disk_image: Option<PathBuf>,
}

#[derive(Clone, Serialize)]
struct SeedConfig {
    user_tasks: Vec<String>,
}

impl Dist {
    pub fn build_riscv(self) -> Result<DistResult> {
        use cargo::RunCargo;
        use image::MakeGptImage;

        println!("{}", "[*] Building D1 boot0".bold().magenta());
        let d1_boot0 = RunCargo::new("d1_boot0", PathBuf::from("seed/d1_boot0/"))
            .workspace(PathBuf::from("seed/"))
            .target(Target::Triple("riscv64imac-unknown-none-elf".to_string()))
            .release(self.release)
            .std_components(vec!["core".to_string()])
            .rustflags("-Clink-arg=-Td1_boot0/link.ld")
            .run()?;
        println!("{}", "[*] Building Seed for RISC-V".bold().magenta());
        let seed_riscv = RunCargo::new("seed_riscv", PathBuf::from("seed/seed_riscv/"))
            .workspace(PathBuf::from("seed/"))
            .target(Target::Triple("riscv64imac-unknown-none-elf".to_string()))
            .release(self.release)
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .rustflags("-Clink-arg=-Tseed_riscv/link.ld")
            .run()?;

        println!("{}", "[*] Building the kernel for RISC-V".bold().magenta());
        let kernel = RunCargo::new("kernel_riscv", PathBuf::from("kernel/kernel_riscv/"))
            .workspace(PathBuf::from("kernel/"))
            .target(Target::Triple("riscv64imac-unknown-none-elf".to_string()))
            .release(self.release)
            .features(self.kernel_features.clone())
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .rustflags("-Clink-arg=-Tkernel_riscv/link.ld")
            .run()?;

        println!("{}", "[*] Building disk image".bold().magenta());
        let image_path = PathBuf::from("poplar_riscv.img");
        let image = MakeGptImage::new(image_path.clone(), 40 * 1024 * 1024, 35 * 1024 * 1024)
            .copy_efi_file("kernel.elf", kernel.clone());
        image.build()?;

        Ok(DistResult { bootloader_path: seed_riscv, kernel_path: kernel, disk_image: Some(image_path) })
    }

    pub fn build_x64(self) -> Result<DistResult> {
        use cargo::RunCargo;
        use image::MakeGptImage;

        println!("{}", "[*] Building Seed for x86_64".bold().magenta());
        let seed_uefi = RunCargo::new("seed_uefi.efi", PathBuf::from("seed/seed_uefi/"))
            .workspace(PathBuf::from("seed/"))
            .target(Target::Triple("x86_64-unknown-uefi".to_string()))
            .release(self.release)
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .std_features(vec!["compiler-builtins-mem".to_string()])
            .run()?;

        println!("{}", "[*] Building the kernel for x86_64".bold().magenta());
        let kernel = RunCargo::new("kernel_x86_64", PathBuf::from("kernel/kernel_x86_64/"))
            .workspace(PathBuf::from("kernel/"))
            .target(Target::Custom {
                triple: "x86_64-kernel".to_string(),
                spec: PathBuf::from("kernel/kernel_x86_64/x86_64-kernel.json"),
            })
            .release(self.release)
            .features(self.kernel_features.clone())
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .std_features(vec!["compiler-builtins-mem".to_string()])
            .run()?;

        let user_tasks = self
            .user_tasks
            .iter()
            .map(|task| {
                let artifact = self.build_userspace_task(
                    &task.name,
                    task.source_dir.clone(),
                    Target::Custom {
                        triple: "x86_64-poplar".to_string(),
                        spec: PathBuf::from("user/x86_64-poplar.json"),
                    },
                )?;
                Ok((task.name.clone(), artifact))
            })
            .collect::<Result<Vec<(String, PathBuf)>>>()?;

        // Generate a file telling Seed how to load us
        let seed_config = self.generate_seed_config();

        println!("{}", "[*] Building disk image".bold().magenta());
        let image_path = PathBuf::from("poplar_x64.img");
        let mut image = MakeGptImage::new(image_path.clone(), 40 * 1024 * 1024, 35 * 1024 * 1024)
            .copy_efi_file("efi/boot/bootx64.efi", seed_uefi.clone())
            .copy_efi_file("kernel.elf", kernel.clone())
            .add_efi_file("config.toml", toml::to_string(&seed_config).unwrap());
        for (name, artifact_path) in user_tasks {
            image = image.copy_efi_file(format!("{}.elf", name), artifact_path);
        }
        image.build()?;

        Ok(DistResult { bootloader_path: seed_uefi, kernel_path: kernel, disk_image: Some(image_path) })
    }

    fn build_userspace_task(&self, name: &str, source_dir: PathBuf, target: Target) -> Result<PathBuf> {
        use cargo::RunCargo;
        println!("{}", format!("[*] Building user task '{}'", name).bold().magenta());

        RunCargo::new(name.to_string(), source_dir)
            .workspace(PathBuf::from("user/")) // TODO: we probably need to provide control over this too
            .target(target)
            .release(self.release)
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .std_features(vec!["compiler-builtins-mem".to_string()])
            .rustflags("-C link-arg=-Tlink.ld")
            .run()
    }

    fn generate_seed_config(&self) -> SeedConfig {
        let user_tasks = self.user_tasks.iter().map(|task| task.name.clone()).collect();
        SeedConfig { user_tasks }
    }
}

pub fn build_opensbi(platform: &str) -> Result<()> {
    println!("{}", format!("[*] Building OpenSBI for platform '{}'", platform).bold().magenta());
    let _dir = pushd("bundled/opensbi")?;
    let output = Command::new("make")
        .arg("LLVM=1")
        .arg(format!("PLATFORM={}", platform))
        .output().unwrap();
    std::io::stdout().write_all(&output.stdout).unwrap();
    std::io::stderr().write_all(&output.stderr).unwrap();
    if !output.status.success() {
        return Err(eyre!("Building OpenSBI failed!"));
    }
    Ok(())
}

fn clean(manifest_dir: PathBuf) -> Result<()> {
    Command::new("cargo")
        .arg("clean")
        .arg("--manifest-path")
        .arg(manifest_dir.join("Cargo.toml"))
        .status()
        .wrap_err("Failed to invoke Cargo to clean a workspace")?
        .success()
        .then_some(())
        .ok_or(eyre!("Failed to clean Cargo workspace"))
}

fn project_root() -> PathBuf {
    Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned()))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
