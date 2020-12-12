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
 */

mod build;

use build::RunCargo;
use std::{path::PathBuf, string::ToString};

#[tokio::main]
async fn main() {
    let matches = clap::App::new("Butler")
        .version("0.1.0")
        .author("Isaac Woods")
        .about("Host-side program for managing Pebble builds")
        .subcommand(clap::SubCommand::with_name("build").about("Builds a Pebble distribution"))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("build") {
        println!("Build requested");
    }

    // TODO: test
    let kernel_build = RunCargo {
        manifest_path: PathBuf::from("kernel/kernel_x86_64/Cargo.toml"),
        target: Some("kernel/kernel_x86_64/x86_64-kernel.json".to_string()),
        release: false,
        std_components: vec!["core".to_string(), "alloc".to_string()],
    };
    assert!(kernel_build.build().await.unwrap().success());

    println!("Success");
}
