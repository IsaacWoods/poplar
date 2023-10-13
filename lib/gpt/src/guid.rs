use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Guid([u8; 16]);

impl Guid {
    pub const UNUSED: Self = Self::parse("00000000-0000-0000-0000-000000000000").unwrap();
    pub const EFI_SYSTEM_PARTITION: Self = Self::parse("c12a7328-f81f-11d2-ba4b-00a0c93ec93b").unwrap();
    pub const LEGACY_MBR_PARTITION: Self = Self::parse("024dee41-33e7-11d3-9d69-0008c781f39f").unwrap();

    /// Parse a GUID in the standard text representation as described by Appendix A of the UEFI standard (sometimes
    /// called the "registry format").
    pub const fn parse(s: &str) -> Option<Guid> {
        let bytes = s.as_bytes();

        // Make sure it's the right length. We then don't need to do any bounds checks.
        if bytes.len() != 36 {
            return None;
        }

        const fn decode_hex(byte: u8) -> Option<u8> {
            match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(0xa + byte - b'a'),
                b'A'..=b'F' => Some(0xa + byte - b'A'),
                _ => None,
            }
        }

        /*
         * Decode each pair of chars into a byte.
         * XXX: I can't seem to figure out why the ordering seems to change halfway through, but I've now spent
         * longer thinking about GUIDs than I ever wanted to, so if it works, it works.
         */
        let mut i = 0;
        let mut buf: [u8; 16] = [0; 16];
        while i < bytes.len() {
            match i {
                8 | 13 | 18 | 23 => {
                    if bytes[i] != b'-' {
                        return None;
                    }
                }
                0 | 1 => buf[3] = (buf[3] << 4) | decode_hex(bytes[i]).unwrap(),
                2 | 3 => buf[2] = (buf[2] << 4) | decode_hex(bytes[i]).unwrap(),
                4 | 5 => buf[1] = (buf[1] << 4) | decode_hex(bytes[i]).unwrap(),
                6 | 7 => buf[0] = (buf[0] << 4) | decode_hex(bytes[i]).unwrap(),
                9 | 10 => buf[5] = (buf[5] << 4) | decode_hex(bytes[i]).unwrap(),
                11 | 12 => buf[4] = (buf[4] << 4) | decode_hex(bytes[i]).unwrap(),
                14 | 15 => buf[7] = (buf[7] << 4) | decode_hex(bytes[i]).unwrap(),
                16 | 17 => buf[6] = (buf[6] << 4) | decode_hex(bytes[i]).unwrap(),
                19 | 20 => buf[8] = (buf[8] << 4) | decode_hex(bytes[i]).unwrap(),
                21 | 22 => buf[9] = (buf[9] << 4) | decode_hex(bytes[i]).unwrap(),
                24 | 25 => buf[10] = (buf[10] << 4) | decode_hex(bytes[i]).unwrap(),
                26 | 27 => buf[11] = (buf[11] << 4) | decode_hex(bytes[i]).unwrap(),
                28 | 29 => buf[12] = (buf[12] << 4) | decode_hex(bytes[i]).unwrap(),
                30 | 31 => buf[13] = (buf[13] << 4) | decode_hex(bytes[i]).unwrap(),
                32 | 33 => buf[14] = (buf[14] << 4) | decode_hex(bytes[i]).unwrap(),
                34 | 35 => buf[15] = (buf[15] << 4) | decode_hex(bytes[i]).unwrap(),
                _ => unreachable!(),
            }
            i += 1;
        }

        Some(Self(buf))
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Guid({:0<8x}-{:0<4x}-{:0<4x}-{:0<2x}{:0<2x}-{:0<2x}{:0<2x}{:0<2x}{:0<2x}{:0<2x}{:0<2x})",
            u32::from_le_bytes(self.0[0..4].try_into().unwrap()),
            u16::from_le_bytes(self.0[4..6].try_into().unwrap()),
            u16::from_le_bytes(self.0[6..8].try_into().unwrap()),
            self.0[8],
            self.0[9],
            self.0[10],
            self.0[11],
            self.0[12],
            self.0[13],
            self.0[14],
            self.0[15],
        )
    }
}
