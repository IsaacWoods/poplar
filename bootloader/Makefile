.PHONY: image.img qemu

image.img:
	cargo xbuild --release --target uefi_x64.json
	dd if=/dev/zero of=image.img bs=512 count=93750
	# Create GPT headers and a single EFI partition
	parted image.img -s -a minimal mklabel gpt
	parted image.img -s -a minimal mkpart EFI FAT16 2048s 93716s
	parted image.img -s -a minimal toggle 1 boot
	# Create a temporary image file for the EFI partition
	dd if=/dev/zero of=efi.img bs=512 count=91669
	mformat -i efi.img -h 32 -t 32 -n 64 -c 1
	# Copy the bootloader over and merge the images
	mcopy -i efi.img target/uefi_x64/release/bootloader.efi ::boot.efi
	mcopy -i efi.img test.txt ::
	dd if=efi.img of=image.img bs=512 count=91669 seek=2048 conv=notrunc
	rm efi.img

qemu: image.img
	qemu-system-x86_64 -bios ../ovmf/OVMF-pure-efi.fd -drive file=image.img,if=ide -net none
