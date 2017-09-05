# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

ARCH?=x86_64
BUILD_DIR:=./build

LINKER_SCRIPT=kernel/src/$(ARCH)/linker.ld
LFLAGS:=-n --gc-sections -T $(LINKER_SCRIPT)

ASM_SOURCES:=$(wildcard kernel/src/$(ARCH)/*.s)
ASM_OBJS:=$(patsubst kernel/src/$(ARCH)/%.s, $(BUILD_DIR)/$(ARCH)/%.o, $(ASM_SOURCES))

.PHONY: kernel clean run debug gdb

os.iso: grub.cfg $(BUILD_DIR)/kernel.bin
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	cp $(BUILD_DIR)/kernel.bin $(BUILD_DIR)/iso/boot/kernel.bin
	cp grub.cfg $(BUILD_DIR)/iso/boot/grub/grub.cfg
	grub2-mkrescue -o $@ $(BUILD_DIR)/iso 2> /dev/null
	rm -r $(BUILD_DIR)/iso

$(BUILD_DIR)/kernel.bin: $(ASM_OBJS) kernel $(LINKER_SCRIPT)
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	ld $(LFLAGS) -o $@ $(ASM_OBJS) kernel/target/$(ARCH)-rustos/debug/librust_os.a

kernel:
	cd kernel ; xargo build --target=$(ARCH)-rustos

$(BUILD_DIR)/$(ARCH)%.o: kernel/src/$(ARCH)%.s
	mkdir -p $(shell dirname $@)
	nasm -g -felf64 $< -o $@

clean:
	cd kernel ; xargo clean
	rm -rf build
	rm -rf os.iso

run: os.iso
	qemu-system-$(ARCH) -enable-kvm -cdrom os.iso

debug: os.iso
	@echo "Connect with (gdb)target remote localhost:1234"
	qemu-system-$(ARCH) -enable-kvm -s -S -cdrom os.iso

gdb:
	rust-os-gdb/bin/rust-gdb "build/kernel.bin" -ex "target remote :1234"
