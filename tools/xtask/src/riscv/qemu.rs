use eyre::{eyre, Result, WrapErr};
use std::{path::PathBuf, process::Command, string::ToString};

pub struct RunQemu {
    pub kernel: PathBuf,
    pub opensbi: PathBuf,
}

impl RunQemu {
    pub fn new(kernel: PathBuf) -> RunQemu {
        RunQemu { kernel, opensbi: PathBuf::from("lib/opensbi/build/platform/generic/firmware/fw_jump.elf") }
    }

    pub fn opensbi(self, opensbi: PathBuf) -> Self {
        Self { opensbi, ..self }
    }

    pub fn run(self) -> Result<()> {
        let mut qemu = Command::new("qemu-system-riscv64");

        qemu.args(&["-M", "virt"]);
        qemu.args(&["-bios", self.opensbi.to_str().unwrap()]);
        qemu.args(&["-kernel", self.kernel.to_str().unwrap()]);

        println!("QEMU command: {:?}", qemu);
        qemu.status()
            .wrap_err("Failed to invoke qemu-system-riscv")?
            .success()
            .then_some(())
            .ok_or(eyre!("Qemu returned an error code"))
    }
}
