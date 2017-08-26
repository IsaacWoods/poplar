# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

ARCH?=x86_64
BUILD_DIR:=./build

ASM_SOURCES:=$(wildcard src/$(ARCH)/*.s)
ASM_OBJS:=$(patsubst src/$(ARCH)/%.s, src/$(ARCH)/%.o, $(ASM_SOURCES))

.PHONY: clean run

os.iso: $(BUILD_DIR)/kernel.bin grub.cfg
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	cp $(BUILD_DIR)/kernel.bin $(BUILD_DIR)/iso/boot/kernel.bin
	cp grub.cfg $(BUILD_DIR)/iso/boot/grub/grub.cfg
	grub-mkrescue -o $@ $(BUILD_DIR)/iso 2> /dev/null
	rm -r $(BUILD_DIR)/iso

$(BUILD_DIR)/kernel.bin: $(ASM_OBJS) linker.ld
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	ld -n -T linker.ld -o $@ $(ASM_OBJS)

%.o: %.s
	mkdir -p $(shell dirname $@)
	nasm -felf64 $< -o $@

clean:
	rm -rf build

run: os.iso
	qemu-system-x86_64 -cdrom os.iso
