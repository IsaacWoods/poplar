//! This module integrates a TOML config file, usually called `Poplar.toml`, and command-line arguments, into the
//! final set of config values.

use crate::DistOptions;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub platform: Platform,
    pub release: bool,
    pub kernel_features: Vec<String>,
    pub user_tasks: Vec<UserTask>,
}

#[derive(Clone, Debug)]
pub struct UserTask {
    pub name: String,
    pub source_dir: PathBuf,
}

/// This represents the options that are read out of the persistent config file. These are then merged with the CLI
/// options and defaults filled in to create a `Config`.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConfigFile {
    platform: Option<Platform>,
    x64: Option<PlatformInfo>,
    rv64_virt: Option<PlatformInfo>,
    mq_pro: Option<PlatformInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub release: Option<bool>,
    pub kernel_features: Option<Vec<String>>,
    pub user_tasks: Option<Vec<String>>,
}

impl Config {
    pub fn new(cli_options: Option<&DistOptions>) -> Config {
        let config_path = match cli_options {
            Some(options) => &options.config_path,
            None => Path::new("Poplar.toml"),
        };
        // TODO: present error message from TOML parsing more nicely
        let file: ConfigFile = toml::from_str(&std::fs::read_to_string(config_path).unwrap()).unwrap();

        let platform = cli_options
            .and_then(|options| options.platform)
            .unwrap_or(file.platform.unwrap_or(Platform::default()));
        let platform_info = match platform {
            Platform::X64 => file.x64.as_ref(),
            Platform::Rv64Virt => file.rv64_virt.as_ref(),
            Platform::MqPro => file.mq_pro.as_ref(),
        };
        let release = cli_options.map_or(false, |options| options.release)
            || platform_info.map_or(false, |info| info.release.unwrap_or(false));
        let kernel_features: Vec<String> = {
            if let Some(from_cli) = cli_options.and_then(|options| options.kernel_features.as_ref()) {
                from_cli.split(',').map(str::to_string).collect()
            } else {
                platform_info.map(|info| info.kernel_features.clone().unwrap_or(vec![])).unwrap_or(vec![])
            }
        };
        let user_tasks: Vec<String> =
            platform_info.map(|info| info.user_tasks.clone().unwrap_or(vec![])).unwrap_or(vec![]);
        let user_tasks = user_tasks
            .into_iter()
            .map(|entry| {
                let mut split = entry.split_whitespace();
                let name = split.next().unwrap().to_string();
                let source_dir = PathBuf::from(split.next().unwrap());
                assert_eq!(split.next(), None);

                UserTask { name, source_dir }
            })
            .collect();

        Config { platform, release, kernel_features, user_tasks }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Platform {
    #[serde(alias = "x64")]
    X64,
    #[serde(alias = "rv64_virt")]
    Rv64Virt,
    #[serde(alias = "mq_pro")]
    MqPro,
}

impl Default for Platform {
    fn default() -> Self {
        Platform::Rv64Virt
    }
}

impl std::str::FromStr for Platform {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "x64" => Ok(Platform::X64),
            "rv64_virt" => Ok(Platform::Rv64Virt),
            "mq_pro" => Ok(Platform::MqPro),
            _ => Err("Unrecognised platform string. Accepted values are `x64` and `riscv`."),
        }
    }
}
