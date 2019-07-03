export ARCH ?= x86_64
export BUILD_DIR ?= $(abspath ./build)

RUST_GDB_INSTALL_PATH ?= ~/bin/rust-gdb/bin

QEMU_COMMON_FLAGS = -cpu host,vmware-cpuid-freq,invtsc \
					-machine q35 \
					-smp 2 \
					-m 512M \
					-usb \
					-device usb-ehci,id=ehci,bus=pcie.0 \
					--no-reboot \
					--no-shutdown \
					-drive if=pflash,format=raw,file=bootloader/ovmf/OVMF_CODE.fd,readonly \
					-drive if=pflash,format=raw,file=bootloader/ovmf/OVMF_VARS.fd,readonly \
					-drive if=ide,format=raw,file=$< \
					-net none

.PHONY: prepare bootloader kernel test_process clean qemu gdb update fmt test

pebble.img: prepare bootloader kernel test_process
	# Create a temporary image for the FAT partition
	dd if=/dev/zero of=$(BUILD_DIR)/fat.img bs=1M count=64
	mkfs.vfat -F 32 $(BUILD_DIR)/fat.img -n BOOT
	# Copy the stuff into the FAT image
	mcopy -i $(BUILD_DIR)/fat.img -s $(BUILD_DIR)/fat/* ::
	# Create the real image
	dd if=/dev/zero of=$@ bs=512 count=93750
	# Create GPT headers and a single EFI partition
	parted $@ -s -a minimal mklabel gpt
	parted $@ -s -a minimal mkpart EFI FAT32 2048s 93716s
	parted $@ -s -a minimal toggle 1 boot
	# Copy the data from efi.img into the correct place
	dd if=$(BUILD_DIR)/fat.img of=$@ bs=512 count=91669 seek=2048 conv=notrunc
	rm $(BUILD_DIR)/fat.img

prepare:
	@mkdir -p $(BUILD_DIR)/fat/EFI/BOOT

bootloader:
	cargo xbuild --release --target x86_64-unknown-uefi --manifest-path bootloader/Cargo.toml
	cp bootloader/target/x86_64-unknown-uefi/release/bootloader.efi $(BUILD_DIR)/fat/EFI/BOOT/BOOTX64.efi

kernel:
	cargo xbuild --target=kernel/src/$(ARCH)/$(ARCH)-kernel.json --manifest-path kernel/Cargo.toml --features arch_$(ARCH)
	ld --gc-sections -T kernel/src/$(ARCH)/link.ld -o $(BUILD_DIR)/fat/kernel.elf kernel/target/$(ARCH)-kernel/debug/libkernel.a

test_process:
	cargo xbuild --target=test_process/x86_64-pebble-userspace.json --manifest-path test_process/Cargo.toml
	cp test_process/target/x86_64-pebble-userspace/debug/test_process $(BUILD_DIR)/fat/test_process.elf

clean:
	cd bootloader && cargo clean
	cd kernel && cargo clean
	rm -rf build pebble.iso

update:
	cargo update --manifest-path bootloader/Cargo.toml
	cargo update --manifest-path kernel/Cargo.toml
	cargo update --manifest-path x86_64/Cargo.toml
	cargo update --manifest-path libmessage/Cargo.toml

fmt:
	@# `cargo fmt` doesn't play nicely with conditional compilation, so we manually `rustfmt` things
	find kernel/src -type f -name "*.rs" -exec rustfmt {} +
	find x86_64/src -type f -name "*.rs" -exec rustfmt {} +
	cd bootloader && cargo fmt
	cd acpi && cargo fmt
	cd libmessage && cargo fmt
	cd userboot && cargo fmt

test:
	cargo test --all-features --manifest-path kernel/Cargo.toml

doc:
	CARGO_TARGET_DIR=pages cargo doc \
		--all-features \
		--manifest-path kernel/Cargo.toml \
		--document-private-items
	mdbook build

qemu: pebble.img
	qemu-system-x86_64 \
		$(QEMU_COMMON_FLAGS) \
		-enable-kvm

qemu-no-kvm: pebble.img
	qemu-system-x86_64 $(QEMU_COMMON_FLAGS)

debug: pebble.img
	qemu-system-x86_64 \
		$(QEMU_COMMON_FLAGS) \
		-d int

gdb: pebble.img
	qemu-system-x86_64 \
		$(QEMU_COMMON_FLAGS) \
		--enable-kvm \
		-s \
		-S \
	& $(RUST_GDB_INSTALL_PATH)/rust-gdb -q "build/fat/kernel.elf" -ex "target remote :1234"
