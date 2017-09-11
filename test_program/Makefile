# Copyright (C) 2017, Isaac Woods.
# See LICENCE.md

.PHONY: clean

test_program.bin: main.s
	nasm -fbin -o $@ $<

clean:
	rm test_program.bin
