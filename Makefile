# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

ARCH?=x86_64
BUILD_DIR:=./build

ASM_SOURCES:=$(wildcard src/$(ARCH)/*.s)
ASM_OBJS:=$(patsubst src/$(ARCH)/%.s, $(BUILD_DIR)/$(ARCH)/%.o, $(ASM_SOURCES))

.PHONY: clean run debug

os.iso: $(BUILD_DIR)/kernel.bin grub.cfg
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	cp $(BUILD_DIR)/kernel.bin $(BUILD_DIR)/iso/boot/kernel.bin
	cp grub.cfg $(BUILD_DIR)/iso/boot/grub/grub.cfg
	grub-mkrescue -o $@ $(BUILD_DIR)/iso 2> /dev/null
	rm -r $(BUILD_DIR)/iso

$(BUILD_DIR)/kernel.bin: $(ASM_OBJS) linker.ld
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	ld -n -T linker.ld -o $@ $(ASM_OBJS)

$(BUILD_DIR)/$(ARCH)%.o: src/$(ARCH)%.s
	mkdir -p $(shell dirname $@)
	nasm -g -felf64 $< -o $@

clean:
	rm -rf build
	rm -rf os.iso

run: os.iso
	qemu-system-$(ARCH) -cdrom os.iso

debug: os.iso
	@echo "Connect with (gdb)target remote localhost:1234"
	qemu-system-$(ARCH) -s -S -cdrom os.iso
