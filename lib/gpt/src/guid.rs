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

        // Check that there are hyphens in the places we expect there to be
        match (bytes[8], bytes[13], bytes[18], bytes[23]) {
            (b'-', b'-', b'-', b'-') => (),
            _ => return None,
        }

        /*
         * Decode pairs of hex-encoded bytes using a lookup table. We do this because it's the smallest unit to appear in
         * the GUID.
         *
         * GUID     aabbccdd-eeff-gghh-iijj-kkllmmnnoopp
         * Index       0   4    9   14   19   24  28  32
         */
        const HEX_TABLE: &[u8; 256] = &{
            let mut table = [0; 256];
            let mut i: u8 = 0;

            loop {
                table[i as usize] = match i {
                    b'0'..=b'9' => i - b'0',
                    b'a'..=b'f' => i - b'a' + 0xa,
                    b'A'..=b'F' => i - b'A' + 0xa,
                    _ => 0xff,
                };

                if i == 255 {
                    break table;
                }

                i += 1
            }
        };
        let indices: [u8; 8] = [0, 4, 9, 14, 19, 24, 28, 32];
        let mut buf: [u8; 16] = [0; 16];
        let mut group = 0;

        while group < 8 {
            let i = indices[group];

            let h1 = HEX_TABLE[bytes[i as usize] as usize];
            let h2 = HEX_TABLE[bytes[(i + 1) as usize] as usize];
            let h3 = HEX_TABLE[bytes[(i + 2) as usize] as usize];
            let h4 = HEX_TABLE[bytes[(i + 3) as usize] as usize];

            if h1 | h2 | h3 | h4 == 0xff {
                return None;
            }

            buf[group * 2] = h1.wrapping_shl(4) | h2;
            buf[group * 2 + 1] = h3.wrapping_shl(4) | h4;
            group += 1;
        }

        Some(Self(buf))
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Guid({:0<8x}-{:0<4x}-{:0<4x}-{:0<2x}{:0<2x}-{:0<2x}{:0<2x}{:0<2x}{:0<2x}{:0<2x}{:0<2x})",
            u32::from_be_bytes(self.0[0..4].try_into().unwrap()),
            u16::from_be_bytes(self.0[4..6].try_into().unwrap()),
            u16::from_be_bytes(self.0[6..8].try_into().unwrap()),
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
