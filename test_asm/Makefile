# Copyright (C) 2017, Pebble Developers.
# See LICENCE.md

.PHONY: clean

test_asm.elf: test_asm.o
	ld -o $@ $<

test_asm.o: test_asm.s
	nasm -felf64 -o $@ $<

clean:
	rm -f test_asm.o test_asm.elf
