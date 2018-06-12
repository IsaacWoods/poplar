export ARCH ?= x86_64
export BUILD_DIR ?= $(abspath ./build)
export RAMDISK ?= $(abspath ./ramdisk)

RUST_GDB_INSTALL_PATH ?= ~/bin/rust-gdb/bin/
GRUB_MKRESCUE ?= grub2-mkrescue

.PHONY: kernel rust ramdisk test_asm test_rust clean qemu gdb update fmt

pebble.iso: kernel ramdisk test_rust kernel/grub.cfg
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	cp $(BUILD_DIR)/kernel.bin $(BUILD_DIR)/iso/boot/kernel.bin
	cp kernel/grub.cfg $(BUILD_DIR)/iso/boot/grub/grub.cfg
	$(GRUB_MKRESCUE) -o $@ $(BUILD_DIR)/iso 2> /dev/null

kernel:
	make -C kernel/$(ARCH) $(BUILD_DIR)/kernel.bin

rust:
	cd rust && \
	python ./x.py build --stage=1 --incremental --target=x86_64-unknown-pebble src/libstd && \
	cd ..

ramdisk:
	mkdir -p $(RAMDISK) && \
	cd $(RAMDISK) && \
	echo "This is a file on the ramdisk" > test_file && \
	tar -c -f $(BUILD_DIR)/iso/ramdisk.tar * && \
	cd ..

test_asm:
	make -C test_asm test_asm.elf
	# cp test_asm/test_asm.elf $(RAMDISK)/test_process.elf

test_rust:
	cd test_rust && \
	cargo rustc -- -Z pre-link-arg=-nostartfiles && \
	cp target/x86_64-pebble-nostd/debug/test_rust $(RAMDISK)/test_process.elf && \
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
