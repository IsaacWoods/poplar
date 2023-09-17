#![feature(type_ascription)]
/*
 * Seeing warnings from the `xtask` is annoying, and we have some stuff hanging around that
 * might be useful in the future.
 */
#![allow(dead_code)]

mod cargo;
mod config;
mod flags;
mod riscv;
mod x64;

use cargo::Target;
use colored::Colorize;
use config::{Arch, Config};
use eyre::{eyre, Result, WrapErr};
use flags::{DistOptions, TaskCmd};
use riscv::qemu::RunQemuRiscV;
use std::{
    env,
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
            match config.arch {
                Arch::X64 => RunQemuX64::new(dist_result.disk_image.unwrap())
                    .open_display(flags.display)
                    .debug_int_firehose(flags.debug_int_firehose)
                    .debug_mmu_firehose(flags.debug_mmu_firehose)
                    .debug_cpu_firehose(flags.debug_cpu_firehose)
                    .run(),
                Arch::RiscV => RunQemuRiscV::new(dist_result.kernel_path, dist_result.disk_image.unwrap())
                    .opensbi(PathBuf::from("lib/opensbi/build/platform/generic/firmware/fw_jump.elf"))
                    .open_display(flags.display)
                    .debug_int_firehose(flags.debug_int_firehose)
                    .run(),
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
            Ok(())
        }
    }
}

fn dist(config: &Config) -> Result<DistResult> {
    let dist = Dist::new().release(config.release).kernel_features_from_cli(config.kernel_features.clone());
    // .user_task("simple_fb")
    // .user_task("platform_bus")
    // .user_task("pci_bus")
    // .user_task("usb_bus_xhci")
    // .user_task_in_dir("test_syscalls", PathBuf::from("user/tests"))
    // .user_task_in_dir("test1", PathBuf::from("user/tests"));

    match config.arch {
        Arch::X64 => dist.build_x64(),
        Arch::RiscV => dist.build_riscv(),
    }
}

struct Dist {
    release: bool,
    kernel_features: Vec<String>,
    user_tasks: Vec<(String, Option<PathBuf>)>,
}

struct DistResult {
    kernel_path: PathBuf,
    // Only produced by some architectures
    disk_image: Option<PathBuf>,
}

impl Dist {
    pub fn new() -> Dist {
        Dist { release: false, kernel_features: vec![], user_tasks: vec![] }
    }

    pub fn release(self, release: bool) -> Dist {
        Dist { release, ..self }
    }

    pub fn kernel_features_from_cli(self, features: Option<String>) -> Dist {
        Dist {
            kernel_features: features
                .map(|features| features.split(',').map(str::to_string).collect())
                .unwrap_or(vec![]),
            ..self
        }
    }

    pub fn user_task<S: Into<String>>(mut self, name: S) -> Dist {
        self.user_tasks.push((name.into(), None));
        self
    }

    pub fn user_task_in_dir<S: Into<String>, P: Into<PathBuf>>(mut self, name: S, dir: P) -> Dist {
        self.user_tasks.push((name.into(), Some(dir.into())));
        self
    }

    pub fn build_riscv(self) -> Result<DistResult> {
        use cargo::RunCargo;

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

        Ok(DistResult { kernel_path: seed_riscv, disk_image: Some(kernel) })
    }

    pub fn build_x64(self) -> Result<DistResult> {
        use cargo::RunCargo;
        use x64::image::MakeGptImage;

        println!("{}", "[*] Building Seed for x86_64".bold().magenta());
        let seed_uefi = RunCargo::new("seed_uefi.efi", PathBuf::from("seed/seed_uefi/"))
            .workspace(PathBuf::from("seed/"))
            .target(Target::Triple("x86_64-unknown-uefi".to_string()))
            .release(self.release)
            .std_components(vec!["core".to_string()])
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

        let user_task_paths = self
            .user_tasks
            .iter()
            .map(|(name, dir)| {
                let artifact = self.build_userspace_task(
                    &name,
                    dir.clone(),
                    Target::Custom {
                        triple: "x86_64-poplar".to_string(),
                        spec: PathBuf::from("user/x86_64-poplar.json"),
                    },
                )?;
                Ok((name.clone(), artifact))
            })
            .collect::<Result<Vec<(String, PathBuf)>>>()?;

        println!("{}", "[*] Building disk image".bold().magenta());
        let image_path = PathBuf::from("poplar_x64.img");
        let mut image = MakeGptImage::new(image_path.clone(), 40 * 1024 * 1024, 35 * 1024 * 1024)
            .add_efi_file("efi/boot/bootx64.efi", seed_uefi)
            .add_efi_file("kernel.elf", kernel.clone());
        for (name, artifact_path) in user_task_paths {
            image = image.add_efi_file(format!("{}.elf", name), artifact_path);
        }
        image.build()?;

        Ok(DistResult { kernel_path: kernel, disk_image: Some(image_path) })
    }

    fn build_userspace_task(&self, name: &str, dir: Option<PathBuf>, target: Target) -> Result<PathBuf> {
        use cargo::RunCargo;
        println!("{}", format!("[*] Building user task '{}'", name).bold().magenta());

        let path = if let Some(dir) = dir { dir.join(name) } else { PathBuf::from("user/").join(name) };
        RunCargo::new(name.to_string(), path)
            .workspace(PathBuf::from("user/"))
            .target(target)
            .release(self.release)
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .std_features(vec!["compiler-builtins-mem".to_string()])
            .rustflags("-C link-arg=-Tlink.ld")
            .run()
    }
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
