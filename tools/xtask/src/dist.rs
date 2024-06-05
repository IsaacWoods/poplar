use crate::{config::Platform, image::MakeGptImage, ramdisk::Ramdisk};
use colored::Colorize;
use serde::Serialize;
use std::path::PathBuf;

/// Represents a number of artifacts from the build process. You can use this to retrieve artifacts
/// by name or type, and build the ramdisk or disk image for a platform.
#[derive(Clone, Debug)]
pub struct DistResult {
    platform: Platform,
    artifacts: Vec<Artifact>,
    seed_config: Option<SeedConfig>,
}

impl DistResult {
    pub fn new(platform: Platform) -> DistResult {
        DistResult { platform, artifacts: Vec::new(), seed_config: None }
    }

    pub fn add(&mut self, artifact: Artifact) {
        self.artifacts.push(artifact);
    }

    pub fn add_seed_config(&mut self, config: SeedConfig) {
        self.seed_config = Some(config);
    }

    pub fn artifact_by_name(&self, name: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|artifact| artifact.name == name)
    }

    /// Get the first artifact that has the matching type
    /// TODO: should this instead by all artifacts with that type??
    pub fn artifact_by_type(&self, typ: ArtifactType) -> Option<&Artifact> {
        self.artifacts.iter().find(|artifact| artifact.typ == typ)
    }

    /// Construct a `Ramdisk`, including all artifacts that are marked to be added.
    pub fn build_ramdisk(&self) -> Ramdisk {
        let mut ramdisk = Ramdisk::new(self.platform);

        for artifact in &self.artifacts {
            if artifact.include_in_ramdisk {
                ramdisk.add(&artifact.name, &artifact.source);
            }
        }

        ramdisk
    }

    pub fn build_disk_image(&self) -> PathBuf {
        println!("{}", "[*] Building disk image".bold().magenta());

        let image_path = PathBuf::from(format!("poplar_{}.img", self.platform));
        let mut image = MakeGptImage::new(image_path.clone(), 40 * 1024 * 1024, 35 * 1024 * 1024);

        for artifact in &self.artifacts {
            if let Some(disk_path) = &artifact.disk_path {
                image = image.copy_efi_file(disk_path, artifact.source.clone());
            }
        }

        // If a config file for Seed is required, add it here
        if let Some(config) = &self.seed_config {
            image = image.add_efi_file("config.toml", toml::to_string(config).unwrap());
        }

        image.build().unwrap();
        image_path
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArtifactType {
    BootShim,
    Bootloader,
    Kernel,
    UserTask,
}

#[derive(Clone, Debug)]
pub struct Artifact {
    pub name: String,
    pub typ: ArtifactType,
    pub source: PathBuf,

    pub include_in_ramdisk: bool,
    pub disk_path: Option<String>,
}

impl Artifact {
    pub fn new(name: &str, typ: ArtifactType, source: PathBuf) -> Artifact {
        Artifact { name: name.to_string(), typ, source, include_in_ramdisk: false, disk_path: None }
    }

    pub fn include_in_ramdisk(self) -> Artifact {
        Artifact { include_in_ramdisk: true, ..self }
    }

    pub fn include_in_disk_image(self, path: String) -> Artifact {
        Artifact { disk_path: Some(path), ..self }
    }
}

/// This represents the expected structure of a Seed config file. It is constructed and serialized
/// to TOML during artifact construction.
#[derive(Clone, Debug, Serialize)]
pub struct SeedConfig {
    pub user_tasks: Vec<String>,
}
