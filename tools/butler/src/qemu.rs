use eyre::{eyre, Result, WrapErr};
use std::{path::PathBuf, process::Command, string::ToString};

pub struct RunQemuX64 {
    pub kvm: bool,
    pub cpus: u16,
    pub ram: String,
    pub qemu_exit_device: bool,
    pub ovmf_dir: PathBuf,
    pub image: PathBuf,
    pub open_display: bool,
}

impl RunQemuX64 {
    pub fn run(self) -> Result<()> {
        let mut qemu = Command::new("qemu-system-x86_64");

        /*
         * Configure some general stuff.
         */
        if self.kvm {
            qemu.arg("-enable-kvm");
        }
        qemu.args(&["-machine", "q35"]);
        qemu.args(&["-cpu", "max,vmware-cpuid-freq,invtsc"]);
        qemu.arg("--no-reboot");
        qemu.args(&["-smp", &self.cpus.to_string()]);
        qemu.args(&["-m", &self.ram.to_string()]);
        qemu.args(&["-serial", "stdio"]);
        if !self.open_display {
            qemu.args(&["-display", "none"]);
        }

        /*
         * Add hardware.
         * TODO: it would be cool to define devices programmatically, and then have it emit the right config
         */
        qemu.args(&["-net", "none"]);
        if self.qemu_exit_device {
            qemu.args(&["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]);
        }

        qemu.args(&["-device", "qemu-xhci,id=xhci,bus=pcie.0"]);
        qemu.args(&["-device", "usb-kbd,bus=xhci.0"]);
        qemu.args(&["-device", "usb-mouse,bus=xhci.0"]);

        /*
         * Add firmware.
         */
        qemu.args(&[
            "-drive",
            &format!(
                "if=pflash,format=raw,file={},readonly",
                self.ovmf_dir.join("OVMF_CODE.fd").to_str().unwrap()
            ),
        ]);
        qemu.args(&[
            "-drive",
            &format!("if=pflash,format=raw,file={}", self.ovmf_dir.join("OVMF_VARS.fd").to_str().unwrap()),
        ]);

        /*
         * Add the image to run.
         */
        qemu.args(&["-drive", &format!("if=ide,format=raw,file={}", self.image.to_str().unwrap())]);

        println!("Qemu command: {:?}", qemu);
        qemu.status()
            .wrap_err("Failed to invoke qemu-system-x86_64")?
            .success()
            .then_some(())
            .ok_or(eyre!("Qemu returned an error code"))
    }
}
