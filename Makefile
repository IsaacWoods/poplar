export ARCH ?= x86_64
export BUILD_DIR ?= $(abspath ./build)

.PHONY: prepare bootloader kernel clean qemu gdb update fmt

pebble.img: prepare bootloader kernel
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
	cd bootloader &&\
	cargo xbuild --release --target uefi_x64.json &&\
	cp target/uefi_x64/release/bootloader.efi $(BUILD_DIR)/fat/EFI/BOOT/BOOTX64.efi &&\
	cd ..

kernel:
	cd kernel/$(ARCH) &&\
	cargo xbuild --target=$(ARCH)-pebble-kernel.json &&\
	ld -n --gc-sections -T linker.ld -o $(BUILD_DIR)/fat/kernel.elf ../target/$(ARCH)-pebble-kernel/debug/libx86_64.a &&\
	cd ..

# This does NOT clean the Rust submodule - it takes ages to build and you probably don't want to
clean:
	make -C kernel/$(ARCH) clean
	rm -rf build pebble.iso

update:
	cd kernel && \
	cargo update && \
	cd x86_64 && \
	cargo update && \
	cd ../..

fmt:
	cd kernel && \
	cargo fmt && \
	cd x86_64 && \
	cargo fmt && \
	cd ../heap_allocator && \
	cargo fmt && \
	cd ../..
	cd libmessage && \
	cargo fmt && \
	cd ..

qemu: pebble.img
	qemu-system-x86_64 \
		-enable-kvm \
		-smp 2 \
		-usb \
		-device usb-ehci,id=ehci \
		--no-reboot \
		--no-shutdown \
		-drive if=pflash,format=raw,file=bootloader/ovmf/OVMF_CODE.fd,readonly \
		-drive if=pflash,format=raw,file=bootloader/ovmf/OVMF_VARS.fd,readonly \
		-drive format=raw,file=$<,if=ide \
		-net none
