.PHONY: kernel_x86_64 efiloader clean

kernel_x86_64: efiloader
	cargo xbuild --target=hal_$(ARCH)/$(ARCH)-kernel.json --features arch_$(ARCH)
	ld --gc-sections -T hal_$(ARCH)/link.ld -o $(BUILD_DIR)/fat/kernel.elf target/$(ARCH)-kernel/debug/libkernel.a

efiloader:
	cargo xbuild --target=x86_64-unknown-uefi --manifest-path efiloader/Cargo.toml
	cp target/x86_64-unknown-uefi/debug/efiloader.efi $(BUILD_DIR)/fat/efiloader.efi

clean:
	cargo clean
