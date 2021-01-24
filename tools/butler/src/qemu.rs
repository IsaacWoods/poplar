use eyre::{eyre, Result, WrapErr};
use std::{path::PathBuf, process::Command, string::ToString};

pub struct QemuOptions {
    /*
     * General
     */
    pub kvm: bool,
    pub cpus: u16,
    pub ram: String,
    pub open_display: bool,

    /*
     * Firmware
     */
    pub ovmf_dir: PathBuf,
    pub ovmf_debugcon_to_file: bool,

    /*
     * Devices
     */
    pub qemu_exit_device: bool,
}

impl Default for QemuOptions {
    fn default() -> Self {
        QemuOptions {
            kvm: true,
            cpus: 2,
            ram: "512M".to_string(),
            open_display: false,

            ovmf_dir: PathBuf::from("bundled/ovmf/"),
            ovmf_debugcon_to_file: false,

            qemu_exit_device: true,
        }
    }
}

pub struct RunQemuX64 {
    pub options: QemuOptions,
    pub image: PathBuf,
}

impl RunQemuX64 {
    pub fn run(self) -> Result<()> {
        let mut qemu = Command::new("qemu-system-x86_64");

        /*
         * Configure some general stuff.
         */
        if self.options.kvm {
            qemu.arg("-enable-kvm");
        }
        qemu.args(&["-machine", "q35"]);
        qemu.args(&["-cpu", "max,vmware-cpuid-freq,invtsc"]);
        qemu.arg("--no-reboot");
        qemu.arg("--no-shutdown");
        qemu.args(&["-smp", &self.options.cpus.to_string()]);
        qemu.args(&["-m", &self.options.ram.to_string()]);
        qemu.args(&["-serial", "stdio"]);
        if !self.options.open_display {
            qemu.args(&["-display", "none"]);
        }

        /*
         * Add hardware.
         * TODO: it would be cool to define devices programmatically, and then have it emit the right config
         */
        qemu.args(&["-net", "none"]);
        if self.options.qemu_exit_device {
            qemu.args(&["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]);
        }
        if self.options.ovmf_debugcon_to_file {
            qemu.args(&["-debugcon", "file:uefi_debug.log"]);
            qemu.args(&["-global", "isa-debugcon.iobase=0x402"]);
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
                self.options.ovmf_dir.join("OVMF_CODE.fd").to_str().unwrap()
            ),
        ]);
        qemu.args(&[
            "-drive",
            &format!("if=pflash,format=raw,file={}", self.options.ovmf_dir.join("OVMF_VARS.fd").to_str().unwrap()),
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
