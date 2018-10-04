.PHONY: qemu

image.img:
	cargo xbuild --release --target uefi_x64.json
	dd if=/dev/zero of=image.img bs=512 count=93750
	# Create GPT headers and a single EFI partition
	parted image.img -s -a minimal mklabel gpt
	parted image.img -s -a minimal mkpart EFI FAT16 2048s 93716s
	parted image.img -s -a minimal toggle 1 boot
	# Create a temporary image file for the EFI FAT partition
	dd if=/dev/zero of=efi.img bs=1M count=64
	mkfs.vfat -F 32 efi.img -n Boot
	# Copy the bootloader over and merge the images
	mcopy -i efi.img target/uefi_x64/release/bootloader.efi ::/EFI/BOOT/BOOTX64.efi
	mcopy -i efi.img test.txt ::
	dd if=efi.img of=image.img bs=512 count=91669 seek=2048 conv=notrunc
	rm efi.img

qemu: image.img
	qemu-system-x86_64 \
		-drive if=pflash,format=raw,file=OVMF_CODE-pure-efi.fd,readonly=on \
		-drive if=pflash,format=raw,file=OVMF_VARS-pure-efi.fd,readonly=off \
		-drive format=raw,file=image.img,if=ide \
		-net none
