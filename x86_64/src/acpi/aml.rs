/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

use core::str;
use alloc::String;
use memory::paging::VirtualAddress;
use super::AcpiInfo;
use bit_field::BitField;

#[derive(Debug)]
enum AmlOpcode
{
    ScopeOp = 0x10,
}

impl AmlOpcode
{
    fn from_u8(byte : u8) -> Option<AmlOpcode>
    {
        use self::AmlOpcode::*;

        match byte
        {
            0x10 => Some(ScopeOp),
            _    => None,
        }
    }
}

pub(super) struct AmlParser
{
    address             : VirtualAddress,
    remaining_bytes     : usize,

    /*
     * This is set when we parse an object with a PkgLength. When it hits 0, we know we've parsed
     * the whole object, removing ambiguities.
     */
    remaining_pkg_bytes : usize,
}

impl Iterator for AmlParser
{
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item>
    {
        match self.remaining_bytes
        {
            0 => None,

            _ =>
            {
                let byte = unsafe { *(self.address.ptr()) };
                self.address = self.address.offset(1);
                self.remaining_bytes -= 1;

                trace!("AML parser consumes: {:#x}", byte);

                Some(byte)
            },
        }
    }
}

impl AmlParser
{
    /// Create a new AmlParser, which will parse from the given address for the given number of
    /// bytes. Unsafe because the parser assumes the address is valid.
    pub(super) unsafe fn new(start : VirtualAddress, length : usize) -> AmlParser
    {
        AmlParser
        {
            address             : start,
            remaining_bytes     : length,
            remaining_pkg_bytes : 0,
        }
    }

    pub(super) fn parse(&mut self, acpi_info : &mut AcpiInfo)
    {
        while let Some(byte) = self.next()
        {
            match AmlOpcode::from_u8(byte)
            {
                Some(AmlOpcode::ScopeOp) =>
                {
                    self.parse_scope_op(acpi_info);
                },

                None =>
                {
                    panic!("Unrecognised AML opcode at top-level: {:#x}", byte);
                },
            }
        }
    }

    fn consume<F>(&mut self, predicate : F) -> u8
        where F : Fn(u8) -> bool
    {
        let byte = self.next().expect("Consume hit end of stream");
        
        if !predicate(byte)
        {
            panic!("AML parser consumed unexpected byte: {:#x}", byte);
        }

        byte
    }

    fn parse_pkg_length(&mut self) -> u32
    {
        /*
         * PkgLength := PkgLeadByte |
         *              <PkgLeadByte ByteData> |
         *              <PkgLeadByte ByteData ByteData> |
         *              <PkgLeadByte ByteData ByteData ByteData> |
         *
         * The maximum value of this is 2^28, so we return u32
         */
        let lead_byte = self.next().unwrap();
        let byte_data_count = lead_byte.get_bits(6..8);
        info!("PkgLength has {} data bytes", byte_data_count);

        if byte_data_count == 0
        {
            return lead_byte.get_bits(0..6) as u32;
        }

        let mut length = lead_byte.get_bits(0..4) as u32;

        for i in 0..byte_data_count
        {
            length += (self.next().unwrap() as u32) << 4 + i * 8;
        }

        /*
         * Set the number of bytes left in the current structure, minus the size of this PkgLength.
         */
        self.remaining_pkg_bytes = length as usize - 1 - byte_data_count as usize;

        length
    }

    fn parse_name_seg(&mut self) -> [u8; 4]
    {
        [self.consume(is_lead_name_char),
         self.consume(is_name_char),
         self.consume(is_name_char),
         self.consume(is_name_char)]
    }

    fn parse_name_path(&mut self) -> String
    {
        /*
         * NamePath         := NameSeg | DualNamePath | MultiNamePath | 0x00
         * DualNamePath     := 0x2E NameSeg NameSeg
         * MultiNamePath    := 0x2F SegCount{ByteData} NameSeg(..SegCount)
         * NameSeg          := <LeadNameChar NameChar NameChar NameChar>
         */
        let first_byte = self.next().unwrap();

        match first_byte
        {
            0x00 =>
            {
                String::from("")
            },

            0x2E =>
            {
                // NamePath := DualNamePath
                let first = self.parse_name_seg();
                let second = self.parse_name_seg();

                let mut path = String::from(str::from_utf8(&first).unwrap());
                path.push_str(str::from_utf8(&second).unwrap());
                path
            },

            0x2F =>
            {
                // NamePath := MultiNamePath
                let seg_count = self.next().unwrap();
                let mut path = String::new();

                for i in 0..seg_count
                {
                    path.push_str(str::from_utf8(&self.parse_name_seg()).unwrap());
                }

                path
            },

            _ =>
            {
                String::from(str::from_utf8(&self.parse_name_seg()).unwrap())
            },
        }
    }

    fn parse_name_string(&mut self) -> String
    {

        /*
         * NameString       := <RootChar NamePath> | <PrefixPath NamePath>
         * PrefixPath       := Nothing | <'^' PrefixPath>
         */
        let first_byte = self.next().unwrap();

        match first_byte
        {
            b'\\' =>
            {
                // NameString := RootChar NamePath
                String::from("\\") + &self.parse_name_path()
            },

            b'^' =>
            {
                // NameString := PrefixPath NamePath
                let string = String::from("^");
                error!("Haven't actually parsed this name string, TODO");
                //TODO
                string
            },

            _ => panic!("Failed to parse name string"),
        }
    }

    fn parse_scope_op(&mut self, acpi_info : &mut AcpiInfo)
    {
        /*
         * DefScope := 0x10 PkgLength NameString TermList
         */
        let pkg_length = self.parse_pkg_length();
        info!("Pkg length = {},{}", pkg_length, self.remaining_pkg_bytes);
        let name_string = self.parse_name_string();
        info!("Name string: {}", name_string);
    }
}

fn is_lead_name_char(byte : u8) -> bool
{
    (byte >= b'A' && byte <= b'Z') || byte == b'_'
}

fn is_digit_char(byte : u8) -> bool
{
    byte >= b'0' && byte <= b'9'
}

fn is_name_char(byte : u8) -> bool
{
    is_lead_name_char(byte) || is_digit_char(byte)
}
