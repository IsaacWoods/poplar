# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

LINKER_SCRIPT=$(ARCH)/linker.ld
LFLAGS:=-n --gc-sections -T $(LINKER_SCRIPT)

ASM_FLAGS:=-felf64 -Fdwarf -g
ASM_SOURCES:=$(wildcard $(ARCH)/src/*.s)
ASM_OBJS:=$(patsubst $(ARCH)/src/%.s, $(BUILD_DIR)/$(ARCH)/%.o, $(ASM_SOURCES))

.PHONY: kernel clean

$(BUILD_DIR)/kernel.bin: $(ASM_OBJS) kernel $(LINKER_SCRIPT)
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	ld $(LFLAGS) -o $@ $(ASM_OBJS) target/$(ARCH)-rustos/debug/librust_os.a

kernel:
	RUST_TARGET_PATH="$(shell pwd)" xargo build --target=$(ARCH)-rustos

$(BUILD_DIR)/$(ARCH)%.o: $(ARCH)/src/%.s
	mkdir -p $(shell dirname $@)
	nasm $(ASM_FLAGS) $< -o $@

clean:
	rm -f $(ASM_OBJS)
	xargo clean
