.PHONY: qemu bootloader img_dir

image.img: img_dir
	cargo xbuild --release --target uefi_x64.json
	dd if=/dev/zero of=image.img bs=512 count=93750
	# Create GPT headers and a single EFI partition
	parted image.img -s -a minimal mklabel gpt
	parted image.img -s -a minimal mkpart EFI FAT16 2048s 93716s
	parted image.img -s -a minimal toggle 1 boot
	# Copy the data from efi.img into the correct place
	dd if=efi.img of=image.img bs=512 count=91669 seek=2048 conv=notrunc
	rm efi.img

img_dir: bootloader
	mkdir -p build/EFI/BOOT
	cp target/uefi_x64/release/bootloader.efi build/EFI/BOOT/BOOTX64.efi
	# Create a temporary file for the EFI FAT
	dd if=/dev/zero of=efi.img bs=1M count=64
	mkfs.vfat -F 32 efi.img -n BOOT
	# Copy the stuff into the image
	mcopy -i efi.img -s build/EFI ::
	mcopy -i efi.img test.txt ::

bootloader:
	cargo xbuild --release --target uefi_x64.json

qemu: image.img
	qemu-system-x86_64 \
		-drive if=pflash,format=raw,file=ovmf/OVMF_CODE.fd,readonly=on \
		-drive if=pflash,format=raw,file=ovmf/OVMF_VARS.fd,readonly=on \
		-drive format=raw,file=image.img,if=ide \
		-net none
