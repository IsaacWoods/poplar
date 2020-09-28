.PHONY: user echo pci_bus simple_fb update clean

user: echo pci_bus simple_fb;

echo:
	cargo build -Z build-std=core,alloc --target $(PLATFORM)-pebble-userspace.json --manifest-path echo/Cargo.toml
	cp target/$(PLATFORM)-pebble-userspace/debug/echo $(BUILD_DIR)/fat/echo.elf

pci_bus:
	cargo build -Z build-std=core,alloc --target $(PLATFORM)-pebble-userspace.json --manifest-path pci_bus/Cargo.toml
	cp target/$(PLATFORM)-pebble-userspace/debug/pci_bus $(BUILD_DIR)/fat/pci_bus.elf

simple_fb:
	cargo build -Z build-std=core,alloc --target $(PLATFORM)-pebble-userspace.json --manifest-path simple_fb/Cargo.toml
	cp target/$(PLATFORM)-pebble-userspace/debug/simple_fb $(BUILD_DIR)/fat/simple_fb.elf

update:
	cargo update

clean:
	cargo clean
