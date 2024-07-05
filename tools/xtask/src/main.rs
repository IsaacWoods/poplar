/*
 * Seeing warnings from the `xtask` is annoying, and we have some stuff hanging around that
 * might be useful in the future.
 */
#![allow(dead_code)]

mod cargo;
mod config;
mod dist;
mod doc;
mod flags;
mod image;
mod ramdisk;
mod riscv;
mod serial;
mod x64;

use crate::{
    cargo::RunCargo,
    dist::{Artifact, ArtifactType, DistResult, SeedConfig},
};
use cargo::Target;
use colored::Colorize;
use config::{Config, Platform};
use doc::DocGenerator;
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

    let flags = flags::Task::from_env_or_exit();
    match flags.subcommand {
        TaskCmd::Dist(flags) => {
            let config = config::Config::new(Some(&DistOptions::from(&flags)));
            dist(&config)?;
            Ok(())
        }

        TaskCmd::Qemu(flags) => {
            let config = config::Config::new(Some(&DistOptions::from(&flags)));
            let dist_result = dist(&config)?;

            match config.platform {
                Platform::X64 => RunQemuX64::new(dist_result.build_disk_image())
                    .open_display(flags.display)
                    .debug_int_firehose(flags.debug_int_firehose)
                    .debug_mmu_firehose(flags.debug_mmu_firehose)
                    .debug_cpu_firehose(flags.debug_cpu_firehose)
                    .run(),
                Platform::Rv64Virt => {
                    let ramdisk = dist_result.build_ramdisk();
                    // TODO: support disk images here again at some point
                    RunQemuRiscV::new(
                        dist_result.artifact_by_type(ArtifactType::Bootloader).unwrap().source.clone(),
                        None,
                    )
                    .ramdisk(Some(ramdisk))
                    .open_display(flags.display)
                    .debug_int_firehose(flags.debug_int_firehose)
                    .run()
                }
                _ => {
                    panic!("Platform does not support running in QEMU");
                }
            }
        }

        TaskCmd::Boot(flags) => {
            let config = config::Config::new(Some(&DistOptions::from(&flags)));
            let dist_result = dist(&config)?;

            match config.platform {
                Platform::MqPro => {
                    let serial = serial::Serial::new(&Path::new("/dev/ttyUSB0"), 115200);

                    let bootloader = &dist_result.artifact_by_type(ArtifactType::Bootloader).unwrap().source;

                    println!("{}", format!("[*] Uploading and running code on device").bold().magenta());
                    Command::new("xfel").arg("ddr").arg("d1").status().unwrap();
                    Command::new("xfel").arg("write").arg("0x40000000").arg("bundled/opensbi/build/platform/generic/firmware/fw_jump.bin").status().unwrap();
                    Command::new("xfel").arg("write").arg("0x40080000").arg(bootloader).status().unwrap();

                    // Load the ramdisk into memory
                    let ramdisk = dist_result.build_ramdisk();
                    let (header, entries) = ramdisk.create();
                    const RAMDISK_BASE_ADDR: u64 = 0x4000_0000 + 0x100000;  // 1 MiB above the base of RAM
                    Command::new("xfel").arg("write").arg(format!("{:#x}", RAMDISK_BASE_ADDR)).arg(header).status().unwrap();
                    for (offset, source) in entries {
                        println!("Loading ramdisk entry {:?} at {:#x}", source, RAMDISK_BASE_ADDR + offset as u64);
                        Command::new("xfel").arg("write").arg(format!("{:#x}", RAMDISK_BASE_ADDR + offset as u64)).arg(source).status().unwrap();
                    }

                    // Tell the device to start running code!
                    Command::new("xfel").arg("exec").arg("0x40000000").status().unwrap();

                    println!("{}", format!("[*] Listening to serial").bold().magenta());
                    serial.listen();
                }
                Platform::Uconsole => {
                    let serial = serial::Serial::new(&Path::new("/dev/ttyUSB0"), 115200);

                    println!("{}", format!("[*] Uploading and running code on device").bold().magenta());
                    Command::new("xfel").arg("ddr").arg("d1").status().unwrap();
                    todo!();

                    println!("{}", format!("[*] Listening to serial").bold().magenta());
                    serial.listen();
                }
                other => panic!("Platform '{:?}' does not support booting directly to a device (use `qemu` to emulate in QEMU instead?)!", other),
            }
        }

        TaskCmd::Opensbi(flags) => {
            let config = config::Config::new(Some(&DistOptions::from(&flags)));
            match config.platform {
                Platform::MqPro => {
                    let fdt_path = compile_device_tree(Path::new("bundled/device_tree/d1_mangopi_mq_pro.dts"))
                        .unwrap()
                        .canonicalize()
                        .unwrap();
                    build_opensbi("generic", &fdt_path, 0x4000_0000, 0x4008_0000)
                }
                _ => Err(eyre!("OpenSBI is only needed for RISC-V platforms!")),
            }
        }

        TaskCmd::Devicetree(flags) => compile_device_tree(&flags.path).map(|_| ()),

        TaskCmd::Doc(flags) => {
            let generator = DocGenerator::new(flags);
            generator.generate()
        }

        TaskCmd::Clean(_) => {
            // TODO: put a big list of crates that need cleaning etc. in the config?
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
            clean(PathBuf::from("lib/usb"))?;
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
        Platform::Rv64Virt => dist.build_rv64_virt(),
        Platform::MqPro => dist.build_mq_pro(),
        Platform::Uconsole => dist.build_uconsole(),
    }
}

struct Dist {
    release: bool,
    kernel_features: Vec<String>,
    user_tasks: Vec<config::UserTask>,
}

impl Dist {
    pub fn build_rv64_virt(self) -> Result<DistResult> {
        let mut result = DistResult::new(Platform::Rv64Virt);

        println!("{}", "[*] Building Seed for RISC-V".bold().magenta());
        let seed_riscv = RunCargo::new("seed_riscv", PathBuf::from("seed/seed_riscv/"))
            .workspace(PathBuf::from("seed/"))
            .target(Target::Triple("riscv64imac-unknown-none-elf".to_string()))
            .release(self.release)
            .features(vec!["platform_rv64_virt".to_string()])
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .rustflags("-Clink-arg=-Tseed_riscv/rv64_virt.ld")
            .run()?;
        result.add(Artifact::new("seed_riscv", ArtifactType::Bootloader, seed_riscv));

        println!("{}", "[*] Building the kernel for RISC-V".bold().magenta());
        let kernel = RunCargo::new("kernel_riscv", PathBuf::from("kernel/kernel_riscv/"))
            .workspace(PathBuf::from("kernel/"))
            .target(Target::Triple("riscv64imac-unknown-none-elf".to_string()))
            .release(self.release)
            .features(vec!["platform_rv64_virt".to_string()])
            .features(self.kernel_features.clone())
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .rustflags("-Clink-arg=-Tkernel_riscv/rv64_virt.ld")
            .run()?;
        result.add(Artifact::new("kernel_riscv", ArtifactType::Kernel, kernel).include_in_ramdisk());

        for task in &self.user_tasks {
            let artifact = self.build_userspace_task(
                &task.name,
                task.source_dir.clone(),
                Target::Triple("riscv64gc-unknown-none-elf".to_string()),
            )?;
            result.add(Artifact::new(&task.name, ArtifactType::UserTask, artifact).include_in_ramdisk());
        }

        result.add_seed_config(self.generate_seed_config());

        Ok(result)
    }

    pub fn build_mq_pro(self) -> Result<DistResult> {
        let mut result = DistResult::new(Platform::MqPro);

        // println!("{}", "[*] Building D1 boot0".bold().magenta());
        // let _d1_boot0 = RunCargo::new("d1_boot0", PathBuf::from("seed/d1_boot0/"))
        //     .workspace(PathBuf::from("seed/"))
        //     .target(Target::Triple("riscv64imac-unknown-none-elf".to_string()))
        //     .release(self.release)
        //     .std_components(vec!["core".to_string()])
        //     .rustflags("-Clink-arg=-Td1_boot0/link.ld")
        //     .flatten_result(true)
        //     .run()?;

        println!("{}", "[*] Building Seed for RISC-V".bold().magenta());
        let seed_riscv = RunCargo::new("seed_riscv", PathBuf::from("seed/seed_riscv/"))
            .workspace(PathBuf::from("seed/"))
            .target(Target::Triple("riscv64imac-unknown-none-elf".to_string()))
            .release(self.release)
            .features(vec!["platform_mq_pro".to_string()])
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .rustflags("-Clink-arg=-Tseed_riscv/mq_pro.ld")
            .flatten_result(true)
            .run()?;
        result.add(Artifact::new("seed_riscv", ArtifactType::Bootloader, seed_riscv));

        println!("{}", "[*] Building the kernel for RISC-V".bold().magenta());
        let kernel = RunCargo::new("kernel_riscv", PathBuf::from("kernel/kernel_riscv/"))
            .workspace(PathBuf::from("kernel/"))
            .target(Target::Triple("riscv64imac-unknown-none-elf".to_string()))
            .release(self.release)
            .features(vec!["platform_mq_pro".to_string()])
            .features(self.kernel_features.clone())
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .rustflags("-Clink-arg=-Tkernel_riscv/mq_pro.ld")
            .run()?;
        result.add(Artifact::new("kernel_riscv", ArtifactType::Kernel, kernel).include_in_ramdisk());

        Ok(result)
    }

    pub fn build_uconsole(self) -> Result<DistResult> {
        let mut result = DistResult::new(Platform::Uconsole);

        println!("{}", "[*] Building D1 boot0".bold().magenta());
        let d1_boot0 = RunCargo::new("d1_boot0", PathBuf::from("seed/d1_boot0/"))
            .workspace(PathBuf::from("seed/"))
            .target(Target::Triple("riscv64imac-unknown-none-elf".to_string()))
            .release(self.release)
            .std_components(vec!["core".to_string()])
            .rustflags("-Clink-arg=-Td1_boot0/link.ld")
            .flatten_result(true)
            .run()?;
        result.add(Artifact::new("d1_boot0", ArtifactType::BootShim, d1_boot0));

        Ok(result)
    }

    pub fn build_x64(self) -> Result<DistResult> {
        let mut result = DistResult::new(Platform::X64);

        println!("{}", "[*] Building Seed for x86_64".bold().magenta());
        let seed_uefi = RunCargo::new("seed_uefi.efi", PathBuf::from("seed/seed_uefi/"))
            .workspace(PathBuf::from("seed/"))
            .target(Target::Triple("x86_64-unknown-uefi".to_string()))
            .release(self.release)
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .std_features(vec!["compiler-builtins-mem".to_string()])
            .run()?;
        result.add(
            Artifact::new("seed_uefi", ArtifactType::Bootloader, seed_uefi)
                .include_in_disk_image("efi/boot/bootx64.efi".to_string()),
        );

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
        result.add(
            Artifact::new("kernel", ArtifactType::Kernel, kernel).include_in_disk_image("kernel.elf".to_string()),
        );

        for task in &self.user_tasks {
            let artifact = self.build_userspace_task(
                &task.name,
                task.source_dir.clone(),
                Target::Custom {
                    triple: "x86_64-poplar".to_string(),
                    spec: PathBuf::from("user/x86_64-poplar.json"),
                },
            )?;
            let path = format!("{}.elf", task.name);
            result.add(Artifact::new(&task.name, ArtifactType::UserTask, artifact).include_in_disk_image(path));
        }

        result.add_seed_config(self.generate_seed_config());

        Ok(result)
    }

    fn build_userspace_task(&self, name: &str, source_dir: PathBuf, target: Target) -> Result<PathBuf> {
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

pub fn build_opensbi(platform: &str, fdt: &Path, load_addr: u64, jump_addr: u64) -> Result<()> {
    println!("{}", format!("[*] Building OpenSBI for platform '{}'", platform).bold().magenta());
    let _dir = pushd("bundled/opensbi")?;
    let status = Command::new("make")
        .arg("LLVM=1")
        .arg(format!("PLATFORM={}", platform))
        .arg(format!("FW_FDT_PATH={}", fdt.display()))
        .arg(format!("FW_TEXT_START={:#x}", load_addr))
        .arg(format!("FW_JUMP_ADDR={:#x}", jump_addr))
        .status()
        .unwrap();
    if !status.success() {
        return Err(eyre!("Building OpenSBI failed!"));
    }
    Ok(())
}

/// Compile the device tree source (`.dts`) at `path` into a device tree blob (`.dtb`). This
/// requires `dtc` to be installed, as well as `cpp` (the C preprocessor) as many device tree
/// sources make use of preprocessor directives.
// TODO: we don't yet correctly handle preprocessor include paths - our current DTs have been
// manually concatenated but in the future we'll probably want to handle this properly.
// TODO: this should probably check if the source file is newer than the blob before re-compiling
// every time (included files though?)
pub fn compile_device_tree(path: &Path) -> Result<PathBuf> {
    use std::process::Stdio;

    println!("{}", format!("[*] Compiling device tree at '{}'", path.display()).bold().magenta());

    let preprocessor = Command::new("cpp")
        .args(&["-x", "assembler-with-cpp"])
        .arg("-nostdinc")
        .arg(path)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let blob_path = path.with_extension("dtb");
    let _compiler = Command::new("dtc")
        .args(&["-O", "dtb"])
        .args(&["-o", blob_path.to_str().unwrap()])
        .stdin(Stdio::from(preprocessor.stdout.unwrap()))
        .status()
        .unwrap();
    Ok(blob_path)
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
