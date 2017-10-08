# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

LINKER_SCRIPT=src/$(ARCH)/linker.ld
LFLAGS:=-n --gc-sections -T $(LINKER_SCRIPT)

ASM_FLAGS:=-g -felf64
#ASM_SOURCES:=$(wildcard src/$(ARCH)/*.s)
#ASM_OBJS:=$(patsubst src/$(ARCH)/%.s, $(BUILD_DIR)/$(ARCH)/%.o, $(ASM_SOURCES))
ASM_SOURCES:=src/x86_64/bootstrap.s
ASM_OBJS:=$(BUILD_DIR)/x86_64/bootstrap.o

.PHONY: kernel clean

$(BUILD_DIR)/kernel.bin: $(ASM_OBJS) kernel $(LINKER_SCRIPT)
	mkdir -p $(BUILD_DIR)/iso/boot/grub
	#ld $(LFLAGS) -o $@ $(ASM_OBJS) target/$(ARCH)-rustos/debug/librust_os.a
	ld $(LFLAGS) -o $@ $(ASM_OBJS)

kernel:
	xargo build --target=$(ARCH)-rustos

$(BUILD_DIR)/$(ARCH)%.o: src/$(ARCH)%.s
	mkdir -p $(shell dirname $@)
	nasm $(ASM_FLAGS) $< -o $@

clean:
	rm -f $(ASM_OBJS)
	xargo clean
