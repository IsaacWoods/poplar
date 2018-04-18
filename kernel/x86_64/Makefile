# Copyright (C) 2017, Pebble Developers.
# See LICENCE.md

LINKER_SCRIPT = linker.ld
LFLAGS :=-n --gc-sections -T $(LINKER_SCRIPT)
TARGET := $(ARCH)-pebble-kernel

ASM_FLAGS:=-felf64 -Fdwarf -g
ASM_SOURCES:=$(wildcard src/*.s)
ASM_OBJS:=$(patsubst src/%.s, $(BUILD_DIR)/$(ARCH)/%.o, $(ASM_SOURCES))

.PHONY: kernel clean

$(BUILD_DIR)/kernel.bin: $(ASM_OBJS) kernel $(LINKER_SCRIPT)
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	ld $(LFLAGS) -o $@ $(ASM_OBJS) target/$(TARGET)/debug/lib$(ARCH).a

kernel:
	RUST_TARGET_PATH="$(shell pwd)" xargo build --target=$(TARGET) --features "clippy"

$(BUILD_DIR)/$(ARCH)%.o: src%.s
	mkdir -p $(shell dirname $@)
	nasm $(ASM_FLAGS) $< -o $@

clean:
	rm -f $(ASM_OBJS)
	xargo clean
	rm -f Cargo.lock
