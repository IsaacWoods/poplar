use eyre::{eyre, Result, WrapErr};
use std::{path::PathBuf, process::Command, string::ToString};

pub struct RunQemuX64 {
    pub image: PathBuf,

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

impl RunQemuX64 {
    pub fn new(image: PathBuf) -> RunQemuX64 {
        RunQemuX64 {
            image,

            kvm: true,
            cpus: 2,
            ram: "1G".to_string(),
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

    pub fn open_display(self, open_display: bool) -> Self {
        Self { open_display, ..self }
    }

    pub fn debug_int_firehose(self, enabled: bool) -> Self {
        Self { debug_int_firehose: enabled, ..self }
    }

    pub fn debug_mmu_firehose(self, enabled: bool) -> Self {
        Self { debug_mmu_firehose: enabled, ..self }
    }

    pub fn debug_cpu_firehose(self, enabled: bool) -> Self {
        Self { debug_cpu_firehose: enabled, ..self }
    }

    fn use_kvm(&self) -> bool {
        self.kvm && !(self.debug_int_firehose || self.debug_mmu_firehose || self.debug_cpu_firehose)
    }

    pub fn run(self) -> Result<()> {
        let mut qemu = Command::new("qemu-system-x86_64");

        /*
         * Configure some general stuff.
         */
        if self.use_kvm() {
            qemu.arg("-enable-kvm");
        }
        if self.wait_for_gdb_connection {
            qemu.args(&["-s", "-S"]);
        }
        if self.debug_int_firehose || self.debug_mmu_firehose || self.debug_cpu_firehose {
            let mut options = Vec::new();
            if self.debug_int_firehose {
                options.push("int");
            }
            if self.debug_mmu_firehose {
                options.push("mmu");
            }
            if self.debug_cpu_firehose {
                options.push("cpu");
            }
            qemu.args(&["-d", &options.join(",")]);
        }
        qemu.args(&["-machine", "q35"]);
        qemu.args(&["-cpu", "max,vmware-cpuid-freq,invtsc"]);
        qemu.arg("--no-reboot");
        // qemu.args(&["-smp", &self.cpus.to_string()]);
        qemu.args(&["-m", &self.ram.to_string()]);

        // Emit serial on both stdio and to a file
        qemu.args(&["-chardev", "stdio,id=char0,logfile=qemu_serial_x64.log"]);
        qemu.args(&["-serial", "chardev:char0"]);

        if !self.open_display {
            qemu.args(&["-display", "none"]);
            // If we're not opening a display, allow connections to the monitor over TCP (open with `nc 127.0.0.1 55555`)
            qemu.args(&["-monitor", "tcp:127.0.0.1:55555,server,nowait"]);
        }

        /*
         * If we're opening a display, we don't want to cause it to close on a crash. If we're just running in the
         * terminal, it's nicer to exit.
         */
        if self.open_display {
            qemu.arg("--no-shutdown");
        }

        /*
         * Add hardware.
         * TODO: it would be cool to define devices programmatically, and then have it emit the right config
         */
        qemu.args(&["-net", "none"]);
        if self.qemu_exit_device {
            qemu.args(&["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]);
        }
        if self.ovmf_debugcon_to_file {
            qemu.args(&["-debugcon", "file:uefi_debug.log"]);
            qemu.args(&["-global", "isa-debugcon.iobase=0x402"]);
        }

        qemu.args(&["-device", "qemu-xhci,id=xhci,bus=pcie.0"]);
        qemu.args(&["-device", "usb-kbd,bus=xhci.0"]);
        qemu.args(&["-device", "usb-mouse,bus=xhci.0"]);

        // XXX: for testing NUMA
        qemu.args(&["-smp", "8"]);
        qemu.args(&["-object", "memory-backend-ram,size=256M,id=m0"]);
        qemu.args(&["-object", "memory-backend-ram,size=256M,id=m1"]);
        qemu.args(&["-object", "memory-backend-ram,size=512M,id=m2"]);
        qemu.args(&["-numa", "node,cpus=0-3,memdev=m0,nodeid=0"]);
        qemu.args(&["-numa", "node,cpus=4-5,memdev=m1,nodeid=1"]);
        qemu.args(&["-numa", "node,cpus=6-7,memdev=m2,nodeid=2"]);
        qemu.args(&["-numa", "dist,src=0,dst=1,val=20"]);
        qemu.args(&["-numa", "dist,src=0,dst=2,val=20"]);
        qemu.args(&["-numa", "dist,src=1,dst=2,val=60"]);

        /*
         * Add firmware.
         */
        qemu.args(&[
            "-drive",
            &format!(
                "if=pflash,format=raw,file={},readonly=on",
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
