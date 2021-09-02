#![feature(bool_to_option, type_ascription)]

mod cargo;
mod flags;
mod image;
mod qemu;

use eyre::Result;
use qemu::RunQemuX64;
use std::{
    env,
    path::{Path, PathBuf},
};
use xshell::pushd;

fn main() -> Result<()> {
    color_eyre::install()?;
    let _root = pushd(project_root())?;

    let flags = flags::Task::from_env()?;
    match flags.subcommand {
        flags::TaskCmd::Help(_) => {
            println!("{}", flags::Task::HELP);
            Ok(())
        }

        flags::TaskCmd::Dist(dist) => {
            Dist::new().release(flags.release).kernel_features_from_cli(flags.kernel_features).build()
        }

        flags::TaskCmd::Qemu(qemu) => {
            Dist::new().release(flags.release).kernel_features_from_cli(flags.kernel_features).build()?;
            RunQemuX64::new(PathBuf::from("pebble.img"))
                .open_display(qemu.display)
                .debug_int_firehose(qemu.debug_int_firehose)
                .debug_mmu_firehose(qemu.debug_mmu_firehose)
                .debug_cpu_firehose(qemu.debug_cpu_firehose)
                .run()
        }
    }
}

struct Dist {
    release: bool,
    kernel_features: Vec<String>,
}

impl Dist {
    pub fn new() -> Dist {
        Dist { release: false, kernel_features: vec![] }
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

    pub fn build(self) -> Result<()> {
        use cargo::{RunCargo, Target};
        use image::MakeGptImage;

        let efiloader = RunCargo::new("efiloader.efi", PathBuf::from("kernel/efiloader/"))
            .workspace(PathBuf::from("kernel/"))
            .target(Target::Triple("x86_64-unknown-uefi".to_string()))
            .release(self.release)
            .std_components(vec!["core".to_string()])
            .std_features(vec!["compiler-builtins-mem".to_string()])
            .run()?;

        let kernel = RunCargo::new("kernel_x86_64", PathBuf::from("kernel/kernel_x86_64/"))
            .workspace(PathBuf::from("kernel/"))
            .target(Target::Custom {
                triple: "x86_64-kernel".to_string(),
                spec: PathBuf::from("kernel/kernel_x86_64/x86_64-kernel.json"),
            })
            .release(self.release)
            .features(self.kernel_features.clone())
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .run()?;

        let test1 = self.build_userspace_task("test1")?;
        let simple_fb = self.build_userspace_task("simple_fb")?;
        let platform_bus = self.build_userspace_task("platform_bus")?;
        let pci_bus = self.build_userspace_task("pci_bus")?;
        let usb_bus_xhci = self.build_userspace_task("usb_bus_xhci")?;

        MakeGptImage::new(PathBuf::from("pebble.img"), 30 * 1024 * 1024, 20 * 1024 * 1024)
            .add_efi_file("efi/boot/bootx64.efi".to_string(), efiloader)
            .add_efi_file("kernel.elf".to_string(), kernel)
            .add_efi_file("test1.elf".to_string(), test1)
            .add_efi_file("simple_fb.elf".to_string(), simple_fb)
            .add_efi_file("platform_bus.elf".to_string(), platform_bus)
            .add_efi_file("pci_bus.elf".to_string(), pci_bus)
            .add_efi_file("usb_bus_xhci.elf".to_string(), usb_bus_xhci)
            .build()?;

        Ok(())
    }

    fn build_userspace_task(&self, name: &str) -> Result<PathBuf> {
        use cargo::{RunCargo, Target};

        RunCargo::new(name.to_string(), PathBuf::from("user/").join(name))
            .workspace(PathBuf::from("user/"))
            .toolchain("pebble".to_string())
            .target(Target::Triple("x86_64-pebble".to_string()))
            .release(self.release)
            .std_components(vec!["core".to_string(), "alloc".to_string()])
            .std_features(vec!["compiler-builtins-mem".to_string()])
            .run()
    }
}

fn project_root() -> PathBuf {
    Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned()))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
