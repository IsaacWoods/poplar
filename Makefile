# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

export ARCH?=x86_64
export BUILD_DIR:=$(abspath ./build)

.PHONY: kernel bootloader clean run debug gdb test_program

# TODO: When we move to using the bootloader, add the phony dependency here
os.iso: grub.cfg kernel test_program
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	cp $(BUILD_DIR)/kernel.bin $(BUILD_DIR)/iso/boot/kernel.bin
	cp test_program/test_program.bin $(BUILD_DIR)/iso/test_program.bin
	cp grub.cfg $(BUILD_DIR)/iso/boot/grub/grub.cfg
	grub2-mkrescue -o $@ $(BUILD_DIR)/iso 2> /dev/null

kernel:
	make -C kernel $(BUILD_DIR)/kernel.bin

bootloader:
	mkdir -p iso
	make -C bootloader
	cp bootloader/bootloader.img bootloader.img		# Needs to be in . for some reason ¯\_(ツ)_/¯
	cp bootloader/bootloader.img iso/bootloader.img
	mkisofs -o bootloader.iso -V 'RustOS' -b bootloader.img -hide bootloader.img iso/
	rm bootloader.img

test_program:
	make -C test_program test_program.bin

clean:
	make -C bootloader clean
	make -C kernel clean
	make -C test_program clean
	rm -rf build
	rm -rf os.iso

run: os.iso
	qemu-system-$(ARCH) -enable-kvm --no-reboot --no-shutdown -cdrom $<

run-bootloader: bootloader.iso
	qemu-system-$(ARCH) -enable-kvm --no-reboot --no-shutdown -cdrom $<

debug: os.iso
	@echo "Connect with (gdb)target remote localhost:1234"
	qemu-system-$(ARCH) -enable-kvm -no-reboot -no-shutdown -s -S -cdrom $<

gdb:
	gdb/bin/rust-gdb -q "build/kernel.bin" -ex "target remote :1234"
