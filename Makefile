# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

ARCH?=x86_64
BUILD_DIR:=./build

LFLAGS:=-n --gc-sections -T linker.ld

ASM_SOURCES:=$(wildcard src/$(ARCH)/*.s)
ASM_OBJS:=$(patsubst src/$(ARCH)/%.s, $(BUILD_DIR)/$(ARCH)/%.o, $(ASM_SOURCES))

.PHONY: kernel clean run debug gdb

os.iso: grub.cfg $(BUILD_DIR)/kernel.bin
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	cp $(BUILD_DIR)/kernel.bin $(BUILD_DIR)/iso/boot/kernel.bin
	cp grub.cfg $(BUILD_DIR)/iso/boot/grub/grub.cfg
	grub2-mkrescue -o $@ $(BUILD_DIR)/iso 2> /dev/null
	rm -r $(BUILD_DIR)/iso

$(BUILD_DIR)/kernel.bin: $(ASM_OBJS) kernel linker.ld
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	ld $(LFLAGS) -o $@ $(ASM_OBJS) target/$(ARCH)-rustos/debug/librust_os.a

kernel:
	xargo build --target=$(ARCH)-rustos

$(BUILD_DIR)/$(ARCH)%.o: src/$(ARCH)%.s
	mkdir -p $(shell dirname $@)
	nasm -g -felf64 $< -o $@

clean:
	xargo clean
	rm -rf build
	rm -rf os.iso

run: os.iso
	qemu-system-$(ARCH) -enable-kvm -cdrom os.iso

debug: os.iso
	@echo "Connect with (gdb)target remote localhost:1234"
	qemu-system-$(ARCH) -enable-kvm -s -S -cdrom os.iso

gdb:
	rust-os-gdb/bin/rust-gdb "build/kernel.bin" -ex "target remote :1234"
