.PHONY: kernel_x86_64 efiloader kernel_rpi4 clean

kernel_x86_64: efiloader
	cargo xbuild --target=kernel_x86_64/x86_64-kernel.json --manifest-path kernel_x86_64/Cargo.toml
	ld --gc-sections -T kernel_x86_64/link.ld -o $(BUILD_DIR)/fat/kernel.elf target/x86_64-kernel/debug/libkernel_x86_64.a

efiloader:
	cargo xbuild --target=x86_64-unknown-uefi --manifest-path efiloader/Cargo.toml
	cp target/x86_64-unknown-uefi/debug/efiloader.efi $(BUILD_DIR)/fat/efiloader.efi

kernel_rpi4:
	RUSTFLAGS="-Ctarget-cpu=cortex-a72" cargo xbuild --target=kernel_rpi4/rpi4-kernel.json --manifest-path kernel_rpi4/Cargo.toml

clean:
	cargo clean
