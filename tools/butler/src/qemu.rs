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
    pub wait_for_gdb_connection: bool,
    /// Passes `-d int` to QEMU. Note that this disables KVM even if `kvm` is set.
    pub debug_int_firehose: bool,
    /// Passes `-d mmu` to QEMU. Note that this disables KVM even if `kvm` is set.
    pub debug_mmu_firehose: bool,
    /// Passes `-d cpu` to QEMU. Note that this disables KVM even if `kvm` is set.
    pub debug_cpu_firehose: bool,

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

impl QemuOptions {
    fn use_kvm(&self) -> bool {
        self.kvm && !(self.debug_int_firehose || self.debug_mmu_firehose || self.debug_cpu_firehose)
    }
}

impl Default for QemuOptions {
    fn default() -> Self {
        QemuOptions {
            kvm: true,
            cpus: 2,
            ram: "512M".to_string(),
            open_display: false,
            wait_for_gdb_connection: false,
            debug_int_firehose: false,
            debug_mmu_firehose: false,
            debug_cpu_firehose: false,

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
        if self.options.use_kvm() {
            qemu.arg("-enable-kvm");
        }
        if self.options.wait_for_gdb_connection {
            qemu.args(&["-s", "-S"]);
        }
        if self.options.debug_int_firehose {
            qemu.args(&["-d", "int"]);
        }
        if self.options.debug_mmu_firehose {
            qemu.args(&["-d", "mmu"]);
        }
        if self.options.debug_cpu_firehose {
            qemu.args(&["-d", "cpu"]);
        }
        qemu.args(&["-machine", "q35"]);
        qemu.args(&["-cpu", "max,vmware-cpuid-freq,invtsc"]);
        qemu.arg("--no-reboot");
        qemu.args(&["-smp", &self.options.cpus.to_string()]);
        qemu.args(&["-m", &self.options.ram.to_string()]);
        qemu.args(&["-serial", "stdio"]);
        if !self.options.open_display {
            qemu.args(&["-display", "none"]);
            // If we're not opening a display, allow connections to the monitor over TCP (open with `nc 127.0.0.1 55555`)
            qemu.args(&["-monitor", "tcp:127.0.0.1:55555,server,nowait"]);
        }
        /*
         * If we're opening a display, we don't want to cause it to close on a crash. If we're just running in the
         * terminal, it's nicer to exit.
         */
        if self.options.open_display {
            qemu.arg("--no-shutdown");
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
