.PHONY: kernel clean

kernel:
	cargo xbuild --target=$(ARCH)-kernel.json --features arch_$(ARCH)
	ld --gc-sections -T src/$(ARCH)/link.ld -o $(BUILD_DIR)/fat/kernel.elf target/$(ARCH)-kernel/debug/libkernel.a

clean:
	cargo clean
