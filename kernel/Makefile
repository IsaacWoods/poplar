.PHONY: kernel_x86_64 efiloader kernel_rpi4 clean

kernel_x86_64: efiloader
	cargo xbuild --target=kernel_x86_64/x86_64-kernel.json --manifest-path kernel_x86_64/Cargo.toml
	cp target/x86_64-kernel/debug/kernel_x86_64 $(BUILD_DIR)/fat/kernel.elf

efiloader:
	cargo xbuild --target=x86_64-unknown-uefi --manifest-path efiloader/Cargo.toml
	cp target/x86_64-unknown-uefi/debug/efiloader.efi $(BUILD_DIR)/fat/efiloader.efi

kernel_rpi4:
	RUSTFLAGS="-Ctarget-cpu=cortex-a72" cargo xbuild --target=kernel_rpi4/rpi4-kernel.json --manifest-path kernel_rpi4/Cargo.toml

clean:
	cargo clean
