//! This module integrates a TOML config file, usually called `Poplar.toml`, and command-line arguments, into the
//! final set of config values.

use crate::DistOptions;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Config {
    pub platform: Platform,
    pub release: bool,
    // TODO: how is `None` handled by the building logic?
    pub kernel_features: Option<String>,
}

/// This represents the options that are read out of the persistent config file. These are then merged with the CLI
/// options and defaults filled in to create a `Config`.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConfigFile {
    platform: Option<Platform>,
    x64: Option<PlatformInfo>,
    riscv: Option<PlatformInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub release: Option<bool>,
    pub kernel_features: Option<String>,
}

impl Config {
    pub fn new(cli_options: &DistOptions) -> Config {
        // TODO: present error message from TOML parsing more nicely
        let file: ConfigFile =
            toml::from_str(&std::fs::read_to_string(&cli_options.config_path).unwrap()).unwrap();

        let platform = cli_options.platform.unwrap_or(file.platform.unwrap_or(Platform::default()));
        let platform_info = match platform {
            Platform::X64 => file.x64.as_ref(),
            Platform::RiscV => file.riscv.as_ref(),
        };
        let release =
            cli_options.release || platform_info.map(|info| info.release.unwrap_or(false)).unwrap_or(false);
        let kernel_features =
            cli_options.kernel_features.clone().or(platform_info.and_then(|info| info.kernel_features.clone()));

        Config { platform, release, kernel_features }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Platform {
    #[serde(alias = "x64")]
    X64,
    #[serde(alias = "riscv")]
    RiscV,
}

impl Default for Platform {
    fn default() -> Self {
        Platform::RiscV
    }
}

impl std::str::FromStr for Platform {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "x64" => Ok(Platform::X64),
            "riscv" => Ok(Platform::RiscV),
            _ => Err("Unrecognised platform string. Accepted values are `x64` and `riscv`."),
        }
    }
}
