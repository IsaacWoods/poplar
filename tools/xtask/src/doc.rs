//! We want to generate documentation for each relevant crate in the Poplar monorepo, and link them
//! such that everything can be accessed from one Rustdoc instance.
//!
//! To do this while preserving as much native `rustdoc` functionality (e.g. inter-crate linking in
//! type descriptions) as possible, we document the 'leaf' crates with all their dependencies, and
//! then manually merge these together into a single Rustdoc tree. By building each 'leaf' with the
//! correct target, we should build target-specific crates in the correct context.

use crate::{
    cargo::{RunCargo, Subcommand, Target},
    flags::Doc as DocFlags,
};
use eyre::Result;
use fs_extra::dir::CopyOptions;
use std::{fs, path::PathBuf};

pub struct Crate {
    path: PathBuf,
    target: Target,
    workspace: Option<PathBuf>,
}

pub struct DocGenerator {
    flags: DocFlags,
    crates: Vec<Crate>,
}

impl DocGenerator {
    pub fn new(flags: DocFlags) -> DocGenerator {
        DocGenerator {
            flags,
            crates: vec![
                Crate {
                    path: PathBuf::from("kernel/kernel_riscv"),
                    target: Target::Triple("riscv64imac-unknown-none-elf".to_string()),
                    workspace: Some(PathBuf::from("kernel")),
                },
                Crate {
                    path: PathBuf::from("seed/seed_riscv"),
                    target: Target::Triple("riscv64imac-unknown-none-elf".to_string()),
                    workspace: Some(PathBuf::from("seed")),
                },
            ],
        }
    }

    pub fn generate(self) -> Result<()> {
        // Create the destination directory if it does not exist
        if !self.flags.path.is_dir() {
            std::fs::create_dir_all(&self.flags.path)?;
        }

        for crat in &self.crates {
            let crate_doc_dir = self.document_crate(&crat)?;

            /*
             * Copy each subdirectory over to the final destination.
             */
            for entry in crate_doc_dir.read_dir()? {
                let entry = entry?;
                if entry.path().is_dir() {
                    fs_extra::copy_items(
                        &[entry.path()],
                        &self.flags.path,
                        &CopyOptions { overwrite: true, ..Default::default() },
                    )?;
                } else if let Some("html") = entry.path().extension().map(|ext| ext.to_str().unwrap()) {
                    fs::copy(entry.path(), &self.flags.path.join(entry.file_name()))?;
                }
            }

            /*
             * Add each crate's data to the JS search index and source file database.
             */
            if !self.flags.path.join("search-index.js").is_file() {
                // If a `search-index.js` file does not yet exist, copy this one across.
                fs::copy(crate_doc_dir.join("search-index.js"), self.flags.path.join("search-index.js"))?;
            } else {
                println!("Need to merge search index: currently not implemented!");
            }
        }

        Ok(())
    }

    /// Document a crate, and its dependencies. Returns the directory containing the generated docs.
    fn document_crate(&self, info: &Crate) -> Result<PathBuf> {
        let mut cargo = RunCargo::new("doc", info.path.clone())
            .subcommand(Subcommand::Doc)
            .target(info.target.clone())
            .extra(vec!["--document-private-items".to_string()]);
        if let Some(workspace) = &info.workspace {
            cargo = cargo.workspace(workspace.clone());
        }

        cargo.run()
    }
}
