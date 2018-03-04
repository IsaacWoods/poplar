# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

.PHONY: clean

flat_binary.bin: flat_binary.s
	nasm -fbin -o $@ $<

clean:
	rm -f flat_binary.bin
