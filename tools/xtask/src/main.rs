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

        flags::TaskCmd::Dist(_) => dist(&flags),

        flags::TaskCmd::Qemu(ref qemu) => {
            dist(&flags)?;
            RunQemuX64::new(PathBuf::from("pebble.img"))
                .open_display(qemu.display)
                .debug_int_firehose(qemu.debug_int_firehose)
                .debug_mmu_firehose(qemu.debug_mmu_firehose)
                .debug_cpu_firehose(qemu.debug_cpu_firehose)
                .run()
        }
    }
}

pub fn dist(flags: &flags::Task) -> Result<()> {
    Dist::new()
        .release(flags.release)
        .kernel_features_from_cli(&flags.kernel_features)
        .user_task("test1")
        .user_task("simple_fb")
        .user_task("platform_bus")
        .user_task("pci_bus")
        .user_task("usb_bus_xhci")
        .build()
}

struct Dist {
    release: bool,
    kernel_features: Vec<String>,
    user_tasks: Vec<String>,
}

impl Dist {
    pub fn new() -> Dist {
        Dist { release: false, kernel_features: vec![], user_tasks: vec![] }
    }

    pub fn release(self, release: bool) -> Dist {
        Dist { release, ..self }
    }

    pub fn kernel_features_from_cli(self, features: &Option<String>) -> Dist {
        Dist {
            kernel_features: features
                .as_ref()
                .map(|features| features.split(',').map(str::to_string).collect())
                .unwrap_or(vec![]),
            ..self
        }
    }

    pub fn user_task<S: Into<String>>(mut self, name: S) -> Dist {
        self.user_tasks.push(name.into());
        self
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

        let user_task_paths = self
            .user_tasks
            .iter()
            .map(|name| {
                let artifact_path = self.build_userspace_task(&name)?;
                Ok((name.clone(), artifact_path))
            })
            .collect::<Result<Vec<(String, PathBuf)>>>()?;

        let mut image = MakeGptImage::new(PathBuf::from("pebble.img"), 30 * 1024 * 1024, 20 * 1024 * 1024)
            .add_efi_file("efi/boot/bootx64.efi", efiloader)
            .add_efi_file("kernel.elf", kernel);
        for (name, artifact_path) in user_task_paths {
            image = image.add_efi_file(format!("{}.elf", name), artifact_path);
        }
        image.build()?;

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
