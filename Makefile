export ARCH ?= x86_64
export BUILD_DIR ?= $(abspath ./build)
export RAMDISK ?= $(abspath ./ramdisk)

RUST_GDB_INSTALL_PATH ?= ~/bin/rust-gdb/bin/
GRUB_MKRESCUE ?= grub2-mkrescue

.PHONY: prepare kernel rust ramdisk clean qemu gdb update fmt

pebble.iso: prepare kernel ramdisk kernel/grub.cfg
	cp $(BUILD_DIR)/kernel.bin $(BUILD_DIR)/iso/boot/kernel.bin
	cp kernel/grub.cfg $(BUILD_DIR)/iso/boot/grub/grub.cfg
	$(GRUB_MKRESCUE) -o $@ $(BUILD_DIR)/iso 2> /dev/null

# This is a general target to prepare the directory structure so all the things exist when we expect them to
prepare:
	@mkdir -p $(RAMDISK)
	@mkdir -p $(BUILD_DIR)/iso/boot/grub

kernel:
	make -C kernel/$(ARCH) $(BUILD_DIR)/kernel.bin

rust:
	cd rust && \
	python ./x.py build --stage=1 --incremental --target=x86_64-unknown-pebble src/libstd && \
	cd ..

# This must be depended upon AFTER everything has been put in $(RAMDISK)
ramdisk:
	cd $(RAMDISK) && \
	echo "This is a file on the ramdisk" > test_file && \
	tar -c -f $(BUILD_DIR)/iso/ramdisk.tar * && \
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

qemu: pebble.iso
	qemu-system-$(ARCH)\
		-enable-kvm\
		-smp 2\
		-usb\
		-device usb-ehci,id=ehci\
		--no-reboot\
		--no-shutdown\
		-cdrom $<

debug: pebble.iso
	@echo "Start and connect a GDB instance by running 'make gdb'"
	qemu-system-$(ARCH)\
		-enable-kvm\
		-no-reboot\
		-no-shutdown\
		-s\
		-S\
		-cdrom $<

gdb:
	$(RUST_GDB_INSTALL_PATH)rust-gdb -q "build/kernel.bin" -ex "target remote :1234"
