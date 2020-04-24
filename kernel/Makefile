.PHONY: kernel_x86_64 efiloader clean

kernel_x86_64: efiloader
	cargo xbuild --target=hal_x86_64/x86_64-kernel.json --features platform_x86_64
	ld --gc-sections -T hal_x86_64/link.ld -o $(BUILD_DIR)/fat/kernel.elf target/x86_64-kernel/debug/libkernel.a

efiloader:
	cargo xbuild --target=x86_64-unknown-uefi --manifest-path efiloader/Cargo.toml
	cp target/x86_64-unknown-uefi/debug/efiloader.efi $(BUILD_DIR)/fat/efiloader.efi

clean:
	cargo clean
