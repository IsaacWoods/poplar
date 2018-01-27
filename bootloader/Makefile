# Copyright (C) 2018, Isaac Woods. See LICENCE.md
.DEFAULT: bootloader.img
.PHONY: clean dump

bootloader.img: stage0.bin
	dd if=/dev/zero of=$@ bs=1024 count=1440
	dd if=stage0.bin of=$@ seek=0 count=1 conv=notrunc

stage0.bin: stage0.s
	nasm -fbin -o $@ $<

dump-stage0: stage0.bin
	objdump -mi386 -Mintel -D -b binary $<

clean:
	rm -rf *.bin
	rm -rf bootloader.img
