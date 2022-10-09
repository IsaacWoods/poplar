use eyre::{eyre, Result, WrapErr};
use std::{fs::File, path::PathBuf, process::Command};

pub struct RunQemuRiscV {
    pub opensbi: PathBuf,
    pub seed: PathBuf,
    pub kernel: PathBuf,

    pub open_display: bool,
}

impl RunQemuRiscV {
    pub fn new(seed: PathBuf, kernel: PathBuf) -> RunQemuRiscV {
        RunQemuRiscV {
            opensbi: PathBuf::from("lib/opensbi/build/platform/generic/firmware/fw_jump.elf"),
            seed,
            kernel,

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
        qemu.args(&["-m", "1G"]);
        qemu.args(&["-bios", self.opensbi.to_str().unwrap()]);
        qemu.args(&["-kernel", self.seed.to_str().unwrap()]);
        // qemu.args(&["-fw_cfg", &format!("opt/poplar.kernel,file={}", self.kernel.to_str().unwrap())]);
        let kernel_size =
            File::open(self.kernel.clone()).expect("Failed to open kernel ELF").metadata().unwrap().len();
        qemu.args(&["-device", &format!("loader,addr=0xb0000000,data={},data-len=4", kernel_size)]);
        qemu.args(&[
            "-device",
            &format!("loader,file={},addr=0xb0000004,force-raw=on", self.kernel.to_str().unwrap()),
        ]);

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
