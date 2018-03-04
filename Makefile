# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

export ARCH?=x86_64
export BUILD_DIR:=$(abspath ./build)

.PHONY: kernel clean run debug gdb flat_binary_test

pebble.iso: grub.cfg kernel flat_binary_test
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	cp $(BUILD_DIR)/kernel.bin $(BUILD_DIR)/iso/boot/kernel.bin
	cp flat_binary_test/flat_binary.bin $(BUILD_DIR)/iso/flat_binary.bin
	cp grub.cfg $(BUILD_DIR)/iso/boot/grub/grub.cfg
	grub2-mkrescue -o $@ $(BUILD_DIR)/iso 2> /dev/null

kernel:
	make -C kernel/$(ARCH) $(BUILD_DIR)/kernel.bin

flat_binary_test:
	make -C flat_binary_test flat_binary.bin

clean:
	make -C kernel/$(ARCH) clean
	make -C flat_binary_test clean
	rm -rf build
	rm -rf pebble.iso

run: pebble.iso
	qemu-system-$(ARCH) -enable-kvm --no-reboot --no-shutdown -cdrom $<

debug: pebble.iso
	@echo "Connect with (gdb)target remote localhost:1234"
	qemu-system-$(ARCH) -enable-kvm -no-reboot -no-shutdown -s -S -cdrom $<

gdb:
	gdb/bin/rust-gdb -q "build/kernel.bin" -ex "target remote :1234"
