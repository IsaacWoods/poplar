//! This module integrates a TOML config file, usually called `Poplar.toml`, and command-line arguments,

use crate::DistOptions;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Config {
    pub arch: Arch,
    pub release: bool,
    // TODO: should this be an option? How is `None` handled?
    pub kernel_features: Option<String>,
}

/// This represents the options that are read out of the persistent config file. These are then merged with the CLI
/// options and defaults filled in to create a `Config`.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConfigFile {
    arch: Option<Arch>,
    release: Option<bool>,
    kernel_features: Option<String>,
}

impl Config {
    pub fn new(cli_options: &DistOptions) -> Config {
        // TODO: present error message from TOML parsing more nicely
        let file: ConfigFile =
            toml::from_str(&std::fs::read_to_string(&cli_options.config_path).unwrap()).unwrap();

        // TODO: document how options from CLI and file are merged, as it depends
        Config {
            arch: cli_options.arch.unwrap_or(file.arch.unwrap_or(Arch::default())),
            release: if let Some(file_release) = file.release {
                cli_options.release || file_release
            } else {
                cli_options.release
            },
            kernel_features: cli_options.kernel_features.clone().or(file.kernel_features),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Arch {
    #[serde(alias = "x64")]
    X64,
    #[serde(alias = "riscv")]
    RiscV,
}

impl Default for Arch {
    fn default() -> Self {
        Arch::RiscV
    }
}

impl std::str::FromStr for Arch {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "x64" => Ok(Arch::X64),
            "riscv" => Ok(Arch::RiscV),
            _ => Err("Unrecognised arch string. Accepted values are `x64` and `riscv`."),
        }
    }
}
