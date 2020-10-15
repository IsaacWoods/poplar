export PLATFORM ?= x86_64
export BUILD_DIR ?= $(abspath ./build)
# In the future, this should just be features we can enable, but this is easier in makefile
export KERNEL_FLAGS ?=

IMAGE_NAME ?= pebble.img
DISK_NAME ?= /dev/sdc
QEMU_DIR ?=
QEMU_COMMON_FLAGS = -cpu max,vmware-cpuid-freq,invtsc \
					-machine q35 \
					-smp 2 \
					-m 512M \
					-device isa-debug-exit,iobase=0xf4,iosize=0x04 \
					-device qemu-xhci,id=xhci,bus=pcie.0 \
						-device usb-kbd,bus=xhci.0 \
						-device usb-mouse,bus=xhci.0 \
					--no-reboot \
					--no-shutdown \
					-drive if=pflash,format=raw,file=bundled/ovmf/OVMF_CODE.fd,readonly \
					-drive if=pflash,format=raw,file=bundled/ovmf/OVMF_VARS.fd \
					-drive if=ide,format=raw,file=$(IMAGE_NAME) \
					-net none
# This can be used to pass extra flags to QEMU
QEMU_EXTRA_FLAGS ?=

.PHONY: image_x86_64 prepare kernel user clean qemu gdb update fmt
.DEFAULT_GOAL := image_$(PLATFORM)

# This is a temporary target to write to a real disk
image_disk: prepare kernel user
	# Create a temporary image for the FAT partition
	dd if=/dev/zero of=$(BUILD_DIR)/fat.img bs=1M count=64
	mkfs.vfat -F 32 $(BUILD_DIR)/fat.img -n BOOT
	# Copy the stuff into the FAT image
	mcopy -i $(BUILD_DIR)/fat.img -s $(BUILD_DIR)/fat/* ::
	# Create GPT headers and a single EFI partition
	sudo parted $(DISK_NAME) -s -a minimal mklabel gpt
	sudo parted $(DISK_NAME) -s -a minimal mkpart EFI FAT32 2048s 93716s
	sudo parted $(DISK_NAME) -s -a minimal toggle 1 boot
	# Copy the data from efi.img into the correct place
	sudo dd if=$(BUILD_DIR)/fat.img of=$(DISK_NAME) bs=512 count=91669 seek=2048 conv=notrunc
	rm $(BUILD_DIR)/fat.img

image_x86_64: prepare kernel user
	# Create a temporary image for the FAT partition
	dd if=/dev/zero of=$(BUILD_DIR)/fat.img bs=1M count=64
	mkfs.vfat -F 32 $(BUILD_DIR)/fat.img -n BOOT
	# Copy the stuff into the FAT image
	mcopy -i $(BUILD_DIR)/fat.img -s $(BUILD_DIR)/fat/* ::
	# Create the real image
	dd if=/dev/zero of=$(IMAGE_NAME) bs=512 count=93750
	# Create GPT headers and a single EFI partition
	parted $(IMAGE_NAME) -s -a minimal mklabel gpt
	parted $(IMAGE_NAME) -s -a minimal mkpart EFI FAT32 2048s 93716s
	parted $(IMAGE_NAME) -s -a minimal toggle 1 boot
	# Copy the data from efi.img into the correct place
	dd if=$(BUILD_DIR)/fat.img of=$(IMAGE_NAME) bs=512 count=91669 seek=2048 conv=notrunc
	rm $(BUILD_DIR)/fat.img

prepare:
	mkdir -p $(BUILD_DIR)/fat/efi/boot/

kernel:
	make -C kernel kernel_$(PLATFORM)

user:
	make -C user

clean:
	make -C kernel clean
	make -C user clean
	rm -rf build
	rm -f $(IMAGE_NAME)

update:
	make -C kernel update
	make -C user update
	cargo update --manifest-path lib/libpebble/Cargo.toml
	cargo update --manifest-path lib/mer/Cargo.toml
	cargo update --manifest-path lib/pebble_util/Cargo.toml

fmt:
	@# `cargo fmt` doesn't play nicely with conditional compilation, so we manually `rustfmt` things
	find kernel/src -type f -name "*.rs" -exec rustfmt {} +
	cd lib/libpebble && cargo fmt
	cd bootloader && cargo fmt

test:
	cargo test --all-features --manifest-path lib/pebble_util/Cargo.toml
	cargo test --manifest-path lib/ptah/Cargo.toml
	make -C kernel test

qemu: image_$(PLATFORM)
	$(QEMU_DIR)qemu-system-x86_64 \
		$(QEMU_COMMON_FLAGS) \
		$(QEMU_EXTRA_FLAGS) \
		-enable-kvm \
		-serial stdio \
		-display none

qemu-no-kvm: image_$(PLATFORM)
	$(QEMU_DIR)qemu-system-x86_64 \
		$(QEMU_COMMON_FLAGS) \
		$(QEMU_EXTRA_FLAGS) \
		-serial stdio \
		-display none

debug: image_$(PLATFORM)
	$(QEMU_DIR)qemu-system-x86_64 \
		$(QEMU_COMMON_FLAGS) \
		$(QEMU_EXTRA_FLAGS) \
		-d int

gdb: image_$(PLATFORM)
	$(QEMU_DIR)qemu-system-x86_64 \
		$(QEMU_COMMON_FLAGS) \
		$(QEMU_EXTRA_FLAGS) \
		--enable-kvm \
		-s \
		-S \
	& tools/rust_gdb -q "build/fat/kernel.elf" -ex "target remote :1234"
