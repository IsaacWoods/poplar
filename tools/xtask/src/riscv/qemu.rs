use eyre::{eyre, Result, WrapErr};
use std::{path::PathBuf, process::Command};

pub struct RunQemuRiscV {
    pub kernel: PathBuf,
    pub opensbi: PathBuf,

    pub open_display: bool,
}

impl RunQemuRiscV {
    pub fn new(kernel: PathBuf) -> RunQemuRiscV {
        RunQemuRiscV {
            kernel,
            opensbi: PathBuf::from("lib/opensbi/build/platform/generic/firmware/fw_jump.elf"),
            open_display: false,
        }
    }

    pub fn opensbi(self, opensbi: PathBuf) -> Self {
        Self { opensbi, ..self }
    }

    pub fn open_display(self, open_display: bool) -> Self {
        Self { open_display, ..self }
    }

    pub fn run(self) -> Result<()> {
        let mut qemu = Command::new("qemu-system-riscv64");

        qemu.args(&["-M", "virt"]);
        qemu.args(&["-bios", self.opensbi.to_str().unwrap()]);
        qemu.args(&["-kernel", self.kernel.to_str().unwrap()]);

        // Emit serial on both stdio and to a file
        qemu.args(&["-chardev", "stdio,id=char0,logfile=qemu_serial_riscv.log"]);
        qemu.args(&["-serial", "chardev:char0"]);

        if !self.open_display {
            qemu.args(&["-display", "none"]);
            // If we're not opening a display, allow connections to the monitor over TCP (open with `nc 127.0.0.1 55555`)
            qemu.args(&["-monitor", "tcp:127.0.0.1:55555,server,nowait"]);
        }

        println!("QEMU command: {:?}", qemu);
        qemu.status()
            .wrap_err("Failed to invoke qemu-system-riscv")?
            .success()
            .then_some(())
            .ok_or(eyre!("Qemu returned an error code"))
    }
}
