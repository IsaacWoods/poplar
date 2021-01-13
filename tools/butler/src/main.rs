/*
 * TODO: this is going to be Pebble's all-in-one building, testing, hosting big-mess-of-stuff application. You use
 * it on a host to create a Pebble distribution and pave it onto a target, either real (e.g. over a network) or a
 * VM on the host.
 *
 * - Read a config file (e.g. Pebble.toml) to specify options
 * - Build a Pebble distribution
 *      - Build a custom Rust toolchain
 *      - Compile a firmware if needed
 *      - Compile all the things - graph of Steps
 *      - Create an image and put all the bits in
 *      - Meanwhile, put a nice tree in stdout to show the build process
 * - Launch QEMU and pave the image onto it
 * - (in the future) listen to the monitor over serial and format the packets nicely
 *
 * Subcommands:
 *    - `update_submodules` - goes through each submodule, looks at git status, pulls it if clean, presents list at
 *    end (color coded!!!) for status of each one - ([DIRTY], [UP TO DATE], [UPDATED], [REMOTE MISSING!!])
 *    - `rust` - used to manage a custom Pebble rust toolchain
 */

#![feature(bool_to_option, type_ascription, unsized_fn_params)]

mod build;
mod qemu;

use build::{
    cargo::{RunCargo, Target},
    image::MakeGptImage,
    BuildStep,
    MakeDirectories,
};
use clap::{App, Arg};
use eyre::Result;
use qemu::RunQemuX64;
use std::{path::PathBuf, string::ToString};

/// A Project is something that you can instruct Butler to build or run. This might be a Pebble distribution, or
/// something else (e.g. a target-side test that doesn't use the Pebble kernel).
pub struct Project {
    name: String,
    build_steps: Vec<Box<dyn BuildStep>>,
    // TODO: abstract
    pub qemu: Option<RunQemuX64>,
}

impl Project {
    pub fn new(name: String) -> Project {
        Project { name, build_steps: Vec::new(), qemu: None }
    }

    pub fn add_build_step<T>(&mut self, step: T)
    where
        T: BuildStep + 'static,
    {
        self.build_steps.push(Box::new(step));
    }

    pub fn build(&mut self) {
        for step in self.build_steps.drain(..) {
            // TODO: print a nice colored heading for each step, making it easy to see what's going on
            // e.g. '[1/34] Building Cargo project at kernel/efiloader
            match step.build() {
                Ok(_) => (),
                Err(err) => panic!("Build of project {} failed: {:?}", self.name, err),
            }
        }
    }

    pub fn run(self) {
        self.qemu.unwrap().run().unwrap()
    }
}

pub fn main() -> Result<()> {
    color_eyre::install()?;

    let matches = App::new("Butler")
        .version("0.1.0")
        .author("Isaac Woods")
        .about("Host-side program for managing Pebble builds")
        .subcommand(App::new("build").about("Build a project").arg(Arg::from_usage("[project]")))
        .subcommand(App::new("run").about("Build and run a project").arg(Arg::from_usage("[project]")))
        .get_matches();

    if let Some(sub_matches) = matches.subcommand_matches("build") {
        project_from_name(sub_matches.value_of("project")).build();
    } else if let Some(sub_matches) = matches.subcommand_matches("run") {
        let mut project = project_from_name(sub_matches.value_of("project"));
        project.build();
        project.run();
    } else {
        /*
         * If no subcommand is supplied, just build and run a normal Pebble distribution.
         */
        let mut pebble = pebble();
        pebble.build();
        pebble.run();
    }

    Ok(())
}

fn project_from_name(name: Option<&str>) -> Project {
    match name {
        Some("pebble") | None => pebble(),
        Some(other) => panic!("Unknown project name: {}", other),
    }
}

fn pebble() -> Project {
    let build_dir = PathBuf::from("build/Pebble");
    let release = false;

    let mut pebble = Project::new("Pebble".to_string());
    // TODO: it would be nice to not need to copy each artifact out of its target folder, and instead just know the
    // correct paths to put into the GPT step
    pebble.add_build_step(MakeDirectories(build_dir.join("fat/efi/boot/")));
    pebble.add_build_step(RunCargo {
        manifest_path: PathBuf::from("kernel/efiloader/Cargo.toml"),
        target: Target::Triple("x86_64-unknown-uefi".to_string()),
        workspace: PathBuf::from("kernel"),
        release,
        std_components: vec!["core".to_string()],
        std_features: vec![],
        artifact_name: "efiloader.efi".to_string(),
        artifact_path: Some(build_dir.join("fat/efi/boot/bootx64.efi")),
    });
    pebble.add_build_step(RunCargo {
        manifest_path: PathBuf::from("kernel/kernel_x86_64/Cargo.toml"),
        target: Target::Custom {
            triple: "x86_64-kernel".to_string(),
            spec: PathBuf::from("kernel/kernel_x86_64/x86_64-kernel.json"),
        },
        workspace: PathBuf::from("kernel"),
        release,
        std_components: vec!["core".to_string(), "alloc".to_string()],
        std_features: vec![],
        artifact_name: "kernel_x86_64".to_string(),
        artifact_path: Some(build_dir.join("fat/kernel.elf")),
    });
    pebble.add_build_step(MakeGptImage {
        image_path: build_dir.join("pebble.img"),
        image_size: 10 * 1024 * 1024,        // 10MiB
        efi_partition_size: 2 * 1024 * 1024, // 5MiB
        efi_part_files: vec![(String::from("efi/boot/bootx64.efi"), build_dir.join("fat/efi/boot/bootx64.efi"))],
    });

    pebble.qemu = Some(RunQemuX64 {
        kvm: true,
        cpus: 2,
        ram: "512M".to_string(),
        qemu_exit_device: true,
        ovmf_dir: PathBuf::from("bundled/ovmf/"),
        image: build_dir.join("pebble.img"),
    });

    pebble
}
